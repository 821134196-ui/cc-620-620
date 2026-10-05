#!/usr/bin/env bash
# 一条命令启动：街道树木病虫害送鉴与处置通知系统
# 用法: ./start.sh [--release]
set -euo pipefail
cd "$(dirname "$0")"

export PATH="$HOME/.cargo/bin:$PATH"

MODE="debug"
CARGO_FLAG=""
if [[ "${1:-}" == "--release" ]]; then
  MODE="release"
  CARGO_FLAG="--release"
fi

echo "==> [1/3] 构建前端 (Svelte + Vite)"
if [[ ! -d frontend/node_modules ]]; then
  (cd frontend && npm install)
fi
(cd frontend && npm run build)

echo "==> [2/3] 构建后端 (Rust + Axum + SQLx/SQLite) [$MODE]"
(cd backend && cargo build $CARGO_FLAG)

echo "==> [3/3] 启动服务  http://localhost:${PORT:-8080}"
cd backend
STATIC_DIR="../frontend/dist" \
DATABASE_URL="${DATABASE_URL:-sqlite://./treecare.db?mode=rwc}" \
PORT="${PORT:-8080}" \
  "./target/$MODE/treecare"
