//! 管理接口（/api/login 与 /api/admin/**）。

use std::convert::Infallible;
use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::Json;
use futures::Stream;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::ai::{engine, image, protocol, ChatMessage, ContentPart};
use crate::auth::is_admin_token;
use crate::config::{AppSettings, ModelSettings};
use crate::db::{NewQuestion, UpdateQuestion};
use crate::logger::RequestLog;
use crate::matching::segment_text;
use crate::state::AppState;

pub const REMOTE_CATALOG_URL: &str = "https://webapi.zaizhexue.top/live/models.json";

type ApiResult = Result<Response, ApiError>;

/// 统一错误体 {success:false, message}
pub struct ApiError(StatusCode, String);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"success": false, "message": self.1}))).into_response()
    }
}

fn bad_request(msg: impl Into<String>) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, msg.into())
}

fn db_err(msg: String) -> ApiError {
    let status = if msg.contains("不存在") { StatusCode::NOT_FOUND } else { StatusCode::INTERNAL_SERVER_ERROR };
    ApiError(status, msg)
}

fn ok_json<T: serde::Serialize>(v: T) -> ApiResult {
    Ok(Json(v).into_response())
}

fn ok_success() -> ApiResult {
    ok_json(json!({"success": true}))
}

// ---------------------------------------------------------------------------
// 登录
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct LoginBody {
    #[serde(default)]
    token: String,
}

pub async fn login(State(state): State<AppState>, Json(body): Json<LoginBody>) -> Json<Value> {
    let token = body.token.trim();
    if token.is_empty() {
        return Json(json!({"success": false, "message": "token不能为空"}));
    }
    let settings = state.settings();
    if is_admin_token(&settings, token) {
        return Json(json!({"success": true, "role": "admin", "name": "管理员"}));
    }
    if settings.multi_user.enabled {
        if let Some(u) = settings.multi_user.users.iter().find(|u| crate::auth::constant_time_eq(u.token.trim(), token)) {
            return Json(json!({"success": true, "role": "user", "name": u.name}));
        }
    }
    Json(json!({"success": false, "message": "token无效"}))
}

// ---------------------------------------------------------------------------
// 设置
// ---------------------------------------------------------------------------

pub async fn get_settings(State(state): State<AppState>) -> Json<AppSettings> {
    Json(state.settings())
}

pub async fn put_settings(State(state): State<AppState>, Json(mut new): Json<AppSettings>) -> ApiResult {
    // 管理员令牌不允许被清空
    if new.admin_token.trim().is_empty() {
        new.admin_token = state.settings().admin_token;
    }
    new.model_response_timeout = new.model_response_timeout.clamp(5, 600);
    new.model_retry_count = new.model_retry_count.min(10);
    let saved = state.save_settings(new).map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    ok_json(saved)
}

pub async fn get_model_settings(State(state): State<AppState>) -> Json<ModelSettings> {
    Json(state.model_settings())
}

pub async fn put_model_settings(State(state): State<AppState>, Json(new): Json<ModelSettings>) -> ApiResult {
    let saved = state.save_model_settings(new).map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    ok_json(saved)
}

