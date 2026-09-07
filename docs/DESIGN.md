# ZError Server 改造设计（VPS 部署 + 网页管理 + 公网题库）

> 本文是所有实现工作的契约。后端、前端、部署脚本都以此为准；与此文冲突的实现视为错误。

## 1. 目标与边界

- 源项目：Tauri 2 桌面应用（Vue 3 + Rust warp）。桌面版中 **AI 调用发生在前端 WebView**，Rust 通过 `model-call-request` 事件把 prompt 抛给前端并阻塞等待 `/api/model/response` 回调。
- 目标：单个 Rust 二进制（`zerror-server`）在 Linux VPS 上常驻运行，提供：
  1. 公网 OCS 题库接口（`/query`，需访问令牌）；
  2. 浏览器管理后台（嵌入式静态资源，管理员令牌登录）；
  3. 服务端 AI 引擎（三种内置协议：`openai-chat` / `openai-response` / `anthropic`，放弃 `custom` JS 协议）。
- 自用场景：单管理员，无多租户，无注册。
- 删除的桌面特性：托盘、自动更新、窗口控制、单实例/UAC、二级窗口、Tauri 事件总线、本地文件对话框、`config.json` 与可执行文件同目录约定。

## 2. 仓库布局

```
ZError/
├── server/                 Rust crate zerror-server（axum 0.7 + tokio + reqwest + rusqlite）
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs         入口：读取环境变量、初始化数据目录/DB、构建 Router、监听
│       ├── config.rs       运行时配置（env）+ AppSettings / ModelSettings 持久化（JSON 文件）
│       ├── state.rs        AppState（Db、Settings、Logger、AiEngine、HttpClient）
│       ├── db.rs           SQLite：schema/迁移 + 题库/文件夹 CRUD + 请求日志 + 每日计数（移植 database.rs）
│       ├── matching.rs     题目匹配评分（jieba + Levenshtein，移植自 database.rs 13-250, 1360-1587）
│       ├── answer.rs       答案抽取 extract_answer_from_json、规范化 normalize_answer_against_options、错误分类（移植 server.rs 1384-1746）
│       ├── prompt.rs       题型识别、答题 prompt、few-shot 消息、同题判重 prompt、总结 prompt、URL 题 prompt（移植 server.rs 156-362 + answerFewShot.ts + Home.vue 3307-3327 + urlQuestion.ts）
│       ├── ai/
│       │   ├── mod.rs      公共类型：ChatMessage、ContentPart、ModelChunk、ModelCallError、trait 入口 call_model_stream
│       │   ├── protocol.rs 三种协议的请求构造 + SSE/非流式响应归一化（移植 modelProtocol.ts 74-695）
│       │   ├── engine.rs   多模型并发 / 重试 / 超时 / 总结 / 多数投票；同题判重；URL 图片题（移植 Home.vue 2017-2258, 2587-2930, 3295-3437, 4086-4281）
│       │   └── image.rs    图片抓取（UA/Referer 策略、磁盘缓存、SSRF 防护）+ 白底/放大预处理（移植 commands.rs 91-173 + Home.vue 3799-3956）
│       ├── logger.rs       请求日志环形缓冲 + broadcast SSE + 持久化（移植 logger.rs，去掉 pending_responses 机制）
│       ├── auth.rs         管理员令牌 / 查询令牌校验（axum extractor + middleware）
│       ├── routes/
│       │   ├── mod.rs      Router 组装、CORS、静态资源 fallback
│       │   ├── public.rs   HEAD /、GET /api/status、GET|POST /query、POST /api/questions/{id}/pending-correction
│       │   ├── admin.rs    /api/login 与 /api/admin/** 全部管理接口
│       │   └── sse.rs      GET /api/admin/logs/stream
│       └── web.rs          rust-embed 嵌入 ../web/dist
├── web/                    Vue 3 + Vite 管理后台（从上游 src/ 移植，去 Tauri）
├── deploy/
│   ├── Dockerfile          多阶段：node 构建 web → rust 构建 server → debian-slim 运行
│   ├── docker-compose.yml  zerror + caddy（自动 HTTPS）
│   ├── Caddyfile
│   ├── zerror.service      systemd unit
│   └── install.sh          原生安装脚本（下载/构建二进制、建用户、装 unit）
├── docs/
│   ├── DESIGN.md           本文
│   └── API.md              REST 接口详细说明（由本文 §6 生成）
└── README.md
```

