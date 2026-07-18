#!/usr/bin/env bash
set -euo pipefail
umask 077

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MODE="${1:-mock}"
TOKEN="${BRIDGE_DEV_API_TOKEN:-bridge-development-token-0000000000000000}"

export BRIDGE_API_TOKEN="$TOKEN"
export BRIDGE_DATABASE="${BRIDGE_DATABASE:-/tmp/bridge-development.db}"

cd "$ROOT"
cargo run -p bridge-gateway &
GATEWAY_PID=$!
trap 'kill "$GATEWAY_PID" 2>/dev/null || true' EXIT INT TERM

if [[ "$MODE" == "real" ]]; then
  export BRIDGE_DEV_API_TOKEN="$TOKEN"
  bun run dev:real
else
  bun run dev
fi
