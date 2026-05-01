#!/bin/bash
# CodexProxy 启动脚本
# 用法：
#   ./scripts/start.sh          # 开发模式
#   ./scripts/start.sh --build  # 构建生产版本

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_DIR"

case "${1:-}" in
  --build)
    echo "构建 CodexProxy..."
    npm run tauri build 2>&1 | tee logs/build.log
    ;;
  *)
    echo "启动 CodexProxy 开发模式..."
    mkdir -p logs
    npm run tauri dev 2>&1 | tee logs/dev.log
    ;;
esac