## 3. 运行时配置（环境变量）

| 变量 | 默认 | 说明 |
|---|---|---|
| `ZERROR_DATA_DIR` | `./data` | 数据目录：`airesponses.db`、`settings.json`、`model_config.json`、`image_cache/` |
| `ZERROR_BIND` | `0.0.0.0:3000` | 监听地址（`SocketAddr`，支持 IPv6） |
| `ZERROR_ADMIN_TOKEN` | 无 | 首次启动时若 `settings.json` 中 `adminToken` 为空，则用此值；仍为空则随机生成并打印到日志且写入 `settings.json` |
| `ZERROR_PUBLIC_URL` | 无 | 对外访问的根 URL（如 `https://qa.example.com`）；为空时由 `X-Forwarded-Proto` / `X-Forwarded-Host` / `Host` 推断 |
| `ZERROR_LOG` | `info` | `tracing` 过滤器 |
| `ZERROR_TRUST_PROXY` | `true` | 是否信任 `X-Forwarded-For` 作为客户端 IP |

## 4. 持久化

### 4.1 SQLite `airesponses.db`

与桌面版完全同构，**可以直接把桌面版的 `airesponses.db` 拷贝到数据目录复用**。`init_database_schema` 的迁移逻辑原样保留。新增索引：

```sql
CREATE INDEX IF NOT EXISTS idx_airesponses_folder ON AIResponses(FolderId);
CREATE INDEX IF NOT EXISTS idx_airesponses_pending ON AIResponses(IsPendingCorrection);
CREATE INDEX IF NOT EXISTS idx_airesponses_createtime ON AIResponses(CreateTime);
```

启用 `PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;`。连接模型：`Arc<parking_lot::Mutex<rusqlite::Connection>>`，所有 DB 调用在 `tokio::task::spawn_blocking` 中执行。

表：`Folders`、`AIResponses`、`RequestLogs`（本版真正写入，保留最近 2000 条）、`DailyRequestCounts`。

### 4.2 `settings.json`（AppSettings）

保留上游 `AppSettings` 的字段名（前端 `settings.ts` 基本不变），删除桌面字段：`network`、`windowSize`、`windowPosition`、`autoUpdate`、`enableNotifications`、`algorithms`、`questionSaveDir`。新增：

```jsonc
{
  "theme": "light", "language": "zh-CN", "autoSave": true,
  "autoAddToQuestionBank": true,
  "modelResponseTimeout": 40,      // 秒，clamp 5..600
  "modelRetryCount": 2,            // max 10
  "defaultDifficulty": "medium", "itemsPerPage": 20, "showExplanation": true,
  "suppressNoModelWarning": false,
  "questionSaveFolderId": null,
  "adminToken": "…",               // 管理员令牌（登录 + Bearer）
  "publicQueryRequireToken": true, // /query 是否必须带令牌
  "multiUser": {                    // 查询令牌：每个 user.token 都可访问 /query
    "enabled": true,
    "users": [{ "id": "u1", "name": "我的OCS", "token": "…", "createdAt": "…" }]
  }
}
```

### 4.3 `model_config.json`（ModelSettings）

与上游 `ModelSettings` 形状一致（`selectedTextModels`、`selectedSummaryModels`、`selectedVisionModel`、`platforms[]`、`globalSettings`）。差异：
- `AIModel.apiProtocol` 只允许 `openai-chat` | `openai-response` | `anthropic`；读到 `custom` 一律按 `openai-chat` 处理并记录 warn；`jsCode` 字段忽略。
- API key 只存服务端；管理接口 GET 时**原样返回**（自用、管理员令牌保护、HTTPS），不做掩码（KISS）。
- 远程目录同步（`webapi.zaizhexue.top/live/models.json`）：服务端提供 `GET /api/admin/remote-catalog` 代理返回原始 JSON（规避 CORS），合并规则仍由前端 `modelConfig.ts` 执行后 PUT 写回。

