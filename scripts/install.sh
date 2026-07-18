#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

for command in cargo rustup bun java; do
  if ! command -v "$command" >/dev/null; then
    echo "Missing required command: $command" >&2
    exit 1
  fi
done

bun install
rustup target add aarch64-linux-android
if ! command -v cargo-ndk >/dev/null; then
  cargo install cargo-ndk --locked
fi

ANDROID_HOME="${ANDROID_HOME:-/opt/homebrew/share/android-commandlinetools}"
if [[ ! -d "$ANDROID_HOME/ndk/27.3.13750724" ]]; then
  echo "Android NDK 27.3.13750724 was not found under $ANDROID_HOME." >&2
  echo "Install it with sdkmanager or set ANDROID_HOME to the correct SDK." >&2
  exit 1
fi

for optional in ollama tailscale adb; do
  if ! command -v "$optional" >/dev/null; then
    echo "Note: $optional is not on PATH; it is required only for its related runtime command."
  fi
done

echo "Bridge development dependencies are ready. Run: just android"