/// 代理拉取远程模型目录（合并规则由前端 modelConfig.ts 负责）
pub async fn remote_catalog(State(state): State<AppState>) -> ApiResult {
    let resp = state
        .http
        .get(REMOTE_CATALOG_URL)
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .map_err(|e| ApiError(StatusCode::BAD_GATEWAY, format!("拉取远程目录失败: {e}")))?;
    if !resp.status().is_success() {
        return Err(ApiError(StatusCode::BAD_GATEWAY, format!("远程目录返回 HTTP {}", resp.status())));
    }
    let v: Value = resp.json().await.map_err(|e| ApiError(StatusCode::BAD_GATEWAY, format!("远程目录不是合法 JSON: {e}")))?;
    ok_json(v)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeBody {
    #[serde(default)]
    base_url: String,
    #[serde(default)]
    api_key: String,
    #[serde(default)]
    custom_headers: Option<std::collections::HashMap<String, String>>,
}

pub async fn probe_models(State(state): State<AppState>, Json(body): Json<ProbeBody>) -> ApiResult {
    let models = protocol::probe_models(&state.http, &body.base_url, &body.api_key, &body.custom_headers.unwrap_or_default())
        .await
        .map_err(|e| ApiError(StatusCode::BAD_GATEWAY, e.to_string()))?;
    ok_json(json!({"models": models}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestModelBody {
    model_id: String,
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default)]
    image_data_url: Option<String>,
    #[serde(default)]
    messages: Option<Vec<ChatMessage>>,
}

/// 模型测试：SSE 流（chunk / done / error）
pub async fn test_model(State(state): State<AppState>, Json(body): Json<TestModelBody>) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let ms = state.model_settings();
    let model = ms.find_model(&body.model_id).ok_or_else(|| bad_request("模型不存在"))?;
    let messages = if let Some(m) = body.messages.filter(|m| !m.is_empty()) {
        m
    } else {
        let prompt = body.prompt.filter(|p| !p.trim().is_empty()).unwrap_or_else(|| "你好，请简单介绍一下你自己。".into());
        match body.image_data_url.filter(|u| u.starts_with("data:image/")) {
            Some(url) => vec![ChatMessage::user_parts(vec![
                ContentPart::Text { text: prompt },
                ContentPart::ImageUrl { url, detail: Some("high".into()) },
            ])],
            None => vec![ChatMessage::user(prompt)],
        }
    };

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Event>();
    let state2 = state.clone();
    tokio::spawn(async move {
        let tx_cb = tx.clone();
        let on_progress: Box<dyn FnMut(&str, &str) + Send> = Box::new(move |content: &str, reasoning: &str| {
            let _ = tx_cb.send(Event::default().event("chunk").data(json!({"content": content, "reasoning_content": reasoning}).to_string()));
        });
        match engine::test_model(&state2, &model, messages, on_progress).await {
            Ok(out) => {
                let _ = tx.send(Event::default().event("done").data(
                    json!({"content": out.content, "reasoning_content": out.reasoning_content, "usage": out.usage, "elapsed_ms": out.elapsed_ms}).to_string(),
                ));
            }
            Err(e) => {
                let _ = tx.send(Event::default().event("error").data(json!({"message": e.to_string()}).to_string()));
            }
        }
    });
    let stream = tokio_stream::wrappers::UnboundedReceiverStream::new(rx).map(Ok::<_, Infallible>);
    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(10))))
}

use tokio_stream::StreamExt as _;

// ---------------------------------------------------------------------------
// 文件夹
// ---------------------------------------------------------------------------

pub async fn folders(State(state): State<AppState>) -> ApiResult {
    ok_json(state.db.get_folders().await.map_err(db_err)?)
}

pub async fn folder_stats(State(state): State<AppState>) -> ApiResult {
    ok_json(state.db.get_folder_stats().await.map_err(db_err)?)
}

pub async fn folder_path(State(state): State<AppState>, Path(id): Path<i64>) -> ApiResult {
    ok_json(state.db.get_folder_path(id).await.map_err(db_err)?)
}

