#!/usr/bin/env bash
set -u

DATA="$HOME/Library/Application Support/Bridge"
TOKEN="$DATA/token"

echo "Gateway launchd job:"
launchctl print "gui/$UID/app.bridge.gateway" 2>/dev/null | head -n 8 || echo "  not installed"
echo
echo "Gateway health:"
if [[ -f "$TOKEN" ]]; then
  curl --silent --show-error --fail -H "Authorization: Bearer $(<"$TOKEN")" http://127.0.0.1:8787/v1/health || true
  echo
else
  echo "  token not installed"
fi
echo
echo "Ollama models:"
ollama list 2>/dev/null | head -n 8 || echo "  Ollama unavailable"
echo
echo "Tailscale Serve:"
tailscale serve status 2>/dev/null || echo "  Tailscale CLI unavailable or Serve not configured"

