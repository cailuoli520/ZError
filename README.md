<div align="center">
  <h2>ZError Server</h2>
  <span>OCS 网课助手 AI 题库 · Linux VPS 版（服务端 + 网页管理后台）</span>
</div>

## 简介

本项目由 [ZError](https://github.com/Miaozeqiu/ZError) 桌面版改造而来：把原本运行在 Tauri 桌面应用里的题库服务和 AI 调用整体搬到 Linux 服务器上，通过浏览器管理，并对公网提供 OCS 题库接口。

- **单个二进制**：Rust（axum）服务端，内嵌 Vue 3 管理后台，SQLite 存储。
- **公网题库接口**：`GET/POST /query`，契约与桌面版一致，可直接填入 OCS 题库配置；访问需令牌。
- **服务端 AI 引擎**：支持 `openai-chat` / `openai-response` / `anthropic` 三种协议；多模型并发、总结/多数投票、同题判重、URL 图片题（视觉模型）。
- **题库管理**：文件夹树、题目增删改查、批量导入导出（csv/xlsx/docx/pdf/txt）、待修正标记、模糊搜索（jieba 分词）。
- **实时日志**：SSE 推送每次查询及模型流式输出到管理后台。
- **可直接复用桌面版数据库**：把桌面版的 `airesponses.db` 拷到数据目录即可。

## 快速开始（Docker Compose，推荐）

需要一台有公网 IP 的 Linux VPS，已安装 Docker，防火墙放开 80（如需 HTTPS 再放开 443）。

```bash
git clone -b vps-server https://github.com/cailuoli520/ZError.git zerror && cd zerror
cp deploy/.env.example deploy/.env
# 编辑 deploy/.env：
#   没有域名 → SITE_ADDRESS=:80（保持默认，用 http://服务器IP 访问）
#   有域名   → SITE_ADDRESS=你的域名（Caddy 自动申请证书，http 自动跳转 https）
#   ZERROR_ADMIN_TOKEN=自定义管理员令牌（可留空自动生成）
docker compose -f deploy/docker-compose.yml --env-file deploy/.env up -d --build
docker compose -f deploy/docker-compose.yml logs -f zerror   # 首次启动会打印管理员令牌
```

- 无域名：浏览器打开 `http://服务器IP`，OCS 的题库地址为 `http://服务器IP/query?token=…`。
- 之后有了域名：把 DNS 解析到服务器，改 `deploy/.env` 的 `SITE_ADDRESS=你的域名`，再执行一次 `docker compose ... up -d`，即自动切到 HTTPS；服务端根据 Caddy 传来的 `X-Forwarded-Proto` 自动生成正确的链接，OCS 里把地址改成 `https://` 即可。

## 原生安装（systemd）

```bash
# 需要 Rust 1.80+ 与 Node.js 20+
sudo ./deploy/install.sh --build --domain 你的域名
```

脚本会创建 `zerror` 用户、安装二进制到 `/opt/zerror`、数据目录 `/var/lib/zerror`、环境变量 `/etc/zerror/env`（含自动生成的管理员令牌），并启用 `zerror.service`（监听 `127.0.0.1:3000`）。HTTPS 请用 Caddy/Nginx 反代，脚本末尾给出 Caddy 示例；反代 SSE 时需关闭缓冲（Caddy `flush_interval -1`，Nginx `proxy_buffering off`）。

## 配置 OCS

1. 管理后台 → **常规设置 → 公网题库访问**：添加一个查询令牌。
2. 首页 → **OCS 配置**：复制生成的题库配置（`url` 形如 `https://域名/query?token=…`），粘贴到 OCS 网课助手的题库配置中。
3. **设置 → 模型设置**：添加平台（Base URL、API Key）和模型，在首页选择文本模型（可多选，≤5）、可选总结模型与视觉模型。

## 环境变量

| 变量 | 默认 | 说明 |
|---|---|---|
| `ZERROR_DATA_DIR` | `./data` | 数据目录（`airesponses.db`、`settings.json`、`model_config.json`、`image_cache/`） |
| `ZERROR_BIND` | `0.0.0.0:3000` | 监听地址 |
| `ZERROR_ADMIN_TOKEN` | 无 | 首次启动写入的管理员令牌；为空则随机生成并打印 |
| `ZERROR_PUBLIC_URL` | 无 | 对外根 URL（用于生成待修正按钮链接）；为空时按 `X-Forwarded-Proto/Host` 推断 |
| `ZERROR_TRUST_PROXY` | `true` | 信任 `X-Forwarded-For` |
| `ZERROR_LOG` | `info` | 日志级别 |

## 从桌面版迁移

- 题库：把桌面版 `~/.local/share/zerror/airesponses.db`（Windows：`%LOCALAPPDATA%\ZError\airesponses.db`）复制到数据目录，重启即自动迁移 schema。
- 模型配置：桌面版 `model_config.json` 可直接放到数据目录；`apiProtocol=custom`（自定义 JS）的模型会按 `openai-chat` 处理，`jsCode` 不再执行。
- 设置：`config.json` 中与题库相关的字段（`autoAddToQuestionBank`、`modelResponseTimeout`、`modelRetryCount`、`questionSaveFolderId`、`adminToken`、`multiUser`）可复制到 `settings.json`。

## 开发

```bash
# 服务端
cd server && cargo run          # 默认 http://0.0.0.0:3000，数据目录 ./data
cargo test
bash tests/smoke.sh             # 用本地 mock OpenAI 跑一遍完整链路

# 前端（开发时代理 /api 与 /query 到 127.0.0.1:3000）
cd web && npm install && npm run dev
npm run build                   # 产物 web/dist，被 server 通过 rust-embed 嵌入
```

接口文档见 [docs/API.md](docs/API.md)，架构与取舍见 [docs/DESIGN.md](docs/DESIGN.md)。

## 与桌面版的差异

- AI 调用在服务端执行，API Key 只保存在服务器（`model_config.json`），管理后台通过管理员令牌读写。
- 不再支持自定义 JS 协议模型、系统托盘、自动更新、二级窗口等桌面特性。
- 公网 `/query` 默认必须携带令牌，防止 AI 额度被滥用。

## 许可证

MIT
