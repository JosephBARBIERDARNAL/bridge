#!/usr/bin/env bash
set -euo pipefail

PLIST="$HOME/Library/LaunchAgents/app.bridge.gateway.plist"
launchctl bootout "gui/$UID/app.bridge.gateway" 2>/dev/null || true
rm -f "$PLIST"
if command -v tailscale >/dev/null; then
  tailscale serve --https=443 off 2>/dev/null || tailscale serve off 2>/dev/null || true
fi
echo "Bridge services removed. Chat history and credentials were preserved."

