# server 模块契约（实现代理必须遵守）

已由主编写完成、**不要修改签名**：`main.rs`、`config.rs`、`state.rs`、`ai/mod.rs`、`web.rs`、`Cargo.toml`（可追加依赖，不可删除）。
代码注释、日志、错误消息使用简体中文（与上游一致）。不要使用 emoji。禁止 `unwrap()` 于运行时可失败路径（解析常量正则除外）。

## db.rs

```rust
#[derive(Clone)]
pub struct Db(Arc<parking_lot::Mutex<rusqlite::Connection>>);
impl Db {
    pub fn open(path: &Path) -> anyhow::Result<Db>;          // WAL、busy_timeout、init_database_schema + 新索引
    pub async fn run<T: Send + 'static>(&self, f: impl FnOnce(&rusqlite::Connection) -> Result<T, String> + Send + 'static) -> Result<T, String>; // spawn_blocking
}
// DTO（与上游 database.rs 同名同字段，serde 输出 snake_case）
pub struct AIResponse { id, question, options: Option<String>, answer: Option<String>, question_type: Option<String>, folder_id: i64, folder_name: Option<String>, create_time: Option<String>, is_ai: bool, is_pending_correction: bool }
pub struct Folder { id, name, parent_id: i64, create_time: Option<String> }
pub struct FolderStat { folder_id, folder_name, question_count }
pub struct FolderPathItem { id, name }
pub struct PaginatedAIResponses { items: Vec<AIResponse>, total: i64 }
pub struct QuestionMatch { id, question, options: Option<String>, answer, is_ai, is_pending_correction, score: f64 }
pub struct NewQuestion { content, options: Option<String>, answer: Option<String>, question_type: Option<String>, folder_id: i64, is_ai: bool }  // camelCase Deserialize
pub struct PersistedRequestLog { ... 与 logger::RequestLog 对应 }
// 方法（均 async，返回 Result<_, String>），语义逐一移植上游 database.rs 同名函数：
get_folders, get_paginated_questions(folder_id: Option<i64>, recursive: bool, pending_only: bool, page: u32, page_size: u32, sort_desc: bool),
get_questions_recursive(folder_id), get_pending_correction_questions, get_pending_correction_question_count, set_question_pending_correction(id, pending),
get_folder_question_count(folder_id), get_folder_path(folder_id), get_folder_stats,
add_question(NewQuestion, save_folder_id_from_settings: Option<i64>) -> i64（folder_id 缺省逻辑 get_target_folder_id / [未分类] 规则保留）,
add_questions_bulk(Vec<NewQuestion>) -> Vec<i64>（单事务）, update_question(id, NewQuestion), move_question, copy_question, delete_question, delete_questions(ids),
clear_folder_questions(id), delete_folder(id, delete_questions), rename_folder(id, name), move_folder(id, parent_id)（新增祖先环检测）, add_folder(name, parent_id) -> i64,
search_questions_fuzzy(keyword_segmented: &str, folder_id: Option<i64>) -> Vec<AIResponse>,
query_exact(title, options) -> Vec<QuestionMatch>, query_candidates(title, options, limit) -> Vec<QuestionMatch>, get_ai_response_by_id(id) -> QuestionMatch,
insert_ai_response(question, answer, options, qtype, is_ai, save_folder_id: Option<i64>) -> i64,
insert_request_log(&PersistedRequestLog, max_logs), load_request_logs(page, page_size) -> (Vec<PersistedRequestLog>, i64), clear_request_logs,
increment_daily_request_count, get_daily_request_counts -> Vec<(String, i64)>
```
上游 `get_configured_save_folder_id` 读 config.json 的地方改为由调用方传入 `save_folder_id`（来自 `AppSettings.question_save_folder_id`）。

