#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DATA="$HOME/Library/Application Support/Bridge"
BIN="$DATA/bin"
LOGS="$HOME/Library/Logs/Bridge"
TOKEN="$DATA/token"
PLIST="$HOME/Library/LaunchAgents/app.bridge.gateway.plist"

mkdir -p "$BIN" "$LOGS" "$(dirname "$PLIST")"
if [[ ! -f "$TOKEN" ]]; then
  umask 077
  openssl rand -hex 32 > "$TOKEN"
fi
chmod 600 "$TOKEN"

cd "$ROOT"
cargo build -p bridge-gateway --release
cp "$ROOT/target/release/bridge-gateway" "$BIN/bridge-gateway"

cat > "$PLIST" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Label</key><string>app.bridge.gateway</string>
  <key>ProgramArguments</key><array><string>$BIN/bridge-gateway</string></array>
  <key>EnvironmentVariables</key><dict>
    <key>BRIDGE_TOKEN_FILE</key><string>$TOKEN</string>
    <key>BRIDGE_MODEL</key><string>gemma4:26b</string>
    <key>RUST_LOG</key><string>bridge_gateway=info</string>
  </dict>
  <key>RunAtLoad</key><true/><key>KeepAlive</key><true/>
  <key>StandardOutPath</key><string>$LOGS/gateway.log</string>
  <key>StandardErrorPath</key><string>$LOGS/gateway.error.log</string>
</dict></plist>
PLIST

launchctl bootout "gui/$UID/app.bridge.gateway" 2>/dev/null || true
launchctl bootstrap "gui/$UID" "$PLIST"

if command -v tailscale >/dev/null; then
  tailscale serve --bg 8787
else
  echo "Tailscale CLI is not on PATH. Run 'tailscale serve --bg 8787' from its CLI after installing Tailscale."
fi

echo "Bridge gateway installed. API token: $TOKEN"
echo "Run 'just status' to verify the installation."

