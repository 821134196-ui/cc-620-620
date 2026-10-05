#!/usr/bin/env bash
# 一条命令启动：构建前端 → 启动 Axum（自动迁移 + 种子数据 + 托管静态资源）
set -euo pipefail
cd "$(dirname "$0")"

# 1) Rust 工具链（若通过 rustup 安装但未进 PATH）
if ! command -v cargo >/dev/null 2>&1; then
  # shellcheck disable=SC1091
  [ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env"
fi
command -v cargo >/dev/null 2>&1 || { echo "未找到 cargo，请先安装 Rust：https://rustup.rs"; exit 1; }
command -v npm  >/dev/null 2>&1 || { echo "未找到 npm，请先安装 Node.js（构建前端用）"; exit 1; }

# 2) 前端依赖与生产构建（web/dist 由 Axum 同源托管）
if [ ! -d web/node_modules ]; then
  echo "==> 安装前端依赖"
  (cd web && npm install)
fi
if [ ! -d web/dist ] || [ -n "$(find web/src web/index.html -newer web/dist -type f 2>/dev/null | head -1)" ]; then
  echo "==> 构建前端"
  (cd web && npm run build)
fi

# 3) 启动后端（首次启动自动建表迁移并写入演示树木）
export BIND_ADDR="${BIND_ADDR:-127.0.0.1:8080}"
export DATABASE_URL="${DATABASE_URL:-sqlite://data/treecare.db?mode=rwc}"
echo "==> 启动服务：http://${BIND_ADDR} （居民公开查询与工作人员台均在此页面切换）"
exec cargo run --release
