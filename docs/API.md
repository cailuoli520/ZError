# ZError Server HTTP 接口

所有 JSON 请求体使用 camelCase，响应体沿用上游 snake_case DTO（`folder_id`、`is_ai` …）。

## 鉴权

| 类型 | 用途 | 传递方式 |
|---|---|---|
| 管理员令牌 | `/api/admin/**`、`/api/admin/logs/stream` | `Authorization: Bearer <token>`；SSE 可用 `?token=` |
| 查询令牌 | `/query`、`/api/questions/{id}/pending-correction` | `?token=`、`X-Token` 头或 `Authorization: Bearer` |

查询令牌 = 管理员令牌，或「常规设置 → 公网题库访问」中添加的任意用户令牌（`multiUser.enabled` 必须为 true）。`publicQueryRequireToken=false` 时允许匿名查询。未授权返回 `401 {code:0,data:null,message:"未授权"}`。

## 公开接口（OCS）

### `HEAD /`
OCS 探测。`200 text/plain "Hello,OCS"`，并更新 `last_ocs_contact_at`。

### `GET /api/status`
```json
{"status":"running","message":"Server is running","version":"3.0.0","uptime_secs":12,"last_ocs_contact_at":1788617600}
```

### `GET /query?title&options&type[&token]` / `POST /query[?token]`
POST 体：`{"title":"…","options":"A. …\nB. …","type":"single"}`

响应（契约与桌面版一致，`data` 为单对象）：
```json
{"code":1,"data":{"id":12,"question":"<div …>题干 + 待修正按钮 HTML</div>","answer":"传动角","is_ai":true,"is_pending_correction":false}}
{"code":0,"data":null,"message":"错误: 未选择模型"}
```
HTTP 状态：命中/AI 成功 200；未授权 401；未选择模型 400；模型超时 408；其它模型错误 500。

处理流程：题库精确命中 → 模糊候选（Top 5）+ AI 同题判重 → AI 答题（题干含图片 URL 时走视觉模型）。`autoAddToQuestionBank=true` 时 AI 答案自动入库到 `questionSaveFolderId` 指定文件夹。

OCS 题库配置示例：
```json
[{"name":"ZE题库(自建版)","url":"https://qa.example.com/query?token=<查询令牌>","method":"get","type":"GM_xmlhttpRequest","contentType":"json","data":{"title":"${title}","options":"${options}","type":"${type}"},"handler":"return (res)=>res.code === 0 ? [res.message, undefined] : [res.data.question,res.data.answer,{ai: res.data.is_ai}]"}]
```

### `POST /api/questions/{id}/pending-correction?token=`
`{success:true,message:"题目已标记为待修正",id}`

### 辅助
`GET /api/time`、`POST /api/echo`。

## 登录

`POST /api/login {"token":"…"}` → `{success:true,role:"admin"|"user",name}` 或 `{success:false,message}`（始终 200）。

## 管理接口 `/api/admin`

### 设置
| 方法 | 路径 | 说明 |
|---|---|---|
| GET / PUT | `/settings` | AppSettings（PUT 全量；`adminToken` 不可置空） |
| GET / PUT | `/model-settings` | ModelSettings（含 API Key，仅管理员可见） |
| GET | `/remote-catalog` | 代理拉取远程模型目录 JSON（合并由前端完成） |
| POST | `/models/test` | `{modelId, prompt?, imageDataUrl?, messages?}` → SSE：`chunk {content,reasoning_content}`、`done {content,reasoning_content,usage,elapsed_ms}`、`error {message}` |
| POST | `/platforms/probe-models` | `{baseUrl, apiKey, customHeaders?}` → `{models:[…]}` |

### 文件夹
| 方法 | 路径 | 说明 |
|---|---|---|
| GET | `/folders` | `Folder[]` |
| GET | `/folders/stats` | `FolderStat[]` |
| GET | `/folders/{id}/path` | 面包屑 |
| GET | `/folders/{id}/count` | `{count}` |
| POST | `/folders` | `{name, parentId}` → `{id}` |
| PATCH | `/folders/{id}` | `{name?, parentId?}`（含祖先环检测） |
| DELETE | `/folders/{id}?deleteQuestions=` | 不删题时题目上移到父级 |
| POST | `/folders/{id}/clear` | 清空子树题目 |

### 题目
| 方法 | 路径 | 说明 |
|---|---|---|
| GET | `/questions?folderId&recursive&pendingOnly&page&pageSize&sortOrder` | `{items,total}` |
| GET | `/questions/search?q&folderId` | `{items, terms}`（服务端 jieba 分词） |
| GET | `/questions/pending-count` | `{count}` |
| GET | `/questions/export?folderId&recursive&pendingOnly` | `AIResponse[]` |
| POST | `/questions` | `{content, options?, answer?, questionType?, folderId, isAi?}` → `{id,item}` |
| POST | `/questions/bulk` | `{items:[…]}` → `{inserted, ids}`（单事务） |
| PATCH | `/questions/{id}` | `{content, options?, answer?, questionType?, folderId?}` 全量更新并清除待修正标记；提供 `folderId` 且变化时移动 |
| DELETE | `/questions/{id}` | |
| POST | `/questions/batch-delete` | `{ids}` |
| POST | `/questions/{id}/move` `/copy` | `{targetFolderId}` |
| POST | `/questions/{id}/pending` | `{pending}` |

### 日志与统计
| 方法 | 路径 | 说明 |
|---|---|---|
| GET | `/logs?page&pageSize` | 持久化日志 `{items,total}`（保留最近 2000 条） |
| GET | `/logs/recent` | 内存中最近 100 条（含进行中） |
| DELETE | `/logs` | 清空 |
| GET | `/logs/stream[?token=]` | SSE，事件名 `log` / `model_call_request` / `model_call_progress` / `model_call_response` / `ocs_head`，data 为扁平 JSON（含 `type`） |
| GET | `/stats/daily` | `[["YYYY-MM-DD", count], …]`（365 天） |

### 其它
| 方法 | 路径 | 说明 |
|---|---|---|
| GET | `/image?url=` | 图片代理 → `{dataUrl}`（拒绝内网地址，≤8MB，磁盘缓存） |
| GET | `/segment?text=` | jieba 分词 |
| GET | `/version` | `{version}` |

## 静态资源
其余 GET 路径返回嵌入的管理后台（SPA，未命中回退 `index.html`）。