## 5. 鉴权

- **管理员**：`POST /api/login {token}`，服务端与 `settings.adminToken` 常量时间比较，成功返回 `{success:true, role:"admin", name:"管理员"}`。之后所有 `/api/admin/**` 与 `/api/admin/logs/stream` 需 `Authorization: Bearer <adminToken>`；SSE 因 `EventSource` 无法设头，允许 `?token=` 查询参数。
- **查询令牌**（公网 `/query` 与 `/api/questions/{id}/pending-correction`）：从 `?token=`、`X-Token` 头或 `Authorization: Bearer` 取值，匹配 `adminToken` 或 `multiUser.users[].token`（`multiUser.enabled` 必须为 true 才接受用户令牌）。`publicQueryRequireToken=false` 时放行匿名。未授权返回 HTTP 401 `{code:0,data:null,message:"未授权"}`。
- 待修正按钮 HTML 中的 URL 带上本次请求使用的 token。

## 6. HTTP 接口

所有 JSON 字段名：请求体 camelCase，响应体沿用上游 snake_case DTO（`folder_id`、`is_ai`…），以便前端 `database.ts` 类型不变。

### 6.1 公开（OCS）

| 方法/路径 | 说明 |
|---|---|
| `HEAD /` | `200 text/plain "Hello,OCS"`；记录 `last_ocs_contact_at` |
| `GET /api/status` | `{status:"running", version, uptime_secs, last_ocs_contact_at}` |
| `GET /query?title&options&type[&token]` / `POST /query` JSON `{title, options?, type?}` | **契约不变**：`{code:1, data:{id, question, answer, is_ai, is_pending_correction}}` 或 `{code:0, data:null, message}`；HTTP 状态：成功/未命中 200，未授权 401，模型失败 500/408，未选择模型 400 |
| `POST /api/questions/{id}/pending-correction?token=` | `{success, message, id}` |

`/query` 处理流程与上游 `resolve_query_with_same_question_check` 一致：精确命中 → 模糊候选（Top 5）+ AI 同题判重 → 正常 AI 答题；`autoAddToQuestionBank` 控制入库；`question` 字段含 HTML 待修正按钮。

### 6.2 管理（Bearer adminToken）

前缀 `/api/admin`。

**设置**
- `GET /settings` → AppSettings；`PUT /settings` body 全量 AppSettings → 保存后返回。
- `GET /model-settings` / `PUT /model-settings` → ModelSettings。
- `GET /remote-catalog` → 代理返回远程模型目录原始 JSON。
- `POST /models/test` body `{platformId, modelId, prompt?, imageDataUrl?, stream?:true}` → `text/event-stream`，事件 `chunk {content, reasoning_content}`、`done {content, reasoning_content, usage?, elapsed_ms}`、`error {message}`。
- `POST /platforms/probe-models` body `{baseUrl, apiKey, customHeaders?}` → `{models:[string]}`（依次尝试 `/models`、`/v1/models`）。

**文件夹**
- `GET /folders` → `Folder[]`；`GET /folders/stats` → `FolderStat[]`；`GET /folders/{id}/path` → `FolderPathItem[]`；`GET /folders/{id}/count` → `{count}`。
- `POST /folders {name, parentId}` → `{id}`；`PATCH /folders/{id} {name?, parentId?}`；`DELETE /folders/{id}?deleteQuestions=bool`；`POST /folders/{id}/clear`。

**题目**
- `GET /questions?folderId&recursive=bool&pendingOnly=bool&page&pageSize&sortOrder=asc|desc` → `PaginatedAIResponses {items, total}`。
- `GET /questions/search?q&folderId` → `{items: AIResponse[], terms: string[]}`（服务端 jieba 分词后模糊搜索，`terms` 供高亮）。
- `GET /questions/pending-count` → `{count}`。
- `POST /questions {content, options?, answer?, questionType?, folderId, isAi?}` → `{id}`。
- `POST /questions/bulk {items:[同上]}` → `{inserted, ids}`。
- `PATCH /questions/{id} {content, options?, answer?, questionType?, folderId}`（清除待修正标记，与上游 update_question 一致）。
- `DELETE /questions/{id}`；`POST /questions/batch-delete {ids}`。
- `POST /questions/{id}/move {targetFolderId}`；`POST /questions/{id}/copy {targetFolderId}`；`POST /questions/{id}/pending {pending:bool}`。
- `GET /questions/export?folderId&recursive` → `AIResponse[]`（前端自行生成 csv/xlsx/docx/pdf）。

