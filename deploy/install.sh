#!/usr/bin/env bash
# ZError Server 安装脚本（systemd）
# 用法：
#   下载 Release：sudo ./deploy/install.sh --download          （从 GitHub Releases 取最新 linux-x86_64 二进制，推荐）
#   已有二进制：  sudo ./deploy/install.sh --bin ./zerror-server
#   本机构建：    sudo ./deploy/install.sh --build            （需要 Rust 工具链与 Node.js 20+，内存 ≥ 4GB）
# 可选：
#   --port 8080                                          对外端口（默认 3000）
#   --cert /path/fullchain.pem --key /path/privkey.pem   启用 HTTPS（证书由你的证书管理器维护，更新后自动热加载）
set -euo pipefail

BIN=""
BUILD=0
DOWNLOAD=0
REPO="${ZERROR_REPO:-cailuoli520/ZError}"
PORT=3000
CERT=""
KEY=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --bin) BIN="$2"; shift 2 ;;
    --build) BUILD=1; shift ;;
    --download) DOWNLOAD=1; shift ;;
    --port) PORT="$2"; shift 2 ;;
    --cert) CERT="$2"; shift 2 ;;
    --key) KEY="$2"; shift 2 ;;
    *) echo "未知参数: $1"; exit 1 ;;
  esac
done

if [[ $EUID -ne 0 ]]; then
  echo "请用 root 运行（sudo）"; exit 1
fi
if [[ ( -n "$CERT" && -z "$KEY" ) || ( -z "$CERT" && -n "$KEY" ) ]]; then
  echo "--cert 与 --key 必须同时提供"; exit 1
fi

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

if [[ $DOWNLOAD -eq 1 ]]; then
  echo "==> 下载最新 Release（${REPO}）"
  TMP="$(mktemp -d)"
  URL="https://github.com/${REPO}/releases/latest/download/zerror-server-linux-x86_64.tar.gz"
  curl -fL --retry 3 -o "$TMP/pkg.tar.gz" "$URL"
  curl -fL --retry 3 -o "$TMP/pkg.sha256" "${URL}.sha256" && (cd "$TMP" && sed 's#zerror-server-linux-x86_64.tar.gz#pkg.tar.gz#' pkg.sha256 | sha256sum -c -)
  tar -C "$TMP" -xzf "$TMP/pkg.tar.gz"
  BIN="$TMP/zerror-server"
fi

if [[ $BUILD -eq 1 ]]; then
  echo "==> 构建管理后台"
  (cd "$ROOT/web" && npm install --no-audit --no-fund && NODE_OPTIONS=--max-old-space-size=2048 npm run build)
  echo "==> 构建服务端（release）"
  (cd "$ROOT/server" && cargo build --release)
  BIN="$ROOT/server/target/release/zerror-server"
fi

if [[ -z "$BIN" || ! -x "$BIN" ]]; then
  echo "未找到可执行文件，请用 --bin 指定或加 --build"; exit 1
fi

echo "==> 创建用户与目录"
id -u zerror >/dev/null 2>&1 || useradd -r -s /usr/sbin/nologin -d /var/lib/zerror zerror
install -d -o zerror -g zerror -m 750 /var/lib/zerror
install -d -m 755 /opt/zerror /etc/zerror
systemctl stop zerror 2>/dev/null || true
install -m 755 "$BIN" /opt/zerror/zerror-server

if [[ ! -f /etc/zerror/env ]]; then
  TOKEN="$(head -c 24 /dev/urandom | od -An -tx1 | tr -d ' \n')"
  {
    echo "# ZError Server 环境变量（各项说明见 deploy/env.example）"
    echo "ZERROR_BIND=0.0.0.0:${PORT}"
    echo "ZERROR_ADMIN_TOKEN=${TOKEN}"
    echo "ZERROR_TLS_CERT=${CERT}"
    echo "ZERROR_TLS_KEY=${KEY}"
    echo "ZERROR_PUBLIC_URL="
    echo "ZERROR_TRUST_PROXY=false"
    echo "ZERROR_LOG=info"
  } > /etc/zerror/env
  chmod 600 /etc/zerror/env
  echo "==> 已生成管理员令牌: ${TOKEN}（保存在 /etc/zerror/env）"
else
  echo "==> 保留已有 /etc/zerror/env（如需启用 HTTPS 请手动填写 ZERROR_TLS_CERT / ZERROR_TLS_KEY）"
fi

if [[ -n "$CERT" ]]; then
  for f in "$CERT" "$KEY"; do
    if ! sudo -u zerror test -r "$f"; then
      echo "警告: zerror 用户无法读取 $f，请授予读权限，例如: setfacl -m u:zerror:r \"$f\""
    fi
  done
fi

install -m 644 "$ROOT/deploy/zerror.service" /etc/systemd/system/zerror.service
systemctl daemon-reload
systemctl enable --now zerror
sleep 1
systemctl --no-pager --lines=5 status zerror || true

SCHEME=http
[[ -n "$CERT" ]] && SCHEME=https
cat <<EOF

安装完成。
- 访问地址: ${SCHEME}://<服务器IP或域名>:${PORT}   （管理后台与 /query 同端口）
- 数据目录: /var/lib/zerror
- 环境变量: /etc/zerror/env   （修改后执行 systemctl restart zerror）
- 日志:     journalctl -u zerror -f
以后启用 HTTPS：在 /etc/zerror/env 填写 ZERROR_TLS_CERT / ZERROR_TLS_KEY 后重启服务；证书文件被证书管理器更新后会自动热加载。
EOF
