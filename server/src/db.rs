//! SQLite 持久层：schema/迁移、题库/文件夹 CRUD、请求日志、每日计数。
//! 逐函数移植自上游 database.rs；所有列名与 SQL 语义保持一致，
//! 以便直接复用桌面版的 airesponses.db。

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use parking_lot::Mutex;
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::matching::{
    compute_query_match_score, extract_urls, is_exact_match_score, normalize_optional_query_text,
    should_require_option_match,
};

// ---------------------------------------------------------------------------
// DTO
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIResponse {
    pub id: i64,
    pub question: String,
    pub options: Option<String>,
    pub answer: Option<String>,
    pub question_type: Option<String>,
    pub folder_id: i64,
    pub folder_name: Option<String>,
    pub create_time: Option<String>,
    pub is_ai: bool,
    pub is_pending_correction: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub id: i64,
    pub name: String,
    pub parent_id: i64,
    pub create_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderStat {
    pub folder_id: i64,
    pub folder_name: String,
    pub question_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderPathItem {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedAIResponses {
    pub items: Vec<AIResponse>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct QuestionMatch {
    pub id: i64,
    pub question: String,
    pub options: Option<String>,
    pub answer: String,
    pub is_ai: bool,
    pub is_pending_correction: bool,
    pub score: f64,
}

/// 新增/更新题目的请求体（camelCase）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NewQuestion {
    pub content: String,
    #[serde(default)]
    pub options: Option<String>,
    #[serde(default)]
    pub answer: Option<String>,
    #[serde(default)]
    pub question_type: Option<String>,
    #[serde(default)]
    pub folder_id: i64,
    #[serde(default)]
    pub is_ai: bool,
}

/// 更新题目的请求体（camelCase）；folder_id 缺省时不移动
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateQuestion {
    pub content: String,
    #[serde(default)]
    pub options: Option<String>,
    #[serde(default)]
    pub answer: Option<String>,
    #[serde(default)]
    pub question_type: Option<String>,
    #[serde(default)]
    pub folder_id: Option<i64>,
}

/// 持久化的请求日志（与 logger::RequestLog 字段一致）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedRequestLog {
    pub id: String,
    pub timestamp: String,
    pub method: String,
    pub path: String,
    pub status: Option<u16>,
    pub response_time: Option<u64>,
    pub request_body: Option<String>,
    pub response_body: Option<String>,
    pub headers: Option<HashMap<String, String>>,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
    pub stage: String,
}

type DbResult<T> = Result<T, String>;

fn e<E: std::fmt::Display>(err: E) -> String {
    err.to_string()
}

// ---------------------------------------------------------------------------
// Db 句柄
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct Db(Arc<Mutex<Connection>>);

impl Db {
    /// 打开数据库并初始化 schema（WAL、busy_timeout）
    pub fn open(path: &Path) -> anyhow::Result<Db> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.busy_timeout(std::time::Duration::from_millis(5000))?;
        let _ = conn.pragma_update(None, "journal_mode", "WAL");
        init_database_schema(&conn).map_err(|m| anyhow::anyhow!(m))?;
        Ok(Db(Arc::new(Mutex::new(conn))))
    }

    /// 仅测试：内存库
    #[cfg(test)]
    pub fn open_memory() -> Db {
        let conn = Connection::open_in_memory().expect("内存库");
        init_database_schema(&conn).expect("schema");
        Db(Arc::new(Mutex::new(conn)))
    }

    /// 在阻塞线程池中执行数据库操作
    pub async fn run<T, F>(&self, f: F) -> DbResult<T>
    where
        T: Send + 'static,
        F: FnOnce(&Connection) -> DbResult<T> + Send + 'static,
    {
        let db = self.0.clone();
        tokio::task::spawn_blocking(move || {
            let conn = db.lock();
            f(&conn)
        })
        .await
        .map_err(|err| format!("数据库任务失败: {err}"))?
    }

    /// 同步执行（测试用）
    #[cfg(test)]
    pub fn run_sync<T, F>(&self, f: F) -> DbResult<T>
    where
        F: FnOnce(&Connection) -> DbResult<T>,
    {
        let conn = self.0.lock();
        f(&conn)
    }

    // ----- 文件夹 -----

    pub async fn get_folders(&self) -> DbResult<Vec<Folder>> {
        self.run(ops::get_folders).await
    }
    pub async fn get_folder_stats(&self) -> DbResult<Vec<FolderStat>> {
        self.run(ops::get_folder_stats).await
    }
    pub async fn get_folder_path(&self, folder_id: i64) -> DbResult<Vec<FolderPathItem>> {
        self.run(move |c| ops::get_folder_path(c, folder_id)).await
    }
    pub async fn get_folder_question_count(&self, folder_id: i64) -> DbResult<i64> {
        self.run(move |c| ops::get_folder_question_count(c, folder_id)).await
    }
    pub async fn add_folder(&self, name: String, parent_id: i64) -> DbResult<i64> {
        self.run(move |c| ops::add_folder(c, &name, parent_id)).await
    }
    pub async fn rename_folder(&self, id: i64, new_name: String) -> DbResult<()> {
        self.run(move |c| ops::rename_folder(c, id, &new_name)).await
    }
    pub async fn move_folder(&self, id: i64, parent_id: i64) -> DbResult<()> {
        self.run(move |c| ops::move_folder(c, id, parent_id)).await
    }
    pub async fn delete_folder(&self, id: i64, delete_questions: bool) -> DbResult<()> {
        self.run(move |c| ops::delete_folder(c, id, delete_questions)).await
    }
    pub async fn clear_folder_questions(&self, id: i64) -> DbResult<()> {
        self.run(move |c| ops::clear_folder_questions(c, id)).await
    }

    // ----- 题目 -----

    pub async fn get_paginated_questions(
        &self,
        folder_id: Option<i64>,
        recursive: bool,
        pending_only: bool,
        page: u32,
        page_size: u32,
        sort_desc: bool,
    ) -> DbResult<PaginatedAIResponses> {
        self.run(move |c| {
            ops::get_paginated_questions(c, folder_id, recursive, pending_only, page, page_size, sort_desc)
        })
        .await
    }
    pub async fn get_questions_recursive(&self, folder_id: i64) -> DbResult<Vec<AIResponse>> {
        self.run(move |c| ops::get_questions_recursive(c, folder_id)).await
    }
    pub async fn get_ai_responses(&self, folder_id: Option<i64>) -> DbResult<Vec<AIResponse>> {
        self.run(move |c| ops::get_ai_responses(c, folder_id)).await
    }
    pub async fn get_pending_correction_questions(&self) -> DbResult<Vec<AIResponse>> {
        self.run(ops::get_pending_correction_questions).await
    }
    pub async fn get_pending_correction_question_count(&self) -> DbResult<i64> {
        self.run(ops::get_pending_correction_question_count).await
    }
    pub async fn set_question_pending_correction(&self, id: i64, pending: bool) -> DbResult<()> {
        self.run(move |c| ops::set_question_pending_correction(c, id, pending)).await
    }
    pub async fn add_question(&self, q: NewQuestion) -> DbResult<AIResponse> {
        self.run(move |c| ops::add_question(c, &q)).await
    }
    pub async fn add_questions_bulk(&self, items: Vec<NewQuestion>) -> DbResult<Vec<i64>> {
        self.run(move |c| ops::add_questions_bulk(c, &items)).await
    }
    pub async fn update_question(&self, id: i64, q: UpdateQuestion) -> DbResult<()> {
        self.run(move |c| ops::update_question(c, id, &q)).await
    }
    pub async fn move_question(&self, question_id: i64, target_folder_id: i64) -> DbResult<()> {
        self.run(move |c| ops::move_question(c, question_id, target_folder_id)).await
    }
    pub async fn copy_question(&self, question_id: i64, target_folder_id: i64) -> DbResult<()> {
        self.run(move |c| ops::copy_question(c, question_id, target_folder_id)).await
    }
    pub async fn delete_question(&self, id: i64) -> DbResult<()> {
        self.run(move |c| ops::delete_question(c, id)).await
    }
    pub async fn delete_questions(&self, ids: Vec<i64>) -> DbResult<()> {
        self.run(move |c| ops::delete_questions(c, &ids)).await
    }
    pub async fn search_questions_fuzzy(
        &self,
        keyword: String,
        folder_id: Option<i64>,
    ) -> DbResult<Vec<AIResponse>> {
        self.run(move |c| ops::search_questions_fuzzy(c, &keyword, folder_id)).await
    }

    // ----- 匹配 -----

    pub async fn query_exact(&self, title: String, options: Option<String>) -> DbResult<Vec<QuestionMatch>> {
        self.run(move |c| {
            let matches = ops::scan_question_matches(c, &title, options.as_deref())?;
            Ok(matches
                .into_iter()
                .filter(|m| is_exact_question_match(&title, options.as_deref(), m))
                .collect())
        })
        .await
    }
    pub async fn query_candidates(
        &self,
        title: String,
        options: Option<String>,
        limit: usize,
    ) -> DbResult<Vec<QuestionMatch>> {
        self.run(move |c| {
            let matches = ops::scan_question_matches(c, &title, options.as_deref())?;
            Ok(matches
                .into_iter()
                .filter(|m| !is_exact_question_match(&title, options.as_deref(), m))
                .take(limit)
                .collect())
        })
        .await
    }
    pub async fn get_ai_response_by_id(&self, id: i64) -> DbResult<QuestionMatch> {
        self.run(move |c| ops::get_ai_response_by_id(c, id)).await
    }
    pub async fn insert_ai_response(
        &self,
        question: String,
        answer: String,
        options: Option<String>,
        question_type: Option<String>,
        is_ai: bool,
        save_folder_id: Option<i64>,
    ) -> DbResult<i64> {
        self.run(move |c| {
            ops::insert_ai_response(c, &question, &answer, options, question_type, is_ai, save_folder_id)
        })
        .await
    }

    // ----- 日志/统计 -----

    pub async fn insert_request_log(&self, log: PersistedRequestLog, max_logs: usize) -> DbResult<()> {
        self.run(move |c| ops::insert_request_log(c, &log, max_logs)).await
    }
    pub async fn load_request_logs(&self, page: u32, page_size: u32) -> DbResult<(Vec<PersistedRequestLog>, i64)> {
        self.run(move |c| ops::load_request_logs(c, page, page_size)).await
    }
    pub async fn clear_request_logs(&self) -> DbResult<()> {
        self.run(ops::clear_request_logs).await
    }
    pub async fn increment_daily_request_count(&self) -> DbResult<()> {
        self.run(ops::increment_daily_request_count).await
    }
    pub async fn get_daily_request_counts(&self) -> DbResult<Vec<(String, i64)>> {
        self.run(ops::get_daily_request_counts).await
    }
}

// ---------------------------------------------------------------------------
// 匹配辅助（需要 QuestionMatch）
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn score_question_row(
    title: &str,
    query_options: &Option<String>,
    require_option_match: bool,
    id: i64,
    question: String,
    db_options: Option<String>,
    answer: String,
    is_ai: bool,
    is_pending_correction: bool,
) -> Option<QuestionMatch> {
    let query_urls = extract_urls(title);
    if !query_urls.is_empty() {
        let db_urls = extract_urls(&question);
        if query_urls != db_urls {
            return None;
        }
    }

    let title_similarity = compute_query_match_score(title, &question)?;

    let has_query_options = query_options.is_some();
    let option_similarity = match (
        query_options.as_deref(),
        normalize_optional_query_text(db_options.as_deref()),
    ) {
        (Some(query_options), Some(db_options)) => compute_query_match_score(query_options, &db_options),
        _ => None,
    };

    let is_exact_title_match = is_exact_match_score(title_similarity);
    if !is_exact_title_match && (require_option_match || has_query_options) && option_similarity.is_none() {
        return None;
    }

    let final_similarity = match option_similarity {
        Some(option_similarity) => title_similarity * 0.7 + option_similarity * 0.3,
        None => title_similarity,
    };

    Some(QuestionMatch {
        id,
        question,
        options: db_options,
        answer,
        is_ai,
        is_pending_correction,
        score: final_similarity,
    })
}

pub fn is_exact_question_match(title: &str, query_options: Option<&str>, matched: &QuestionMatch) -> bool {
    let Some(title_score) = compute_query_match_score(title, &matched.question) else {
        return false;
    };
    if !is_exact_match_score(title_score) {
        return false;
    }
    match normalize_optional_query_text(query_options) {
        None => true,
        Some(query_opts) => match normalize_optional_query_text(matched.options.as_deref()) {
            Some(db_opts) => compute_query_match_score(&query_opts, &db_opts)
                .map(is_exact_match_score)
                .unwrap_or(false),
            None => false,
        },
    }
}

// ---------------------------------------------------------------------------
// 同步操作（持有连接）
// ---------------------------------------------------------------------------

const SELECT_AI_RESPONSE_COLS: &str = "ar.Id, ar.Question, ar.Options, ar.Answer, ar.QuestionType, ar.FolderId, f.Name as FolderName, ar.CreateTime, ar.IsAi, COALESCE(ar.IsPendingCorrection, 0)";

fn map_ai_response_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AIResponse> {
    Ok(AIResponse {
        id: row.get(0)?,
        question: row.get(1)?,
        options: row.get(2)?,
        answer: row.get(3)?,
        question_type: row.get(4)?,
        folder_id: row.get(5)?,
        folder_name: row.get(6)?,
        create_time: row.get(7)?,
        is_ai: row.get(8)?,
        is_pending_correction: row.get(9)?,
    })
}

fn collect_rows<T>(
    rows: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>>,
) -> DbResult<Vec<T>> {
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(e)?);
    }
    Ok(out)
}

