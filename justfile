set dotenv-load := true
set export := true

default:
    @just --list

# Install Bun packages and the Rust Android build tooling.
install:
    ./scripts/install.sh

# Run the gateway and the fast mock browser preview.
dev:
    ./scripts/dev.sh mock

# Run the gateway and browser preview against real Ollama inference.
dev-real:
    ./scripts/dev.sh real

# Run only the Rust gateway.
gateway:
    cargo run -p bridge-gateway

# Run only the mock browser preview.
web:
    bun run dev

# Run only the real browser preview; the gateway must already be running.
web-real:
    bun run dev:real

# Compile and install the debug Android app on a connected phone.
android:
    bun --cwd apps/bridge android

# Compile the debug Android app without installing it.
android-build:
    bun --cwd apps/bridge android:build

# Install the existing debug APK on a connected phone.
android-install:
    adb install -r apps/bridge/android/app/build/outputs/apk/debug/app-debug.apk

# Build a signed release APK (requires android/keystore.properties).
apk:
    ./scripts/build-apk.sh

# Run Rust and TypeScript tests.
test:
    cargo test --workspace
    bun run test

# Compile, type-check, test formatting, and build the web preview.
check:
    cargo check --workspace
    cargo fmt --all -- --check
    bun run typecheck
    bun run build:web

# Format Rust, TypeScript, Kotlin, JSON, and Markdown sources.
fmt:
    cargo fmt --all
    bunx prettier --write "apps/bridge/**/*.{ts,tsx,js,json,md}" "*.{json,md}"

# Build and install the launchd gateway plus persistent Tailscale Serve HTTPS.
mac-install:
    ./scripts/mac-install.sh

# Remove Bridge's launchd job and Tailscale Serve mapping; preserve data.
mac-uninstall:
    ./scripts/mac-uninstall.sh

# Show the health of local Bridge dependencies and services.
status:
    ./scripts/status.sh

# Follow the launchd gateway logs.
logs:
    tail -f "${HOME}/Library/Logs/Bridge/gateway.log" "${HOME}/Library/Logs/Bridge/gateway.error.log"

# Remove reproducible build output while preserving credentials and databases.
clean:
    cargo clean
    rm -rf apps/bridge/dist apps/bridge/android/app/build apps/bridge/android/build