**日志与统计**
- `GET /logs?page&pageSize` → `{items: RequestLog[], total}`；`GET /logs/recent` → 内存最近日志；`DELETE /logs`。
- `GET /logs/stream[?token=]` → SSE，事件名与上游一致：`log`、`model_call_request`、`model_call_progress`、`model_call_response`，data 为扁平 `SSEEvent` JSON（含 `type` 字段）。新增事件 `ocs_head`。
- `GET /stats/daily` → `[[day, count]]`（365 天）。

**其它**
- `GET /image?url=` → `{dataUrl}`（图片代理 + 磁盘缓存；拒绝私网/环回地址；≤ 8 MB；15 s 超时）。
- `GET /segment?text=` → `string[]`。
- `GET /version` → `{version, build}`。

### 6.3 静态资源

其余 GET 路径返回嵌入的 `web/dist`；无匹配文件时回退 `index.html`（SPA）。`GET /` 在有 `Accept: text/html` 时返回后台首页（不再返回 `query_test_page.html`）。

## 7. AI 引擎（服务端）

### 7.1 协议层 `ai/protocol.rs`

输入：`ModelCallInput { messages: Vec<ChatMessage>, stream: bool }`、`ModelCallConfig { base_url, api_key, custom_headers, model_id, temperature, top_p, max_tokens, protocol, thinking: ThinkingConfig }`。输出：`impl Stream<Item = Result<ModelChunk, ModelCallError>>`，`ModelChunk { content: String, reasoning_content: String, usage: Option<Value> }`（增量）。

严格移植 `modelProtocol.ts` 的请求规则：

| 协议 | URL | 头 | body 要点 |
|---|---|---|---|
| openai-chat | `{baseUrl}/v1/chat/completions`（baseUrl 已以 `/v1` 结尾则不重复） | `Authorization: Bearer` | `model, messages, stream, temperature, top_p, max_tokens`；thinking on → `reasoning_effort`；off → `enable_thinking:false`（默认）和/或 `thinking:{type:"disabled"}` |
| openai-response | `{baseUrl}/v1/responses` | 同上 | `model, input[{role, content:[{type:"input_text"|"input_image"}]}], stream, reasoning:{effort}`；off 时 effort = `thinkingOffResponsesEffort`（`none`/`minimal`） |
| anthropic | `{baseUrl}/v1/messages` | `anthropic-version: 2023-06-01`、`x-api-key` **和** `Authorization: Bearer`（兼容中转） | `model, max_tokens, system, messages(user/assistant 折叠), stream, temperature`；thinking on → `thinking:{type:"enabled", budget_tokens=clamp(max_tokens/2,1024,16000)}` 且 temperature=1；off → 不发 thinking 字段 |

`custom_headers` 最后合并覆盖。SSE 解析：按空行分帧、`data:` 行、忽略 `[DONE]`；chat 取 `choices[0].delta.content|reasoning_content|reasoning`；responses 取 `response.output_text.delta` / `*reasoning*delta`；anthropic 取 `content_block_delta.text_delta|thinking_delta`。非流式时一次性归一化。HTTP 非 2xx → `ModelCallError::Http { status, body }`，消息格式 `HTTP <status>: <error.message|message|body>`。

### 7.2 编排 `ai/engine.rs`