pub mod ops {
    use super::*;

    pub fn get_folders(conn: &Connection) -> DbResult<Vec<Folder>> {
        let mut stmt = conn
            .prepare("SELECT Id, Name, ParentId, CreateTime FROM Folders ORDER BY Name")
            .map_err(e)?;
        let rows = stmt
            .query_map([], |row| {
                Ok(Folder {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    parent_id: row.get(2)?,
                    create_time: row.get(3)?,
                })
            })
            .map_err(e)?;
        collect_rows(rows)
    }

    pub fn get_ai_responses(conn: &Connection, folder_id: Option<i64>) -> DbResult<Vec<AIResponse>> {
        let base = format!(
            "SELECT {SELECT_AI_RESPONSE_COLS} FROM AIResponses ar LEFT JOIN Folders f ON ar.FolderId = f.Id"
        );
        match folder_id {
            Some(fid) => {
                let mut stmt = conn
                    .prepare(&format!("{base} WHERE ar.FolderId = ? ORDER BY ar.CreateTime DESC"))
                    .map_err(e)?;
                let rows = stmt.query_map([fid], map_ai_response_row).map_err(e)?;
                collect_rows(rows)
            }
            None => {
                let mut stmt = conn
                    .prepare(&format!("{base} ORDER BY ar.CreateTime DESC"))
                    .map_err(e)?;
                let rows = stmt.query_map([], map_ai_response_row).map_err(e)?;
                collect_rows(rows)
            }
        }
    }