## matching.rs
从上游 database.rs 抽出纯函数：`extract_urls`, `compute_query_match_score(a,b) -> Option<f64>`, `min_keyword_coverage`, `score_question_row`, `is_exact_match_score`, `is_exact_question_match`, `QUERY_STOPWORDS`, 全局 `JIEBA: Lazy<Jieba>`，`segment_text(&str) -> Vec<String>`（cut_for_search，去空白）。保留上游单元测试。

## answer.rs
`extract_answer_from_json(&str) -> String`（七级回退）、`normalize_answer_against_options(answer, options: Option<&str>) -> String`、`parse_option_letter_map`、`strip_leading_option_label`、`is_model_error(&str) -> Option<String>`、`classify_model_failure(&str) -> Option<(u16, String)>`、`is_timeout_like_model_failure`、`strip_markdown_code_block`、`get_most_frequent_answer(&[String]) -> Option<String>`（移植 Home.vue getMostFrequentSuccessfulAnswer）。保留上游测试。

## prompt.rs
`QuestionKind` + `detect_question_kind`、`build_model_query_prompt(title, options, qtype) -> String`、`build_answer_messages(query: &str) -> Vec<ai::ChatMessage>`（few-shot，移植 answerFewShot.ts）、`build_same_question_check_prompt(title, options, &[QuestionMatch]) -> String`（Home.vue 3307-3327 文案）、`parse_same_question_result(&str) -> Option<i64>`、`build_summary_prompt(query, combined) -> String`（Home.vue 2776）、URL 题：`UrlQuestionMode`、`detect_url_question_mode`、`parse_url_options -> Vec<(String,String)>`、`build_url_question_prompt(title, options, qtype) -> String`、`resolve_url_answer(raw, options) -> String`（移植 urlQuestion.ts 与 Home.vue 4086-4281）。`contains_url(&str) -> bool`。

## ai/protocol.rs
```rust
pub fn resolve_endpoint(base_url: &str, protocol: ApiProtocol) -> String; // 处理 /v1 重复
pub fn build_request(cfg: &ModelCallConfig, input: &ModelCallInput) -> Result<(String /*url*/, reqwest::header::HeaderMap, serde_json::Value), ModelCallError>;
pub async fn call_model_stream(http: &reqwest::Client, cfg: &ModelCallConfig, input: &ModelCallInput, timeout: Duration)
    -> Result<Pin<Box<dyn Stream<Item = Result<ModelChunk, ModelCallError>> + Send>>, ModelCallError>;
pub async fn call_model(http, cfg, input, timeout, on_chunk: Option<Box<dyn FnMut(&ModelChunk) + Send>>) -> Result<ModelCallOutput, ModelCallError>; // 聚合
pub async fn probe_models(http, base_url, api_key, custom_headers) -> Result<Vec<String>, ModelCallError>; // /models 与 /v1/models
```
测试：三种协议的 body 构造（thinking on/off）、SSE 分帧解析。

## ai/image.rs
`pub async fn fetch_image_as_data_url(http, cache_dir: &Path, url: &str) -> Result<String, String>`（SSRF 防护：解析主机，拒绝私网/环回/链路本地；≤8MB；15s；磁盘缓存 sha256(url).b64；UA/Referer 策略移植 commands.rs 91-173）；`pub fn preprocess_for_vision(data_url: &str, min_side: u32) -> Result<String, String>`（透明→白底、短边 <min_side 时等比放大，image crate）；`pub fn extract_image_urls(text: &str) -> Vec<String>`。

