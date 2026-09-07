//! 公开接口：OCS 探测、题库查询、待修正标记、健康检查。
//! /query 流程逐行移植自上游 server.rs resolve_query_with_same_question_check。

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::ai::engine::{self, EngineCtx, EngineError};
use crate::answer::{classify_model_failure, extract_answer_from_json, normalize_answer_against_options};
use crate::auth::{check_query_token, token_from_request};
use crate::db::QuestionMatch;
use crate::matching::contains_url;
use crate::prompt;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// 线协议类型（types.rs）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QueryRequest {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub options: Option<String>,
    #[serde(rename = "type", default)]
    pub query_type: Option<String>,
    /// 查询令牌（GET 时随参数传入；不回显）
    #[serde(default, skip_serializing)]
    pub token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryData {
    pub id: i64,
    pub question: String,
    pub answer: String,
    pub is_ai: bool,
    pub is_pending_correction: bool,
}

/// data 序列化为单个对象（取第一条），兼容 OCS 配置 res.data.question / res.data.answer
#[derive(Debug, Clone)]
pub struct QueryResponse {
    pub code: i32,
    pub data: Option<Vec<QueryData>>,
    pub message: Option<String>,
}

impl Serialize for QueryResponse {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("code", &self.code)?;
        match &self.data {
            Some(list) if !list.is_empty() => map.serialize_entry("data", &list[0])?,
            _ => map.serialize_entry("data", &Option::<QueryData>::None)?,
        }
        if let Some(msg) = &self.message {
            map.serialize_entry("message", msg)?;
        }
        map.end()
    }
}

impl QueryResponse {
    pub fn success(data: Vec<QueryData>) -> Self {
        Self { code: 1, data: Some(data), message: None }
    }
    #[cfg(test)]
    pub fn not_found() -> Self {
        Self { code: 0, data: None, message: Some("No matching records found".into()) }
    }
    pub fn error(message: String) -> Self {
        Self { code: 0, data: None, message: Some(message) }
    }
}

// ---------------------------------------------------------------------------
// 辅助
// ---------------------------------------------------------------------------

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// 对外根 URL：ZERROR_PUBLIC_URL → X-Forwarded-Proto/Host → Host
pub fn resolve_request_origin(state: &AppState, headers: &HeaderMap) -> String {
    if let Some(u) = &state.runtime.public_url {
        return u.clone();
    }
    let hv = |name: &str| headers.get(name).and_then(|v| v.to_str().ok()).map(|s| s.trim()).filter(|s| !s.is_empty());
    let proto = hv("x-forwarded-proto").map(|s| s.split(',').next().unwrap_or("http").trim().to_string()).unwrap_or_else(|| "http".into());
    let host = hv("x-forwarded-host")
        .map(|s| s.split(',').next().unwrap_or("").trim().to_string())
        .or_else(|| hv("host").map(|s| s.to_string()))
        .unwrap_or_else(|| format!("127.0.0.1:{}", state.runtime.bind.port()));
    format!("{proto}://{host}")
}

fn build_pending_correction_button(origin: &str, token: Option<&str>, question_id: i64, is_pending: bool) -> String {
    if question_id <= 0 {
        return String::new();
    }
    if is_pending {
        return "<button type=\"button\" disabled style=\"padding:4px 10px;border:none;border-radius:999px;background:#f59e0b;color:#fff;font-size:12px;cursor:not-allowed;opacity:0.75;white-space:nowrap;\">已标记待修正</button>".to_string();
    }
    let token_q = token.map(|t| format!("?token={}", urlencoding::encode(t))).unwrap_or_default();
    let url = format!("{origin}/api/questions/{question_id}/pending-correction{token_q}");
    format!(
        r#"<button type="button" style="padding:4px 10px;border:none;border-radius:999px;background:#ef4444;color:#fff;font-size:12px;cursor:pointer;white-space:nowrap;" onclick="(async()=>{{const btn=this;if(btn.dataset.loading==='1')return;const text=btn.textContent||'标记为待修正';btn.dataset.loading='1';btn.disabled=true;btn.textContent='标记中...';try{{const res=await fetch('{url}',{{method:'POST'}});const data=await res.json().catch(()=>({{success:false,message:'标记失败'}}));if(!res.ok||!data.success)throw new Error(data.message||'标记失败');btn.textContent='已标记待修正';btn.style.opacity='0.75';btn.style.cursor='not-allowed';}}catch(error){{btn.disabled=false;btn.textContent=text;alert(error&&error.message?error.message:'标记失败');}}finally{{delete btn.dataset.loading;}}}})()">标记为待修正</button>"#
    )
}

