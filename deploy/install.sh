#!/usr/bin/env bash
# ZError Server 原生安装脚本（systemd）
# 用法：
#   1) 已有二进制：  sudo ./install.sh --bin ./zerror-server
#   2) 本机构建：    sudo ./install.sh --build      （需要 Rust 工具链与 Node.js 20+）
# 可选：--domain qa.example.com  生成 /etc/caddy/Caddyfile 片段提示
set -euo pipefail

BIN=""
BUILD=0
DOMAIN=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --bin) BIN="$2"; shift 2 ;;
    --build) BUILD=1; shift ;;
    --domain) DOMAIN="$2"; shift 2 ;;
    *) echo "未知参数: $1"; exit 1 ;;
  esac
done

if [[ $EUID -ne 0 ]]; then
  echo "请用 root 运行（sudo）"; exit 1
fi

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

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
install -m 755 "$BIN" /opt/zerror/zerror-server

if [[ ! -f /etc/zerror/env ]]; then
  TOKEN="$(head -c 24 /dev/urandom | od -An -tx1 | tr -d ' \n')"
  cat > /etc/zerror/env <<EOF
# ZError Server 环境变量
ZERROR_BIND=127.0.0.1:3000
ZERROR_ADMIN_TOKEN=${TOKEN}
${DOMAIN:+ZERROR_PUBLIC_URL=https://$DOMAIN}
ZERROR_LOG=info
EOF
  chmod 600 /etc/zerror/env
  echo "==> 已生成管理员令牌: ${TOKEN}（保存在 /etc/zerror/env）"
fi

install -m 644 "$ROOT/deploy/zerror.service" /etc/systemd/system/zerror.service
systemctl daemon-reload
systemctl enable --now zerror
sleep 1
systemctl --no-pager --lines=5 status zerror || true

cat <<EOF

安装完成。
- 本地地址: http://127.0.0.1:3000  （管理后台与 /query 同端口）
- 数据目录: /var/lib/zerror
- 环境变量: /etc/zerror/env
建议用 Caddy 做 HTTPS 反代，示例 /etc/caddy/Caddyfile：

${DOMAIN:-qa.example.com} {
    reverse_proxy 127.0.0.1:3000 {
        flush_interval -1
    }
}
EOF
