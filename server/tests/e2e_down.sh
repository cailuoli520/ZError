#!/usr/bin/env bash
# 停止本地端到端环境（按进程名精确匹配，避免误杀父 shell）。
pkill -x zerror-server 2>/dev/null || true
for pid in $(pgrep -f "mock_openai\.py$" 2>/dev/null); do kill "$pid" 2>/dev/null || true; done
echo "e2e 环境已停止"