fn build_query_data(origin: &str, token: Option<&str>, question_id: i64, question: &str, answer: String, is_ai: bool, is_pending: bool) -> QueryData {
    let escaped = escape_html(question).replace('\n', "<br>");
    let button = build_pending_correction_button(origin, token, question_id, is_pending);
    let question_html = if button.is_empty() {
        escaped
    } else {
        format!(
            "<div style=\"display:flex;align-items:flex-start;gap:8px;flex-wrap:wrap;\"><span style=\"flex:1 1 auto;min-width:0;\">{escaped}</span>{button}</div>"
        )
    };
    QueryData { id: question_id, question: question_html, answer, is_ai, is_pending_correction: is_pending }
}

fn client_ip(state: &AppState, headers: &HeaderMap, addr: Option<SocketAddr>) -> String {
    if state.runtime.trust_proxy {
        if let Some(v) = headers
            .get("x-forwarded-for")
            .or_else(|| headers.get("x-real-ip"))
            .and_then(|v| v.to_str().ok())
        {
            if let Some(first) = v.split(',').next().map(str::trim).filter(|s| !s.is_empty()) {
                return first.to_string();
            }
        }
    }
    addr.map(|a| a.ip().to_string()).unwrap_or_else(|| "127.0.0.1".into())
}

struct QueryContext {
    origin: String,
    token: Option<String>,
}

fn engine_error_response(err: &EngineError) -> (u16, QueryResponse) {
    (err.http_status(), QueryResponse::error(err.to_string()))
}

async fn insert_ai(state: &AppState, request: &QueryRequest, answer: &str, is_ai: bool) -> i64 {
    let settings = state.settings();
    if !settings.auto_add_to_question_bank {
        tracing::info!("autoAddToQuestionBank=false，跳过将 AI 回答写入题库");
        return 0;
    }
    match state
        .db
        .insert_ai_response(
            request.title.clone(),
            answer.to_string(),
            request.options.clone(),
            request.query_type.clone(),
            is_ai,
            settings.question_save_folder_id,
        )
        .await
    {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("AI 回答入库失败: {}", e);
            0
        }
    }
}

/// 正常 AI 答题（wait_and_store_ai_answer）
async fn answer_with_ai(state: &AppState, ctx: &QueryContext, request: &QueryRequest, request_id: &str, has_url: bool) -> (u16, QueryResponse) {
    let ectx = EngineCtx { state, request_id };
    let outcome = if has_url {
        engine::answer_url_question(&ectx, &request.title, request.options.as_deref(), request.query_type.as_deref()).await
    } else {
        let query = prompt::build_model_query_prompt(&request.title, request.options.as_deref(), request.query_type.as_deref());
        engine::answer_question(&ectx, &query).await
    };
    let outcome = match outcome {
        Ok(o) => o,
        Err(e) => return engine_error_response(&e),
    };
    let model_content = outcome.content;
    if let Some((status, msg)) = classify_model_failure(&model_content) {
        return (status, QueryResponse::error(msg));
    }

    let mut extracted = extract_answer_from_json(&model_content);
    if model_content.contains("题目不完整,无法确定具体问题.") {
        extracted = String::new();
    }
    extracted = normalize_answer_against_options(extracted.trim(), request.options.as_deref());

    let inserted_id = if extracted.is_empty() {
        tracing::warn!("AI 最终处理结果答案为空，跳过保存题目");
        0
    } else {
        insert_ai(state, request, &extracted, true).await
    };
    let data = build_query_data(&ctx.origin, ctx.token.as_deref(), inserted_id, &request.title, extracted, true, false);
    (200, QueryResponse::success(vec![data]))
}

