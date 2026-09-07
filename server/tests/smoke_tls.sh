#!/usr/bin/env bash
# HTTPS 冒烟测试：自签证书启动服务，验证 https 访问、链接协议推断、证书热加载。
set -u
cd "$(dirname "$0")/.."
BIN=${ZERROR_BIN:-./target/debug/zerror-server}
DATA=/tmp/zerror-tls-smoke
rm -rf "$DATA" && mkdir -p "$DATA/certs"
gen_cert() {
  openssl req -x509 -newkey rsa:2048 -nodes -days 2 -subj "/CN=$1" \
    -keyout "$DATA/certs/key.pem" -out "$DATA/certs/cert.pem" >/dev/null 2>&1
}
gen_cert first.local

ZERROR_DATA_DIR="$DATA" ZERROR_BIND=127.0.0.1:18443 ZERROR_ADMIN_TOKEN=adm ZERROR_TRUST_PROXY=false \
  ZERROR_TLS_CERT="$DATA/certs/cert.pem" ZERROR_TLS_KEY="$DATA/certs/key.pem" \
  "$BIN" > "$DATA/server.log" 2>&1 &
SRV=$!
trap 'kill $SRV 2>/dev/null' EXIT
sleep 2
B=https://127.0.0.1:18443
echo "--- status over https"; curl -sk $B/api/status | head -c 120; echo
echo "--- cert CN: $(echo | openssl s_client -connect 127.0.0.1:18443 2>/dev/null | openssl x509 -noout -subject 2>/dev/null)"
echo "--- plain http rejected -> $(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:18443/api/status 2>/dev/null || echo 'conn-error(expected)')"
curl -sk -X POST $B/api/admin/questions -H 'Authorization: Bearer adm' -H 'content-type: application/json' -d '{"content":"q1","answer":"a1","folderId":0}' >/dev/null
echo "--- pending button scheme: $(curl -sk "$B/query?title=q1&token=adm" | grep -o "https://127.0.0.1:18443/api/questions/[0-9]*/pending-correction" | head -1)"
if [[ "${SKIP_RELOAD:-0}" != "1" ]]; then
  echo "--- regenerate cert and wait for hot reload (~65s)"
  sleep 1; gen_cert second.local; sleep 65
  echo "--- cert CN now: $(echo | openssl s_client -connect 127.0.0.1:18443 2>/dev/null | openssl x509 -noout -subject 2>/dev/null)"
  grep -c "已热加载" "$DATA/server.log" | sed 's/^/--- reload log lines: /'
fi
echo "--- server log"; grep -v "^$" "$DATA/server.log" | cut -c1-160 | tail -4