- `answer_question(ctx, request) -> Result<AnswerOutcome, EngineError>`：
  1. 选中文本模型 `selectedTextModels`（≤5，过滤 enabled）；为空 → `EngineError::NoModelSelected`（HTTP 400，消息含「未选择模型」）。
  2. 每个模型并发调用；`messages = prompt::build_answer_messages(query)`（few-shot）；单模型或视觉模型时按 `modelRetryCount` 重试；超时 `modelResponseTimeout` 秒（单次）。
  3. 进度：每个模型每 ≥800 ms 广播一次 `model_call_progress`。
  4. 有总结模型且 ≥1 成功 → 总结 prompt（上游文案）→ 总结输出；否则多数投票（对 `extract_answer_from_json` 后的答案做空白/代码块归一后计数，平局取首个成功）。
  5. 返回 `{content, reasoning_content, per_model: Vec<ModelResult>}`，由路由层做 `extract_answer_from_json` + `normalize_answer_against_options` + 入库。
- `check_same_question(ctx, title, options, candidates) -> Option<i64>`：用第一个可用文本模型，prompt 见 `prompt::build_same_question_check_prompt`，超时 30 s，解析 `{same, matched_id}`。
- `answer_url_question(ctx, title, options, qtype)`：提取图片 URL → `image::fetch_as_data_url` → 白底/放大 → 视觉模型（`selectedVisionModel`，无则 `EngineError::NoVisionModel`）多模态调用 → 解析末行 `ANSWER:` → `prompt::resolve_url_answer` 映射到选项正文。
- 整个 `/query` 处理放在 `tokio::spawn` 中，不受客户端断开影响；绝对上限 `model_wait_budget_secs` 逻辑保留。

## 8. 前端（web/）

- 保留：布局/主题/题库管理（FileTree、QuestionList、QuestionDetail、QuestionEditor、各对话框）、设置页（General、Model、About）、首页请求日志面板与详情、OCS 配置对话框、模型选择对话框、导入/导出（改用 `<input type=file>` / Blob 下载）。
- 删除：AppHeader 窗口控制与拖拽区、更新相关（useAppUpdate、UpdateDialog、versionCheck、updateDownload）、environmentDetector、二级窗口逻辑、Tauri 事件总线（改 `mitt`）、`main.ts` 的 `__TAURI_INTERNALS__` 等待、全局自定义右键菜单中的 Tauri 剪贴板（改 `navigator.clipboard`）、Home.vue 中整个 AI 引擎（callModelAPI、callModelWithStreaming、同题判重、URL 题分析、心跳）。
- 新增：`src/services/api.ts`（fetch 封装，自动附带 Bearer，401 跳登录）、`src/views/Login.vue`、路由状态（无 vue-router，沿用现有 Sidebar 切换）；`services/database.ts`、`settings.ts`、`modelConfig.ts` 改为 REST 客户端但**保持导出签名**，视图层改动最小。
- Home.vue 改为「运行台」：服务状态卡片（`/api/status`）、OCS 配置（url 用 `ZERROR_PUBLIC_URL` 或 `location.origin`，带查询令牌）、模型选择（写回 `/api/admin/model-settings`）、请求日志实时表（SSE）+ 详情（含 `model_call_progress/response` 渲染）、测试查询按钮（调用 `/query`）。
- 模型测试对话框：改为消费 `POST /api/admin/models/test` 的 SSE。
- 构建：`pnpm build` → `web/dist`，被 `server` 通过 `rust-embed` 嵌入。

## 9. 部署

- Docker：`deploy/Dockerfile`（多阶段）；`docker-compose.yml` 含 `zerror`（卷 `./data:/data`，`ZERROR_DATA_DIR=/data`）与 `caddy`（`Caddyfile` 反代 `zerror:3000`，域名通过 `.env` 的 `DOMAIN`）。
- systemd：`install.sh` 创建 `zerror` 用户、`/opt/zerror/zerror-server`、`/var/lib/zerror`，安装 `zerror.service`（`EnvironmentFile=/etc/zerror/env`），HTTPS 由用户自配 Caddy/Nginx（README 给出 Caddy 示例）。

## 10. 非目标 / 已知取舍

- 不支持 `custom` JS 协议模型（用户已确认）。
- 管理接口不做 API key 掩码（自用）。
- 不做多管理员 / RBAC。
- `.doc` 转换、二级窗口、远程 questionImageAlgorithm 注入一律删除。