/// 三级解析：精确命中 → 候选 + AI 判重 → AI 答题
async fn resolve_query(state: &AppState, ctx: &QueryContext, request: &QueryRequest, request_id: &str) -> (u16, QueryResponse) {
    let has_url = contains_url(&request.title) || request.options.as_deref().map(contains_url).unwrap_or(false);
    let opts = request.options.clone();

    // 1) 精确命中
    match state.db.query_exact(request.title.clone(), opts.clone()).await {
        Ok(hits) if !hits.is_empty() => {
            tracing::info!("精确匹配命中: {} 条", hits.len());
            let data: Vec<QueryData> = hits
                .into_iter()
                .map(|m| {
                    let answer = normalize_answer_against_options(&m.answer, request.options.as_deref());
                    build_query_data(&ctx.origin, ctx.token.as_deref(), m.id, &m.question, answer, m.is_ai, m.is_pending_correction)
                })
                .collect();
            return (200, QueryResponse::success(data));
        }
        Ok(_) => {}
        Err(e) => return (500, QueryResponse::error(format!("Database error: {e}"))),
    }

    // 2) 模糊候选 → AI 同题判断
    let candidates: Vec<QuestionMatch> = match state
        .db
        .query_candidates(request.title.clone(), opts.clone(), prompt::SAME_QUESTION_CANDIDATE_LIMIT)
        .await
    {
        Ok(c) => c,
        Err(e) => return (500, QueryResponse::error(format!("Database error: {e}"))),
    };

    if !candidates.is_empty() {
        tracing::info!("发现 {} 条近似候选，请求 AI 同题判断", candidates.len());
        let check_prompt = prompt::build_same_question_check_prompt(&request.title, request.options.as_deref(), &candidates);
        let ectx = EngineCtx { state, request_id };
        match tokio::time::timeout(Duration::from_secs(45), engine::check_same_question(&ectx, &check_prompt)).await {
            Ok(Ok(judge)) => {
                if let Some(matched_id) = prompt::parse_same_question_result(&judge) {
                    let matched = match candidates.iter().find(|c| c.id == matched_id).cloned() {
                        Some(m) => Some(m),
                        None => state.db.get_ai_response_by_id(matched_id).await.ok(),
                    };
                    if let Some(matched) = matched {
                        let answer = normalize_answer_against_options(&matched.answer, request.options.as_deref());
                        let response_id = if state.settings().auto_add_to_question_bank {
                            let id = insert_ai(state, request, &answer, matched.is_ai).await;
                            if id > 0 { id } else { matched.id }
                        } else {
                            matched.id
                        };
                        let data = build_query_data(&ctx.origin, ctx.token.as_deref(), response_id, &request.title, answer, matched.is_ai, false);
                        return (200, QueryResponse::success(vec![data]));
                    }
                    tracing::warn!("同题判断返回的 matched_id={} 无效，回落正常答题", matched_id);
                } else {
                    tracing::info!("AI 判定为不同题或解析失败，回落正常答题");
                }
            }
            Ok(Err(EngineError::NoModelSelected)) => return engine_error_response(&EngineError::NoModelSelected),
            Ok(Err(e)) => tracing::warn!("同题判断失败: {}，回落正常答题", e),
            Err(_) => tracing::warn!("同题判断超时，回落正常答题"),
        }
    }

    // 3) 正常 AI 答题
    answer_with_ai(state, ctx, request, request_id, has_url).await
}

async fn handle_query(state: AppState, method: &str, headers: HeaderMap, addr: Option<SocketAddr>, query_string: Option<String>, request: QueryRequest) -> Response {
    let token = request.token.clone().or_else(|| token_from_request(&headers, query_string.as_deref()));
    if check_query_token(&state.settings(), token.as_deref()).is_none() {
        return (StatusCode::UNAUTHORIZED, Json(QueryResponse::error("未授权".into()))).into_response();
    }

    let start = Instant::now();
    let header_map: HashMap<String, String> = headers
        .iter()
        .filter(|(k, _)| k.as_str() != "authorization" && k.as_str() != "x-token")
        .filter_map(|(k, v)| v.to_str().ok().map(|s| (k.to_string(), s.to_string())))
        .collect();
    let user_agent = headers.get("user-agent").and_then(|v| v.to_str().ok()).map(|s| s.to_string());
    let ip = client_ip(&state, &headers, addr);
    let request_id = state.logger.log_request_start(
        method,
        "/query",
        Some(ip),
        user_agent,
        Some(header_map),
        Some(serde_json::to_string(&request).unwrap_or_default()),
    );
    let ctx = QueryContext { origin: resolve_request_origin(&state, &headers), token };

    // 独立任务执行：请求端断开时仍会写完 completed 日志与入库
    let (inactivity, absolute) = state.settings().model_wait_budget_secs(contains_url(&request.title));
    let _ = inactivity;
    let state_task = state.clone();
    let request_id_task = request_id.clone();
    let join = tokio::spawn(async move {
        let result = match tokio::time::timeout(Duration::from_secs(absolute), resolve_query(&state_task, &ctx, &request, &request_id_task)).await {
            Ok(r) => r,
            Err(_) => (408, QueryResponse::error(format!("Model call failed: 等待超过 {absolute} 秒"))),
        };
        let elapsed = start.elapsed().as_millis() as u64;
        state_task
            .logger
            .log_request_complete(&request_id_task, result.0, elapsed, Some(serde_json::to_string(&result.1).unwrap_or_default()));
        result
    });

    match join.await {
        Ok((status, body)) => {
            let code = StatusCode::from_u16(status).unwrap_or(StatusCode::OK);
            (code, Json(body)).into_response()
        }
        Err(e) => {
            tracing::error!("/query 任务失败: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(QueryResponse::error("Internal query task failed".into()))).into_response()
        }
    }
}

// ---------------------------------------------------------------------------
// handlers
// ---------------------------------------------------------------------------

pub async fn query_get(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    uri: axum::http::Uri,
    Query(request): Query<QueryRequest>,
) -> Response {
    handle_query(state, "GET", headers, Some(addr), uri.query().map(|s| s.to_string()), request).await
}

pub async fn query_post(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    uri: axum::http::Uri,
    body: Result<Json<QueryRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let request = match body {
        Ok(Json(r)) => r,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(QueryResponse::error(format!("请求体无效: {e}")))).into_response(),
    };
    handle_query(state, "POST", headers, Some(addr), uri.query().map(|s| s.to_string()), request).await
}

