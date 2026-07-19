set dotenv-load := true
set export := true

default:
    @just --list

# Install Bun packages and the Rust Android build tooling.
install:
    ./scripts/install.sh

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

# Run android-core and TypeScript tests.
test:
    cargo test --workspace
    bun run test

# Compile, lint, type-check, and test formatting.
check:
    cargo check --workspace
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets -- -D warnings
    bun run typecheck

# Format Rust, TypeScript, JavaScript, JSON, and Markdown sources.
fmt:
    cargo fmt --all
    bunx prettier --write "apps/bridge/**/*.{ts,tsx,js,json,md}" "*.{json,md}"

# Remove reproducible client build output while preserving credentials.
clean:
    cargo clean
    rm -rf apps/bridge/android/app/build apps/bridge/android/build