## ai/engine.rs
```rust
pub struct EngineCtx<'a> { pub state: &'a AppState, pub request_id: &'a str }  // 通过 state.logger 广播进度
pub struct ModelResult { pub model_id: String, pub model_name: String, pub platform_name: String, pub content: String, pub reasoning_content: String, pub error: Option<String>, pub elapsed_ms: u64 }
pub struct AnswerOutcome { pub content: String, pub reasoning_content: String, pub per_model: Vec<ModelResult>, pub used_summary: bool }
#[derive(thiserror::Error)] pub enum EngineError { NoModelSelected, NoVisionModel, AllModelsFailed(String), Timeout, Other(String) }
impl EngineError { pub fn http_status(&self) -> u16 }  // NoModelSelected→400, Timeout→408, 其余 500
pub async fn answer_question(ctx, query: &str) -> Result<AnswerOutcome, EngineError>;
pub async fn check_same_question(ctx, prompt: &str) -> Result<String, EngineError>;
pub async fn answer_url_question(ctx, title, options: Option<&str>, qtype: Option<&str>) -> Result<AnswerOutcome, EngineError>;
pub async fn test_model(state, model: &ResolvedModel, messages: Vec<ChatMessage>, on_chunk) -> Result<ModelCallOutput, ModelCallError>;
```
进度广播：`state.logger.send_model_call_request(request_id, query)`、`send_model_call_progress(request_id, content)`（≥800ms 节流）、`send_model_call_response(request_id, content, reasoning, is_success)`。

## logger.rs
移植上游 `RequestLog`、`ModelCallRequest/Progress/Response`、`SSEEvent`（tag="type"，新增 `OcsHead { timestamp }` rename "ocs_head"）。
```rust
#[derive(Clone)] pub struct RequestLogger { ... }
impl RequestLogger {
    pub fn new(db: Db, max_memory_logs: usize) -> Self;
    pub fn subscribe(&self) -> broadcast::Receiver<SSEEvent>;
    pub fn log_request_start(&self, method, path, ip, ua, headers, body) -> String /*id*/;  // 同时 increment_daily_request_count（仅 /query）
    pub fn log_request_complete(&self, id, status, response_time_ms, response_body);  // 持久化到 RequestLogs（max 2000）
    pub fn send_model_call_request / send_model_call_progress / send_model_call_response / send_ocs_head
    pub fn recent(&self) -> Vec<RequestLog>;
}
```
去掉 pending_responses / wait_for_model_response 机制。

## auth.rs
```rust
pub fn constant_time_eq(a: &str, b: &str) -> bool;                       // subtle
pub fn extract_bearer(headers: &HeaderMap) -> Option<String>;
pub fn is_admin_token(settings: &AppSettings, token: &str) -> bool;
pub enum QueryAuth { Admin, User(String /*name*/), Anonymous }
pub fn check_query_token(settings: &AppSettings, token: Option<&str>) -> Option<QueryAuth>;  // None=拒绝
pub async fn require_admin(State(state), req: Request, next: Next) -> Response;             // axum middleware：Bearer 或 ?token=
pub struct AdminUser;  // FromRequestParts extractor（可选）
```

## routes/
`mod.rs`: `pub fn build_router(state: AppState) -> axum::Router`：CORS（any origin，允许 content-type/authorization/x-token，方法 GET/POST/PUT/PATCH/DELETE/OPTIONS/HEAD），`TraceLayer`，公开路由 + `/api/login` + `/api/admin` 嵌套（`require_admin` 中间件）+ `fallback(web::serve)`。
`public.rs`: 见 DESIGN §6.1；`/query` 处理流程严格移植 server.rs `resolve_query_with_same_question_check` + `wait_and_store_ai_answer`，只是把 emit/wait 换成 `ai::engine` 调用。响应 HTTP 状态：成功/未命中 200、401 未授权、400/408/500 按 EngineError。对外 URL 由 `ZERROR_PUBLIC_URL` 或 `X-Forwarded-Proto/Host` 推断。待修正按钮 URL 带 `?token=`。
`admin.rs`: DESIGN §6.2 全部端点；统一错误体 `{success:false, message}` + 合理状态码。
`sse.rs`: `GET /api/admin/logs/stream`，`axum::response::Sse`，事件名 = SSEEvent 的 tag 值（`request_log` 事件名用 `log`，其余同名），keep-alive 15s。
