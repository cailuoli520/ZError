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

## 部署（systemd）

需要一台 Linux VPS（有公网 IP）。二进制由 GitHub Actions 自动构建并发布到 Releases，无需在服务器上编译。

```bash
git clone -b vps-server https://github.com/cailuoli520/ZError.git zerror && cd zerror
sudo ./deploy/install.sh --download --port 3000
# 也可以：--bin ./zerror-server（自带二进制）或 --build（本机编译，需 Rust + Node.js 20+，内存 ≥ 4GB）
```

脚本会创建 `zerror` 用户、把二进制装到 `/opt/zerror`、数据目录 `/var/lib/zerror`、环境变量 `/etc/zerror/env`（含自动生成的管理员令牌），并启用 `zerror.service`。防火墙放开端口后，浏览器打开 `http://服务器IP:3000` 登录后台；OCS 题库地址为 `http://服务器IP:3000/query?token=…`（后台首页「OCS 配置」会按当前访问地址自动生成）。

### 启用 HTTPS

程序不申请、不续签证书，只负责使用你已有的证书（acme.sh、certbot 等由你自己管理）。在 `/etc/zerror/env` 中填写 PEM 证书链与私钥路径后重启：

```bash
ZERROR_TLS_CERT=/path/to/fullchain.pem
ZERROR_TLS_KEY=/path/to/privkey.pem
```

```bash
sudo systemctl restart zerror
```

也可以在安装时直接传入 `--cert … --key …`。证书文件被证书管理器更新后，服务会在 60 秒内自动热加载，无需重启。注意 `zerror` 用户需要有读取证书文件的权限（例如 `setfacl -m u:zerror:r 文件`）。如需监听 443，`zerror.service` 已授予 `CAP_NET_BIND_SERVICE`，把 `ZERROR_BIND` 改为 `0.0.0.0:443` 即可。

### 日常维护

```bash
journalctl -u zerror -f          # 日志
sudo systemctl restart zerror    # 修改 /etc/zerror/env 后重启
```

升级：`sudo ./deploy/install.sh --download`（会保留已有 `/etc/zerror/env` 与数据）。

### 构建与发布（GitHub Actions）

推送到 `vps-server` 分支会自动执行前端构建、`cargo test`、release 编译与冒烟测试，并上传 `zerror-server-linux-x86_64` 构件；推送 `v*` 标签（如 `git tag v3.0.0 && git push origin v3.0.0`）会自动创建 Release 并附带二进制包，`install.sh --download` 即从这里下载。

## 配置 OCS

1. 管理后台 → **常规设置 → 公网题库访问**：添加一个查询令牌。
2. 首页 → **OCS 配置**：复制生成的题库配置（`url` 形如 `http://服务器IP:3000/query?token=…`），粘贴到 OCS 网课助手的题库配置中。
3. **设置 → 模型设置**：添加平台（Base URL、API Key）和模型，在首页选择文本模型（可多选，≤5）、可选总结模型与视觉模型。

## 环境变量

| 变量 | 默认 | 说明 |
|---|---|---|
| `ZERROR_DATA_DIR` | `./data` | 数据目录（`airesponses.db`、`settings.json`、`model_config.json`、`image_cache/`） |
| `ZERROR_BIND` | `0.0.0.0:3000` | 监听地址 |
| `ZERROR_TLS_CERT` / `ZERROR_TLS_KEY` | 无 | PEM 证书链与私钥；同时设置即以 HTTPS 监听，文件更新后自动热加载 |
| `ZERROR_ADMIN_TOKEN` | 无 | 首次启动写入的管理员令牌；为空则随机生成并打印 |
| `ZERROR_PUBLIC_URL` | 无 | 对外根 URL（用于生成待修正按钮链接）；为空时按 `X-Forwarded-Proto/Host` 推断 |
| `ZERROR_TRUST_PROXY` | `true` | 信任 `X-Forwarded-For` / `X-Forwarded-Proto`（直接对外时建议 `false`） |
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