pub async fn folder_count(State(state): State<AppState>, Path(id): Path<i64>) -> ApiResult {
    ok_json(json!({"count": state.db.get_folder_question_count(id).await.map_err(db_err)?}))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderBody {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    parent_id: Option<i64>,
}

pub async fn add_folder(State(state): State<AppState>, Json(body): Json<FolderBody>) -> ApiResult {
    let name = body.name.unwrap_or_default();
    let id = state.db.add_folder(name, body.parent_id.unwrap_or(0)).await.map_err(|e| bad_request(e))?;
    ok_json(json!({"id": id}))
}

pub async fn patch_folder(State(state): State<AppState>, Path(id): Path<i64>, Json(body): Json<FolderBody>) -> ApiResult {
    if let Some(name) = body.name {
        state.db.rename_folder(id, name).await.map_err(|e| bad_request(e))?;
    }
    if let Some(parent) = body.parent_id {
        state.db.move_folder(id, parent).await.map_err(|e| bad_request(e))?;
    }
    ok_success()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteFolderQuery {
    #[serde(default)]
    delete_questions: bool,
}

pub async fn delete_folder(State(state): State<AppState>, Path(id): Path<i64>, Query(q): Query<DeleteFolderQuery>) -> ApiResult {
    state.db.delete_folder(id, q.delete_questions).await.map_err(|e| bad_request(e))?;
    ok_success()
}

pub async fn clear_folder(State(state): State<AppState>, Path(id): Path<i64>) -> ApiResult {
    state.db.clear_folder_questions(id).await.map_err(db_err)?;
    ok_success()
}

// ---------------------------------------------------------------------------
// 题目
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListQuestionsQuery {
    #[serde(default)]
    folder_id: Option<i64>,
    #[serde(default)]
    recursive: bool,
    #[serde(default)]
    pending_only: bool,
    #[serde(default = "one")]
    page: u32,
    #[serde(default = "twenty")]
    page_size: u32,
    #[serde(default)]
    sort_order: Option<String>,
}
fn one() -> u32 {
    1
}
fn twenty() -> u32 {
    20
}

pub async fn list_questions(State(state): State<AppState>, Query(q): Query<ListQuestionsQuery>) -> ApiResult {
    let sort_desc = !matches!(q.sort_order.as_deref(), Some("asc"));
    ok_json(
        state
            .db
            .get_paginated_questions(q.folder_id, q.recursive, q.pending_only, q.page, q.page_size, sort_desc)
            .await
            .map_err(db_err)?,
    )
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    #[serde(default)]
    q: String,
    #[serde(default)]
    folder_id: Option<i64>,
}

pub async fn search_questions(State(state): State<AppState>, Query(q): Query<SearchQuery>) -> ApiResult {
    let terms: Vec<String> = segment_text(&q.q).into_iter().filter(|t| !t.trim().is_empty()).collect();
    let keyword = if terms.is_empty() { q.q.trim().to_string() } else { terms.join(" ") };
    let items = state.db.search_questions_fuzzy(keyword, q.folder_id).await.map_err(db_err)?;
    ok_json(json!({"items": items, "terms": terms}))
}

pub async fn pending_count(State(state): State<AppState>) -> ApiResult {
    ok_json(json!({"count": state.db.get_pending_correction_question_count().await.map_err(db_err)?}))
}

pub async fn add_question(State(state): State<AppState>, Json(body): Json<NewQuestion>) -> ApiResult {
    if body.content.trim().is_empty() {
        return Err(bad_request("题目内容不能为空"));
    }
    let created = state.db.add_question(body).await.map_err(|e| bad_request(e))?;
    ok_json(json!({"id": created.id, "item": created}))
}

#[derive(Deserialize)]
pub struct BulkBody {
    #[serde(default)]
    items: Vec<NewQuestion>,
}

pub async fn add_questions_bulk(State(state): State<AppState>, Json(body): Json<BulkBody>) -> ApiResult {
    let items: Vec<NewQuestion> = body.items.into_iter().filter(|q| !q.content.trim().is_empty()).collect();
    if items.is_empty() {
        return Err(bad_request("没有可导入的题目"));
    }
    let ids = state.db.add_questions_bulk(items).await.map_err(db_err)?;
    ok_json(json!({"inserted": ids.len(), "ids": ids}))
}

pub async fn update_question(State(state): State<AppState>, Path(id): Path<i64>, Json(body): Json<UpdateQuestion>) -> ApiResult {
    if body.content.trim().is_empty() {
        return Err(bad_request("题目内容不能为空"));
    }
    state.db.update_question(id, body).await.map_err(db_err)?;
    ok_success()
}

pub async fn delete_question(State(state): State<AppState>, Path(id): Path<i64>) -> ApiResult {
    state.db.delete_question(id).await.map_err(db_err)?;
    ok_success()
}

#[derive(Deserialize)]
pub struct IdsBody {
    #[serde(default)]
    ids: Vec<i64>,
}

pub async fn batch_delete(State(state): State<AppState>, Json(body): Json<IdsBody>) -> ApiResult {
    state.db.delete_questions(body.ids).await.map_err(db_err)?;
    ok_success()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetFolderBody {
    target_folder_id: i64,
}

pub async fn move_question(State(state): State<AppState>, Path(id): Path<i64>, Json(body): Json<TargetFolderBody>) -> ApiResult {
    state.db.move_question(id, body.target_folder_id).await.map_err(db_err)?;
    ok_success()
}

pub async fn copy_question(State(state): State<AppState>, Path(id): Path<i64>, Json(body): Json<TargetFolderBody>) -> ApiResult {
    state.db.copy_question(id, body.target_folder_id).await.map_err(db_err)?;
    ok_success()
}

#[derive(Deserialize)]
pub struct PendingBody {
    #[serde(default = "default_true")]
    pending: bool,
}
fn default_true() -> bool {
    true
}

pub async fn set_pending(State(state): State<AppState>, Path(id): Path<i64>, Json(body): Json<PendingBody>) -> ApiResult {
    state.db.set_question_pending_correction(id, body.pending).await.map_err(db_err)?;
    ok_success()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportQuery {
    #[serde(default)]
    folder_id: Option<i64>,
    #[serde(default = "default_true")]
    recursive: bool,
    #[serde(default)]
    pending_only: bool,
}

pub async fn export_questions(State(state): State<AppState>, Query(q): Query<ExportQuery>) -> ApiResult {
    let items = if q.pending_only {
        state.db.get_pending_correction_questions().await
    } else {
        match q.folder_id {
            Some(fid) if q.recursive => state.db.get_questions_recursive(fid).await,
            other => state.db.get_ai_responses(other).await,
        }
    }
    .map_err(db_err)?;
    ok_json(items)
}

// ---------------------------------------------------------------------------
// 日志 / 统计
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageQuery {
    #[serde(default = "one")]
    page: u32,
    #[serde(default = "fifty")]
    page_size: u32,
}
fn fifty() -> u32 {
    50
}

pub async fn list_logs(State(state): State<AppState>, Query(q): Query<PageQuery>) -> ApiResult {
    let (items, total) = state.db.load_request_logs(q.page, q.page_size).await.map_err(db_err)?;
    let items: Vec<RequestLog> = items.into_iter().map(RequestLog::from).collect();
    ok_json(json!({"items": items, "total": total}))
}

/// 内存中的最近日志（含进行中的请求）
pub async fn recent_logs(State(state): State<AppState>) -> ApiResult {
    ok_json(state.logger.recent())
}

pub async fn clear_logs(State(state): State<AppState>) -> ApiResult {
    state.db.clear_request_logs().await.map_err(db_err)?;
    state.logger.clear_memory();
    ok_success()
}

pub async fn daily_stats(State(state): State<AppState>) -> ApiResult {
    ok_json(state.db.get_daily_request_counts().await.map_err(db_err)?)
}

// ---------------------------------------------------------------------------
// 其它
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct UrlQuery {
    #[serde(default)]
    url: String,
}

pub async fn image_proxy(State(state): State<AppState>, Query(q): Query<UrlQuery>) -> ApiResult {
    if q.url.trim().is_empty() {
        return Err(bad_request("url 不能为空"));
    }
    let data_url = image::fetch_image_as_data_url(&state.http, &state.runtime.image_cache_dir(), &q.url)
        .await
        .map_err(|e| ApiError(StatusCode::BAD_GATEWAY, e))?;
    ok_json(json!({"dataUrl": data_url}))
}

#[derive(Deserialize)]
pub struct TextQuery {
    #[serde(default)]
    text: String,
}

pub async fn segment(Query(q): Query<TextQuery>) -> Json<Vec<String>> {
    Json(segment_text(&q.text))
}

pub async fn version() -> Json<Value> {
    Json(json!({"version": crate::VERSION, "build": "server"}))
}