/// HEAD / ：OCS 探测
pub async fn head_root(State(state): State<AppState>) -> Response {
    state.touch_ocs_contact();
    state.logger.send_ocs_head();
    ([(axum::http::header::CONTENT_TYPE, "text/plain; charset=utf-8")], "Hello,OCS").into_response()
}

pub async fn status(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(json!({
        "status": "running",
        "message": "Server is running",
        "version": crate::VERSION,
        "uptime_secs": state.started_at.elapsed().as_secs(),
        "last_ocs_contact_at": state.last_ocs_contact(),
    }))
}

pub async fn time() -> Json<serde_json::Value> {
    let now = chrono::Utc::now();
    Json(json!({"timestamp": now.timestamp(), "time": now.to_rfc3339()}))
}

pub async fn echo(Json(body): Json<serde_json::Value>) -> Json<serde_json::Value> {
    Json(json!({"echo": body, "received_at": chrono::Utc::now().to_rfc3339()}))
}

pub async fn mark_pending_correction(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    uri: axum::http::Uri,
) -> Response {
    let token = token_from_request(&headers, uri.query());
    if check_query_token(&state.settings(), token.as_deref()).is_none() {
        return (StatusCode::UNAUTHORIZED, Json(json!({"success": false, "message": "未授权"}))).into_response();
    }
    match state.db.set_question_pending_correction(id, true).await {
        Ok(()) => Json(json!({"success": true, "message": "题目已标记为待修正", "id": id})).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "message": e}))).into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_response_serializes_first_item_as_object() {
        let r = QueryResponse::success(vec![
            QueryData { id: 1, question: "q".into(), answer: "a".into(), is_ai: true, is_pending_correction: false },
            QueryData { id: 2, question: "q2".into(), answer: "b".into(), is_ai: false, is_pending_correction: false },
        ]);
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["code"], 1);
        assert_eq!(v["data"]["id"], 1);
        assert!(v.get("message").is_none());
        let nf = serde_json::to_value(QueryResponse::not_found()).unwrap();
        assert_eq!(nf["code"], 0);
        assert!(nf["data"].is_null());
        assert_eq!(nf["message"], "No matching records found");
    }

    #[test]
    fn query_request_accepts_type_alias() {
        let r: QueryRequest = serde_json::from_str(r#"{"title":"t","type":"single","token":"x"}"#).unwrap();
        assert_eq!(r.query_type.as_deref(), Some("single"));
        let s = serde_json::to_string(&r).unwrap();
        assert!(!s.contains("token"));
    }

    #[test]
    fn query_data_html_contains_button_with_token() {
        let d = build_query_data("https://h", Some("tk"), 5, "a<b\nc", "x".into(), true, false);
        assert!(d.question.contains("a&lt;b<br>c"));
        assert!(d.question.contains("/api/questions/5/pending-correction?token=tk"));
        let no_btn = build_query_data("https://h", None, 0, "q", "x".into(), true, false);
        assert_eq!(no_btn.question, "q");
    }
}