    /// 递归子树 CTE（不含 Id=0 的自环）
    const FOLDER_TREE_CTE: &str = "WITH RECURSIVE folder_tree AS (
        SELECT Id, Name, ParentId FROM Folders WHERE Id = ?
        UNION ALL
        SELECT f.Id, f.Name, f.ParentId FROM Folders f
        INNER JOIN folder_tree ft ON f.ParentId = ft.Id
        WHERE f.Id != ft.Id AND f.Id != 0
    )";

    pub fn get_paginated_questions(
        conn: &Connection,
        folder_id: Option<i64>,
        recursive: bool,
        pending_only: bool,
        page: u32,
        page_size: u32,
        sort_desc: bool,
    ) -> DbResult<PaginatedAIResponses> {
        let page = page.max(1) as i64;
        let page_size = (page_size as i64).clamp(1, 200);
        let offset = (page - 1) * page_size;
        let dir = if sort_desc { "DESC" } else { "ASC" };

        let (count_sql, data_sql, params): (String, String, Vec<i64>) = if pending_only {
            (
                "SELECT COUNT(*) FROM AIResponses WHERE COALESCE(IsPendingCorrection, 0) = 1".into(),
                format!(
                    "SELECT {SELECT_AI_RESPONSE_COLS} FROM AIResponses ar LEFT JOIN Folders f ON ar.FolderId = f.Id
                     WHERE COALESCE(ar.IsPendingCorrection, 0) = 1 ORDER BY ar.CreateTime {dir} LIMIT ? OFFSET ?"
                ),
                vec![],
            )
        } else {
            match folder_id {
                Some(0) => (
                    "SELECT COUNT(*) FROM AIResponses WHERE FolderId = 0".into(),
                    format!(
                        "SELECT {SELECT_AI_RESPONSE_COLS} FROM AIResponses ar INNER JOIN Folders f ON ar.FolderId = f.Id
                         WHERE ar.FolderId = 0 ORDER BY ar.CreateTime {dir} LIMIT ? OFFSET ?"
                    ),
                    vec![],
                ),
                Some(fid) if recursive => (
                    format!(
                        "{FOLDER_TREE_CTE} SELECT COUNT(*) FROM AIResponses ar INNER JOIN folder_tree ft ON ar.FolderId = ft.Id"
                    ),
                    format!(
                        "{FOLDER_TREE_CTE} SELECT {SELECT_AI_RESPONSE_COLS} FROM AIResponses ar
                         INNER JOIN folder_tree ft ON ar.FolderId = ft.Id
                         INNER JOIN Folders f ON ar.FolderId = f.Id
                         ORDER BY ar.CreateTime {dir} LIMIT ? OFFSET ?"
                    ),
                    vec![fid],
                ),
                Some(fid) => (
                    "SELECT COUNT(*) FROM AIResponses WHERE FolderId = ?".into(),
                    format!(
                        "SELECT {SELECT_AI_RESPONSE_COLS} FROM AIResponses ar LEFT JOIN Folders f ON ar.FolderId = f.Id
                         WHERE ar.FolderId = ? ORDER BY ar.CreateTime {dir} LIMIT ? OFFSET ?"
                    ),
                    vec![fid],
                ),
                None => (
                    "SELECT COUNT(*) FROM AIResponses".into(),
                    format!(
                        "SELECT {SELECT_AI_RESPONSE_COLS} FROM AIResponses ar LEFT JOIN Folders f ON ar.FolderId = f.Id
                         ORDER BY ar.CreateTime {dir} LIMIT ? OFFSET ?"
                    ),
                    vec![],
                ),
            }
        };

        let total: i64 = conn
            .query_row(&count_sql, rusqlite::params_from_iter(params.iter()), |r| r.get(0))
            .map_err(e)?;
        let mut all_params = params.clone();
        all_params.push(page_size);
        all_params.push(offset);
        let mut stmt = conn.prepare(&data_sql).map_err(e)?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(all_params.iter()), map_ai_response_row)
            .map_err(e)?;
        let items = collect_rows(rows)?;
        Ok(PaginatedAIResponses { items, total })
    }

    pub fn get_questions_recursive(conn: &Connection, folder_id: i64) -> DbResult<Vec<AIResponse>> {
        if folder_id == 0 {
            let mut stmt = conn
                .prepare(&format!(
                    "SELECT {SELECT_AI_RESPONSE_COLS} FROM AIResponses ar INNER JOIN Folders f ON ar.FolderId = f.Id
                     WHERE ar.FolderId = 0 ORDER BY ar.CreateTime DESC"
                ))
                .map_err(e)?;
            let rows = stmt.query_map([], map_ai_response_row).map_err(e)?;
            return collect_rows(rows);
        }
        let mut stmt = conn
            .prepare(&format!(
                "{FOLDER_TREE_CTE} SELECT {SELECT_AI_RESPONSE_COLS} FROM AIResponses ar
                 INNER JOIN folder_tree ft ON ar.FolderId = ft.Id
                 INNER JOIN Folders f ON ar.FolderId = f.Id
                 ORDER BY ar.CreateTime DESC"
            ))
            .map_err(e)?;
        let rows = stmt.query_map([folder_id], map_ai_response_row).map_err(e)?;
        collect_rows(rows)
    }

    pub fn get_pending_correction_questions(conn: &Connection) -> DbResult<Vec<AIResponse>> {
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {SELECT_AI_RESPONSE_COLS} FROM AIResponses ar LEFT JOIN Folders f ON ar.FolderId = f.Id
                 WHERE COALESCE(ar.IsPendingCorrection, 0) = 1 ORDER BY ar.CreateTime DESC"
            ))
            .map_err(e)?;
        let rows = stmt.query_map([], map_ai_response_row).map_err(e)?;
        collect_rows(rows)
    }

    pub fn get_pending_correction_question_count(conn: &Connection) -> DbResult<i64> {
        conn.query_row(
            "SELECT COUNT(*) FROM AIResponses WHERE COALESCE(IsPendingCorrection, 0) = 1",
            [],
            |row| row.get(0),
        )
        .map_err(e)
    }

    pub fn set_question_pending_correction(conn: &Connection, id: i64, pending: bool) -> DbResult<()> {
        let affected = conn
            .execute(
                "UPDATE AIResponses SET IsPendingCorrection = ? WHERE Id = ?",
                rusqlite::params![pending, id],
            )
            .map_err(e)?;
        if affected == 0 {
            return Err("题目不存在".to_string());
        }
        Ok(())
    }

    pub fn get_folder_question_count(conn: &Connection, folder_id: i64) -> DbResult<i64> {
        conn.query_row(
            "SELECT COUNT(*) FROM AIResponses WHERE FolderId = ?",
            [folder_id],
            |row| row.get(0),
        )
        .map_err(e)
    }

    pub fn get_folder_path(conn: &Connection, folder_id: i64) -> DbResult<Vec<FolderPathItem>> {
        let mut stmt = conn
            .prepare(
                "WITH RECURSIVE folder_path AS (
                   SELECT Id as id, Name as name, ParentId, 0 as level FROM Folders WHERE Id = ?
                   UNION ALL
                   SELECT f.Id as id, f.Name as name, f.ParentId, fp.level + 1 as level
                   FROM Folders f INNER JOIN folder_path fp ON f.Id = fp.ParentId
                   WHERE f.Id != fp.id
                 )
                 SELECT id, name FROM folder_path ORDER BY level DESC",
            )
            .map_err(e)?;
        let rows = stmt
            .query_map([folder_id], |row| {
                Ok(FolderPathItem { id: row.get(0)?, name: row.get(1)? })
            })
            .map_err(e)?;
        collect_rows(rows)
    }

    pub fn get_folder_stats(conn: &Connection) -> DbResult<Vec<FolderStat>> {
        let mut stmt = conn
            .prepare(
                "SELECT f.Id, COALESCE(f.Name, '[未分类]'), COUNT(ar.Id) as questionCount
                 FROM Folders f LEFT JOIN AIResponses ar ON f.Id = ar.FolderId
                 GROUP BY f.Id, f.Name ORDER BY questionCount DESC, f.Name",
            )
            .map_err(e)?;
        let rows = stmt
            .query_map([], |row| {
                Ok(FolderStat {
                    folder_id: row.get(0)?,
                    folder_name: row.get(1)?,
                    question_count: row.get(2)?,
                })
            })
            .map_err(e)?;
        collect_rows(rows)
    }

    /// 智能归类：目标文件夹已有子文件夹时，放入其「[未分类]」子文件夹
    fn get_target_folder_id(conn: &Connection, parent_folder_id: i64) -> rusqlite::Result<i64> {
        if parent_folder_id == 0 {
            return Ok(0);
        }
        let sub_folders_exist: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM Folders WHERE ParentId = ?)",
            [parent_folder_id],
            |row| row.get(0),
        )?;
        if !sub_folders_exist {
            return Ok(parent_folder_id);
        }
        let uncategorized_id: Option<i64> = conn
            .query_row(
                "SELECT Id FROM Folders WHERE ParentId = ? AND Name = '[未分类]'",
                [parent_folder_id],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(id) = uncategorized_id {
            Ok(id)
        } else {
            conn.execute(
                "INSERT INTO Folders (Name, ParentId, CreateTime) VALUES (?, ?, datetime('now'))",
                rusqlite::params!["[未分类]", parent_folder_id],
            )?;
            Ok(conn.last_insert_rowid())
        }
    }

    /// 解析配置的保存文件夹（不存在则回退默认文件夹）
    fn resolve_save_folder_id(conn: &Connection, configured: Option<i64>) -> i64 {
        let configured = configured.unwrap_or(0);
        if configured <= 0 {
            return 0;
        }
        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM Folders WHERE Id = ?)",
                [configured],
                |row| row.get(0),
            )
            .unwrap_or(false);
        if !exists {
            tracing::warn!("配置的题目保存文件夹不存在，已回退到默认文件夹: {}", configured);
            return 0;
        }
        match get_target_folder_id(conn, configured) {
            Ok(id) => id,
            Err(err) => {
                tracing::warn!("解析题目保存文件夹失败，已回退到默认文件夹: {}", err);
                0
            }
        }
    }

    fn fetch_ai_response(conn: &Connection, id: i64) -> DbResult<AIResponse> {
        conn.query_row(
            &format!(
                "SELECT {SELECT_AI_RESPONSE_COLS} FROM AIResponses ar LEFT JOIN Folders f ON ar.FolderId = f.Id WHERE ar.Id = ?"
            ),
            [id],
            map_ai_response_row,
        )
        .map_err(e)
    }

    pub fn add_question(conn: &Connection, q: &NewQuestion) -> DbResult<AIResponse> {
        let answer = q.answer.clone().unwrap_or_default();
        if q.is_ai && answer.trim().is_empty() {
            return Err("AI处理结果答案为空，不保存题目".to_string());
        }
        let target_folder_id = get_target_folder_id(conn, q.folder_id).map_err(e)?;
        conn.execute(
            "INSERT INTO AIResponses (Question, Options, Answer, QuestionType, FolderId, IsAi, CreateTime)
             VALUES (?, ?, ?, ?, ?, ?, datetime('now'))",
            rusqlite::params![q.content, q.options, answer, q.question_type, target_folder_id, q.is_ai],
        )
        .map_err(e)?;
        fetch_ai_response(conn, conn.last_insert_rowid())
    }

    /// 单事务批量导入
    pub fn add_questions_bulk(conn: &Connection, items: &[NewQuestion]) -> DbResult<Vec<i64>> {
        conn.execute_batch("BEGIN").map_err(e)?;
        let mut ids = Vec::with_capacity(items.len());
        let result: DbResult<()> = (|| {
            for q in items {
                let target_folder_id = get_target_folder_id(conn, q.folder_id).map_err(e)?;
                conn.execute(
                    "INSERT INTO AIResponses (Question, Options, Answer, QuestionType, FolderId, IsAi, CreateTime)
                     VALUES (?, ?, ?, ?, ?, ?, datetime('now'))",
                    rusqlite::params![
                        q.content,
                        q.options,
                        q.answer.clone().unwrap_or_default(),
                        q.question_type,
                        target_folder_id,
                        q.is_ai
                    ],
                )
                .map_err(e)?;
                ids.push(conn.last_insert_rowid());
            }
            Ok(())
        })();
        match result {
            Ok(()) => {
                conn.execute_batch("COMMIT").map_err(e)?;
                Ok(ids)
            }
            Err(err) => {
                let _ = conn.execute_batch("ROLLBACK");
                Err(err)
            }
        }
    }

    /// 更新题目（编辑后清除待修正标记，与上游一致）；提供 folder_id 且变化时同时移动
    pub fn update_question(conn: &Connection, id: i64, q: &UpdateQuestion) -> DbResult<()> {
        let affected = conn
            .execute(
                "UPDATE AIResponses SET Question = ?, Options = ?, Answer = ?, QuestionType = ?, IsPendingCorrection = 0 WHERE Id = ?",
                rusqlite::params![
                    q.content,
                    q.options,
                    q.answer.clone().unwrap_or_default(),
                    q.question_type,
                    id
                ],
            )
            .map_err(e)?;
        if affected == 0 {
            return Err("题目不存在".to_string());
        }
        if let Some(target) = q.folder_id.filter(|f| *f >= 0) {
            let current: i64 = conn
                .query_row("SELECT FolderId FROM AIResponses WHERE Id = ?", [id], |r| r.get(0))
                .map_err(e)?;
            if target != current {
                move_question(conn, id, target)?;
            }
        }
        Ok(())
    }

    pub fn move_question(conn: &Connection, question_id: i64, target_folder_id: i64) -> DbResult<()> {
        let actual = get_target_folder_id(conn, target_folder_id).map_err(e)?;
        conn.execute(
            "UPDATE AIResponses SET FolderId = ? WHERE Id = ?",
            rusqlite::params![actual, question_id],
        )
        .map_err(e)?;
        Ok(())
    }

    pub fn copy_question(conn: &Connection, question_id: i64, target_folder_id: i64) -> DbResult<()> {
        let actual = get_target_folder_id(conn, target_folder_id).map_err(e)?;
        let (q, o, a, qt, ia, ipc): (String, Option<String>, String, Option<String>, bool, bool) = conn
            .query_row(
                "SELECT Question, Options, Answer, QuestionType, IsAi, COALESCE(IsPendingCorrection, 0) FROM AIResponses WHERE Id = ?",
                [question_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
            )
            .map_err(e)?;
        conn.execute(
            "INSERT INTO AIResponses (Question, Options, Answer, QuestionType, FolderId, IsAi, IsPendingCorrection, CreateTime)
             VALUES (?, ?, ?, ?, ?, ?, ?, datetime('now'))",
            rusqlite::params![q, o, a, qt, actual, ia, ipc],
        )
        .map_err(e)?;
        Ok(())
    }

    pub fn delete_question(conn: &Connection, id: i64) -> DbResult<()> {
        conn.execute("DELETE FROM AIResponses WHERE Id = ?", [id]).map_err(e)?;
        Ok(())
    }

    pub fn delete_questions(conn: &Connection, ids: &[i64]) -> DbResult<()> {
        conn.execute_batch("BEGIN").map_err(e)?;
        for id in ids {
            if let Err(err) = conn.execute("DELETE FROM AIResponses WHERE Id = ?", [id]) {
                let _ = conn.execute_batch("ROLLBACK");
                return Err(e(err));
            }
        }
        conn.execute_batch("COMMIT").map_err(e)
    }

    /// 收集某文件夹及其真实子树（排除 Id=0 自环）
    fn collect_folder_subtree_ids(conn: &Connection, id: i64) -> DbResult<Vec<i64>> {
        if id == 0 {
            return Ok(vec![0]);
        }
        let mut stmt = conn
            .prepare(&format!("{FOLDER_TREE_CTE} SELECT Id FROM folder_tree"))
            .map_err(e)?;
        let rows = stmt.query_map([id], |row| row.get::<_, i64>(0)).map_err(e)?;
        let mut ids = collect_rows(rows)?;
        if ids.is_empty() {
            ids.push(id);
        }
        Ok(ids)
    }

    pub fn clear_folder_questions(conn: &Connection, id: i64) -> DbResult<()> {
        for fid in collect_folder_subtree_ids(conn, id)? {
            conn.execute("DELETE FROM AIResponses WHERE FolderId = ?", [fid]).map_err(e)?;
        }
        Ok(())
    }

    pub fn delete_folder(conn: &Connection, id: i64, delete_questions: bool) -> DbResult<()> {
        if id == 0 {
            return Err("默认文件夹不能被删除".to_string());
        }
        let folder_ids = collect_folder_subtree_ids(conn, id)?;
        if delete_questions {
            for fid in &folder_ids {
                conn.execute("DELETE FROM AIResponses WHERE FolderId = ?", [fid]).map_err(e)?;
            }
        } else {
            // 题目移到父文件夹（或默认文件夹）
            let parent_id: i64 = conn
                .query_row("SELECT ParentId FROM Folders WHERE Id = ?", [id], |row| row.get(0))
                .unwrap_or(0);
            for fid in &folder_ids {
                conn.execute(
                    "UPDATE AIResponses SET FolderId = ? WHERE FolderId = ?",
                    [parent_id, *fid],
                )
                .map_err(e)?;
            }
        }
        for fid in &folder_ids {
            conn.execute("DELETE FROM Folders WHERE Id = ?", [fid]).map_err(e)?;
        }
        Ok(())
    }

    pub fn rename_folder(conn: &Connection, id: i64, new_name: &str) -> DbResult<()> {
        let name = new_name.trim();
        if name.is_empty() {
            return Err("文件夹名称不能为空".to_string());
        }
        conn.execute("UPDATE Folders SET Name = ? WHERE Id = ?", rusqlite::params![name, id])
            .map_err(e)?;
        Ok(())
    }

    /// 移动文件夹；沿 ParentId 上溯检测祖先环
    pub fn move_folder(conn: &Connection, id: i64, parent_id: i64) -> DbResult<()> {
        if id == parent_id {
            return Err("不能把文件夹移动到自身".to_string());
        }
        if id == 0 {
            return Err("默认文件夹不能移动".to_string());
        }
        let mut cursor = parent_id;
        let mut guard = 0;
        while cursor != 0 && guard < 10_000 {
            if cursor == id {
                return Err("不能把文件夹移动到自己的子文件夹中".to_string());
            }
            let next: Option<i64> = conn
                .query_row("SELECT ParentId FROM Folders WHERE Id = ?", [cursor], |r| r.get(0))
                .optional()
                .map_err(e)?;
            match next {
                Some(p) if p != cursor => cursor = p,
                _ => break,
            }
            guard += 1;
        }
        conn.execute("UPDATE Folders SET ParentId = ? WHERE Id = ?", [parent_id, id])
            .map_err(e)?;
        Ok(())
    }

    pub fn add_folder(conn: &Connection, name: &str, parent_id: i64) -> DbResult<i64> {
        let name = name.trim();
        if name.is_empty() {
            return Err("文件夹名称不能为空".to_string());
        }
        conn.execute(
            "INSERT INTO Folders (Name, ParentId, CreateTime) VALUES (?, ?, datetime('now'))",
            rusqlite::params![name, parent_id],
        )
        .map_err(e)?;
        Ok(conn.last_insert_rowid())
    }

    /// 关键词（空格分隔，需全部命中题干/答案/选项）模糊搜索
    pub fn search_questions_fuzzy(
        conn: &Connection,
        keyword: &str,
        folder_id: Option<i64>,
    ) -> DbResult<Vec<AIResponse>> {
        let candidates = match folder_id {
            Some(0) => get_questions_recursive(conn, 0)?,
            Some(fid) => get_questions_recursive(conn, fid)?,
            None => get_ai_responses(conn, None)?,
        };
        let keyword_lower = keyword.to_lowercase();
        let terms: Vec<&str> = keyword_lower.split_whitespace().collect();
        let mut results: Vec<AIResponse> = if terms.is_empty() {
            candidates
        } else {
            candidates
                .into_iter()
                .filter(|item| {
                    let q = item.question.to_lowercase();
                    let a = item.answer.clone().unwrap_or_default().to_lowercase();
                    let o = item.options.clone().unwrap_or_default().to_lowercase();
                    terms.iter().all(|t| q.contains(t) || a.contains(t) || o.contains(t))
                })
                .collect()
        };
        results.sort_by(|a, b| b.create_time.cmp(&a.create_time));
        Ok(results)
    }

    /// 全表扫描评分，按分降序
    pub fn scan_question_matches(
        conn: &Connection,
        title: &str,
        options: Option<&str>,
    ) -> DbResult<Vec<QuestionMatch>> {
        let query_options = normalize_optional_query_text(options);
        let require_option_match = should_require_option_match(title);
        let mut stmt = conn
            .prepare("SELECT Id, Question, Options, Answer, IsAi, COALESCE(IsPendingCorrection, 0) FROM AIResponses")
            .map_err(e)?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, bool>(4)?,
                    row.get::<_, bool>(5)?,
                ))
            })
            .map_err(e)?;
        let mut results = Vec::new();
        for row in rows {
            let (id, question, db_options, answer, is_ai, is_pending) = row.map_err(e)?;
            if let Some(m) = score_question_row(
                title,
                &query_options,
                require_option_match,
                id,
                question,
                db_options,
                answer,
                is_ai,
                is_pending,
            ) {
                results.push(m);
            }
        }
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        Ok(results)
    }

    pub fn get_ai_response_by_id(conn: &Connection, id: i64) -> DbResult<QuestionMatch> {
        conn.query_row(
            "SELECT Id, Question, Options, Answer, IsAi, COALESCE(IsPendingCorrection, 0) FROM AIResponses WHERE Id = ?",
            [id],
            |row| {
                Ok(QuestionMatch {
                    id: row.get(0)?,
                    question: row.get(1)?,
                    options: row.get(2)?,
                    answer: row.get(3)?,
                    is_ai: row.get(4)?,
                    is_pending_correction: row.get(5)?,
                    score: 1.0,
                })
            },
        )
        .map_err(e)
    }

    pub fn insert_ai_response(
        conn: &Connection,
        question: &str,
        answer: &str,
        options: Option<String>,
        question_type: Option<String>,
        is_ai: bool,
        save_folder_id: Option<i64>,
    ) -> DbResult<i64> {
        if answer.trim().is_empty() {
            return Err("AI处理结果答案为空，不保存题目".to_string());
        }
        let folder_id = resolve_save_folder_id(conn, save_folder_id);
        let folder_name: String = conn
            .query_row("SELECT Name FROM Folders WHERE Id = ?", [folder_id], |row| row.get(0))
            .unwrap_or_else(|_| "默认文件夹".to_string());
        conn.execute(
            "INSERT INTO AIResponses (Question, Answer, Options, QuestionType, IsAi, IsPendingCorrection, CreateTime, FolderId, FolderName)
             VALUES (?, ?, ?, ?, ?, 0, datetime('now'), ?, ?)",
            rusqlite::params![question, answer, options, question_type, is_ai, folder_id, folder_name],
        )
        .map_err(e)?;
        Ok(conn.last_insert_rowid())
    }

    // ----- 请求日志 -----

    fn map_request_log_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PersistedRequestLog> {
        let headers_raw: Option<String> = row.get(8)?;
        Ok(PersistedRequestLog {
            id: row.get(0)?,
            timestamp: row.get(1)?,
            method: row.get(2)?,
            path: row.get(3)?,
            status: row.get(4)?,
            response_time: row.get(5)?,
            request_body: row.get(6)?,
            response_body: row.get(7)?,
            headers: headers_raw.and_then(|v| serde_json::from_str(&v).ok()),
            ip: row.get(9)?,
            user_agent: row.get(10)?,
            stage: row.get(11)?,
        })
    }

    pub fn insert_request_log(conn: &Connection, log: &PersistedRequestLog, max_logs: usize) -> DbResult<()> {
        let headers = log
            .headers
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(e)?;
        conn.execute(
            "INSERT INTO RequestLogs (RequestId, Timestamp, Method, Path, Status, ResponseTime, RequestBody, ResponseBody, Headers, Ip, UserAgent, Stage)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                log.id,
                log.timestamp,
                log.method,
                log.path,
                log.status,
                log.response_time,
                log.request_body,
                log.response_body,
                headers,
                log.ip,
                log.user_agent,
                log.stage,
            ],
        )
        .map_err(e)?;
        if max_logs > 0 {
            conn.execute(
                "DELETE FROM RequestLogs WHERE LogId NOT IN (SELECT LogId FROM RequestLogs ORDER BY LogId DESC LIMIT ?)",
                [max_logs as i64],
            )
            .map_err(e)?;
        }
        Ok(())
    }

    pub fn load_request_logs(conn: &Connection, page: u32, page_size: u32) -> DbResult<(Vec<PersistedRequestLog>, i64)> {
        let page = page.max(1) as i64;
        let page_size = (page_size as i64).clamp(1, 500);
        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM RequestLogs", [], |r| r.get(0))
            .map_err(e)?;
        let mut stmt = conn
            .prepare(
                "SELECT RequestId, Timestamp, Method, Path, Status, ResponseTime, RequestBody, ResponseBody, Headers, Ip, UserAgent, Stage
                 FROM RequestLogs ORDER BY LogId DESC LIMIT ? OFFSET ?",
            )
            .map_err(e)?;
        let rows = stmt
            .query_map(rusqlite::params![page_size, (page - 1) * page_size], map_request_log_row)
            .map_err(e)?;
        Ok((collect_rows(rows)?, total))
    }

    pub fn clear_request_logs(conn: &Connection) -> DbResult<()> {
        conn.execute("DELETE FROM RequestLogs", []).map_err(e)?;
        Ok(())
    }

    /// 今天的 /query 请求数 +1
    pub fn increment_daily_request_count(conn: &Connection) -> DbResult<()> {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        conn.execute(
            "INSERT INTO DailyRequestCounts (Day, Count) VALUES (?, 1)
             ON CONFLICT(Day) DO UPDATE SET Count = Count + 1",
            [&today],
        )
        .map_err(e)?;
        Ok(())
    }

    /// 最近 365 天的按日计数
    pub fn get_daily_request_counts(conn: &Connection) -> DbResult<Vec<(String, i64)>> {
        let mut stmt = conn
            .prepare(
                "SELECT Day, Count FROM DailyRequestCounts WHERE Day >= DATE('now', '-364 days') ORDER BY Day ASC",
            )
            .map_err(e)?;
        let rows = stmt
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))
            .map_err(e)?;
        collect_rows(rows)
    }
}

