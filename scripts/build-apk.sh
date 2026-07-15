#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PROPERTIES="$ROOT/apps/bridge/android/keystore.properties"
if [[ ! -f "$PROPERTIES" ]]; then
  echo "Missing $PROPERTIES" >&2
  echo "Copy keystore.properties.example, create a keystore with keytool, and fill in its values." >&2
  exit 1
fi

cd "$ROOT"
bun --cwd apps/bridge apk
echo "APK: $ROOT/apps/bridge/android/app/build/outputs/apk/release/app-release.apk"

