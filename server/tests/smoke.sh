#!/usr/bin/env bash
# 冒烟测试：启动 mock OpenAI 与 zerror-server，跑一遍公开/管理接口。
set -u
cd "$(dirname "$0")/.."
B=http://127.0.0.1:18080
DATA=/tmp/zerror-smoke
rm -rf "$DATA" && mkdir -p "$DATA"
cat > "$DATA/model_config.json" <<'EOF'
{"selectedTextModels":["m1"],"platforms":[{"id":"p1","name":"mock","displayName":"Mock","baseUrl":"http://127.0.0.1:18081","apiKey":"k","enabled":true,"models":[{"id":"m1","name":"mock-1","displayName":"Mock 1","platformId":"p1","category":"text","enabled":true,"apiProtocol":"openai-chat"}]}]}
EOF

python3 tests/mock_openai.py &
MOCK=$!
ZERROR_DATA_DIR="$DATA" ZERROR_BIND=127.0.0.1:18080 ZERROR_ADMIN_TOKEN=adm ZERROR_LOG=info ${ZERROR_BIN:-./target/debug/zerror-server} > "$DATA/server.log" 2>&1 &
SRV=$!
trap 'kill $MOCK $SRV 2>/dev/null' EXIT
sleep 2
H='Authorization: Bearer adm'
J='content-type: application/json'

echo "--- HEAD /"; curl -s -I -X HEAD $B/ | head -1
echo "--- status"; curl -s $B/api/status; echo
echo "--- query no token -> $(curl -s -o /dev/null -w '%{http_code}' "$B/query?title=x")"
echo "--- login"; curl -s -X POST $B/api/login -H "$J" -d '{"token":"adm"}'; echo
echo "--- add token user"
curl -s $B/api/admin/settings -H "$H" | python3 -c "import json,sys; s=json.load(sys.stdin); s['multiUser']={'enabled':True,'users':[{'id':'u1','name':'ocs','token':'ocs-token','createdAt':''}]}; print(json.dumps(s))" > "$DATA/s.json"
curl -s -X PUT $B/api/admin/settings -H "$H" -H "$J" -d @"$DATA/s.json" | head -c 120; echo
echo "--- add folder + question"
curl -s -X POST $B/api/admin/folders -H "$H" -H "$J" -d '{"name":"机械原理","parentId":0}'; echo
curl -s -X POST $B/api/admin/questions -H "$H" -H "$J" -d '{"content":"韩国的首都在哪里","answer":"首尔","folderId":1}' | head -c 200; echo
echo "--- exact hit via user token"; curl -s "$B/query?title=韩国的首都在哪里&token=ocs-token"; echo
echo "--- AI path (mock)"; curl -s -X POST "$B/query?token=ocs-token" -H "$J" -d '{"title":"凸轮机构中从动件运动规律取决于（ ）。","options":"A. 压力角\nB. 传动角\nC. 极力夹角","type":"single"}'; echo
echo "--- stored list"; curl -s "$B/api/admin/questions?page=1&pageSize=5" -H "$H" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d['total'], [(i['question'][:12], i['answer'], i['folder_name']) for i in d['items']])"
echo "--- fuzzy variant -> same-question check -> AI"; curl -s "$B/query?title=韩国首都是哪里&token=ocs-token"; echo
echo "--- search"; curl -s "$B/api/admin/questions/search?q=首都" -H "$H" | head -c 200; echo
echo "--- probe models"; curl -s -X POST $B/api/admin/platforms/probe-models -H "$H" -H "$J" -d '{"baseUrl":"http://127.0.0.1:18081","apiKey":"k"}'; echo
echo "--- model test SSE"; curl -s -N -X POST $B/api/admin/models/test -H "$H" -H "$J" -d '{"modelId":"m1","prompt":"hi"}' | head -c 500; echo
echo "--- logs"; curl -s "$B/api/admin/logs?pageSize=2" -H "$H" | head -c 300; echo
echo "--- daily"; curl -s $B/api/admin/stats/daily -H "$H"; echo
echo "--- static: $(curl -s $B/ | head -c 60) / spa route -> $(curl -s -o /dev/null -w '%{http_code}' $B/some/spa/route)"
echo "--- server log tail"; tail -5 "$DATA/server.log"
