#!/usr/bin/env bash
# 启动本地端到端环境：mock OpenAI + zerror-server（嵌入前端），供浏览器验证。
set -u
cd "$(dirname "$0")/.."
DATA="${1:-/tmp/zerror-e2e-$(date +%s)}"
mkdir -p "$DATA"
cat > "$DATA/model_config.json" <<'EOF'
{"selectedTextModels":["m1"],"platforms":[{"id":"p1","name":"mock","displayName":"Mock","baseUrl":"http://127.0.0.1:18081","apiKey":"k","enabled":true,"models":[{"id":"m1","name":"mock-1","displayName":"Mock 1","platformId":"p1","category":"text","enabled":true,"apiProtocol":"openai-chat"}]}]}
EOF
pkill -x zerror-server 2>/dev/null || true
pgrep -f tests/mock_openai.py >/dev/null || (python3 tests/mock_openai.py >/dev/null 2>&1 &)
ZERROR_DATA_DIR="$DATA" ZERROR_BIND=127.0.0.1:18080 ZERROR_ADMIN_TOKEN=adm ./target/debug/zerror-server > "$DATA/server.log" 2>&1 &
sleep 2
echo "data dir: $DATA"
curl -s http://127.0.0.1:18080/api/status; echo
curl -s http://127.0.0.1:18080/ | head -c 160; echo