// ---------------------------------------------------------------------------
// Schema / 迁移（与上游一致，追加索引）
// ---------------------------------------------------------------------------

fn get_table_columns(conn: &Connection, table_name: &str) -> DbResult<HashSet<String>> {
    let pragma = format!("PRAGMA table_info('{table_name}')");
    let mut stmt = conn.prepare(&pragma).map_err(e)?;
    let cols = stmt.query_map([], |row| row.get::<_, String>(1)).map_err(e)?;
    let mut names = HashSet::new();
    for col in cols {
        names.insert(col.map_err(e)?);
    }
    Ok(names)
}

fn ensure_column(
    conn: &Connection,
    table_columns: &mut HashSet<String>,
    column_name: &str,
    alter_sql: &str,
    backfill_sqls: &[&str],
) -> DbResult<()> {
    if table_columns.contains(column_name) {
        return Ok(());
    }
    conn.execute(alter_sql, []).map_err(e)?;
    for sql in backfill_sqls {
        conn.execute(sql, []).map_err(e)?;
    }
    table_columns.insert(column_name.to_string());
    Ok(())
}

pub fn init_database_schema(conn: &Connection) -> DbResult<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS Folders (
          Id INTEGER PRIMARY KEY AUTOINCREMENT,
          Name TEXT NOT NULL,
          ParentId INTEGER DEFAULT 0,
          CreateTime DATETIME DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )
    .map_err(e)?;

    let mut folder_columns = get_table_columns(conn, "Folders")?;
    ensure_column(
        conn,
        &mut folder_columns,
        "ParentId",
        "ALTER TABLE Folders ADD COLUMN ParentId INTEGER DEFAULT 0",
        &["UPDATE Folders SET ParentId = 0 WHERE ParentId IS NULL"],
    )?;
    ensure_column(
        conn,
        &mut folder_columns,
        "CreateTime",
        "ALTER TABLE Folders ADD COLUMN CreateTime DATETIME",
        &["UPDATE Folders SET CreateTime = datetime('now') WHERE CreateTime IS NULL"],
    )?;

    let exists_default: i64 = conn
        .query_row("SELECT COUNT(1) FROM Folders WHERE Id = 0", [], |row| row.get(0))
        .unwrap_or(0);
    if exists_default == 0 {
        let _ = conn.execute(
            "INSERT INTO Folders (Id, Name, ParentId) VALUES (0, '默认文件夹', 0)",
            [],
        );
    }

    conn.execute(
        "CREATE TABLE IF NOT EXISTS AIResponses (
          Id INTEGER PRIMARY KEY AUTOINCREMENT,
          Question TEXT NOT NULL,
          Options TEXT,
          QuestionType TEXT,
          Answer TEXT NOT NULL,
          CreateTime DATETIME DEFAULT CURRENT_TIMESTAMP,
          FolderId INTEGER DEFAULT 0,
          FolderName TEXT DEFAULT '默认文件夹',
          IsAi BOOLEAN DEFAULT 1,
          IsPendingCorrection BOOLEAN DEFAULT 0
        )",
        [],
    )
    .map_err(e)?;

    let mut cols = get_table_columns(conn, "AIResponses")?;
    let had_folder_name = cols.contains("FolderName");
    ensure_column(conn, &mut cols, "QuestionType", "ALTER TABLE AIResponses ADD COLUMN QuestionType TEXT", &[])?;
    ensure_column(
        conn,
        &mut cols,
        "CreateTime",
        "ALTER TABLE AIResponses ADD COLUMN CreateTime DATETIME",
        &["UPDATE AIResponses SET CreateTime = datetime('now') WHERE CreateTime IS NULL"],
    )?;
    ensure_column(
        conn,
        &mut cols,
        "FolderId",
        "ALTER TABLE AIResponses ADD COLUMN FolderId INTEGER DEFAULT 0",
        &["UPDATE AIResponses SET FolderId = 0 WHERE FolderId IS NULL"],
    )?;
    if had_folder_name {
        conn.execute(
            "UPDATE AIResponses
             SET FolderId = COALESCE(
               (SELECT Id FROM Folders WHERE Folders.Name = AIResponses.FolderName
                ORDER BY CASE WHEN Id = 0 THEN 0 ELSE 1 END, Id LIMIT 1),
               0)
             WHERE FolderId IS NULL OR FolderId = 0",
            [],
        )
        .map_err(e)?;
    }
    ensure_column(
        conn,
        &mut cols,
        "FolderName",
        "ALTER TABLE AIResponses ADD COLUMN FolderName TEXT DEFAULT '默认文件夹'",
        &["UPDATE AIResponses SET FolderName = '默认文件夹' WHERE FolderName IS NULL OR trim(FolderName) = ''"],
    )?;
    conn.execute(
        "UPDATE AIResponses
         SET FolderName = COALESCE((SELECT Name FROM Folders WHERE Folders.Id = AIResponses.FolderId), '默认文件夹')
         WHERE FolderName IS NULL OR trim(FolderName) = ''",
        [],
    )
    .map_err(e)?;
    ensure_column(
        conn,
        &mut cols,
        "IsAi",
        "ALTER TABLE AIResponses ADD COLUMN IsAi BOOLEAN DEFAULT 1",
        &["UPDATE AIResponses SET IsAi = 1 WHERE IsAi IS NULL"],
    )?;
    ensure_column(
        conn,
        &mut cols,
        "IsPendingCorrection",
        "ALTER TABLE AIResponses ADD COLUMN IsPendingCorrection BOOLEAN DEFAULT 0",
        &["UPDATE AIResponses SET IsPendingCorrection = 0 WHERE IsPendingCorrection IS NULL"],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS RequestLogs (
          LogId INTEGER PRIMARY KEY AUTOINCREMENT,
          RequestId TEXT NOT NULL,
          Timestamp TEXT NOT NULL,
          Method TEXT NOT NULL,
          Path TEXT NOT NULL,
          Status INTEGER,
          ResponseTime INTEGER,
          RequestBody TEXT,
          ResponseBody TEXT,
          Headers TEXT,
          Ip TEXT,
          UserAgent TEXT,
          Stage TEXT NOT NULL
        )",
        [],
    )
    .map_err(e)?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_request_logs_request_id ON RequestLogs(RequestId)",
        [],
    )
    .map_err(e)?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS DailyRequestCounts (
          Day TEXT PRIMARY KEY,
          Count INTEGER NOT NULL DEFAULT 0
        )",
        [],
    )
    .map_err(e)?;

    // VPS 版新增索引
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_airesponses_folder ON AIResponses(FolderId);
         CREATE INDEX IF NOT EXISTS idx_airesponses_pending ON AIResponses(IsPendingCorrection);
         CREATE INDEX IF NOT EXISTS idx_airesponses_createtime ON AIResponses(CreateTime);",
    )
    .map_err(e)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(content: &str, answer: &str, folder_id: i64) -> NewQuestion {
        NewQuestion {
            content: content.into(),
            answer: Some(answer.into()),
            folder_id,
            ..Default::default()
        }
    }

    #[test]
    fn schema_and_crud_roundtrip() {
        let db = Db::open_memory();
        db.run_sync(|c| {
            let fid = ops::add_folder(c, "课程A", 0)?;
            let child = ops::add_folder(c, "第一章", fid)?;
            // 目标文件夹已有子文件夹 → 归入 [未分类]
            let added = ops::add_question(c, &q("题目1", "答案1", fid))?;
            assert_ne!(added.folder_id, fid);
            assert_eq!(added.folder_name.as_deref(), Some("[未分类]"));
            let direct = ops::add_question(c, &q("题目2", "答案2", child))?;
            assert_eq!(direct.folder_id, child);

            let page = ops::get_paginated_questions(c, Some(fid), true, false, 1, 20, true)?;
            assert_eq!(page.total, 2);
            let flat = ops::get_paginated_questions(c, Some(child), false, false, 1, 20, true)?;
            assert_eq!(flat.total, 1);

            // 祖先环检测
            assert!(ops::move_folder(c, fid, child).is_err());
            assert!(ops::move_folder(c, child, 0).is_ok());

            // 删除文件夹不删题：题目回到父级
            ops::delete_folder(c, child, false)?;
            let root = ops::get_paginated_questions(c, Some(0), false, false, 1, 20, true)?;
            assert_eq!(root.total, 1);

            // 更新：不带 folderId 时不移动；带 folderId 时移动
            let moved_id = ops::add_question(c, &q("题目3", "答案3", 0))?.id;
            ops::update_question(c, moved_id, &UpdateQuestion { content: "题目3改".into(), answer: Some("A".into()), ..Default::default() })?;
            let still: i64 = c.query_row("SELECT FolderId FROM AIResponses WHERE Id = ?", [moved_id], |r| r.get(0)).map_err(e)?;
            assert_eq!(still, 0);
            ops::update_question(c, moved_id, &UpdateQuestion { content: "题目3改".into(), folder_id: Some(fid), ..Default::default() })?;
            let moved: i64 = c.query_row("SELECT FolderId FROM AIResponses WHERE Id = ?", [moved_id], |r| r.get(0)).map_err(e)?;
            assert_ne!(moved, 0);

            // 精确匹配
            let id = ops::insert_ai_response(c, "韩国的首都在哪里", "首尔", None, None, true, None)?;
            let m = ops::scan_question_matches(c, "韩国的首都在哪里", None)?;
            assert_eq!(m[0].id, id);
            assert!(is_exact_question_match("韩国的首都在哪里", None, &m[0]));
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn migrates_legacy_database_schema() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE Folders (Id INTEGER PRIMARY KEY AUTOINCREMENT, Name TEXT NOT NULL)", []).unwrap();
        conn.execute(
            "CREATE TABLE AIResponses (Id INTEGER PRIMARY KEY AUTOINCREMENT, Question TEXT NOT NULL, Options TEXT, Answer TEXT NOT NULL, FolderName TEXT DEFAULT '默认文件夹')",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO Folders (Id, Name) VALUES (0, '默认文件夹')", []).unwrap();
        conn.execute("INSERT INTO AIResponses (Question, Options, Answer, FolderName) VALUES ('题目', NULL, '答案', '默认文件夹')", []).unwrap();

        init_database_schema(&conn).unwrap();

        let cols = get_table_columns(&conn, "AIResponses").unwrap();
        for c in ["QuestionType", "CreateTime", "FolderId", "FolderName", "IsAi", "IsPendingCorrection"] {
            assert!(cols.contains(c), "缺少列 {c}");
        }
        let (folder_id, is_ai, pending): (i64, i64, i64) = conn
            .query_row("SELECT FolderId, IsAi, IsPendingCorrection FROM AIResponses LIMIT 1", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .unwrap();
        assert_eq!((folder_id, is_ai, pending), (0, 1, 0));
    }
}
