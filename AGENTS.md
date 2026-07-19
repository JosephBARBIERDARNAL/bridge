# Repository Guidelines

## Project Structure & Module Organization

Bridge is a Bun and Rust repository for a private Android chat client.

- `apps/bridge/src/` contains the React Native and TypeScript UI.
- `apps/bridge/android/` contains the native Android project and Kotlin bridge code.
- `crates/android-core/` contains the Rust HTTP/SSE client exported to Android through UniFFI.
- `scripts/` contains Android development, installation, and packaging helpers.
- Android `res/` directories contain visual assets.

The separately deployed Mai backend owns the HTTP API, SQLite history, Ollama integration, web tools, and macOS service scripts.

## Build, Test, and Development Commands

Use the root `justfile` as the main command interface:

- `just install` installs Bun dependencies and Rust Android tooling.
- `just android` builds and installs the debug app on a connected phone.
- `just android-build` assembles a debug APK without installing it.
- `just apk` assembles a signed release APK.
- `just test` runs `android-core` and Bun tests.
- `just check` runs Rust compilation, formatting, and Clippy checks plus TypeScript type-checking.
- `just fmt` formats Rust, TypeScript, JavaScript, JSON, and Markdown.

Always check for lint, format, and test issues before submitting changes.

## Coding Style & Naming Conventions

Rust uses edition 2024 and standard `rustfmt`; use `snake_case` for modules and functions and `PascalCase` for types. TypeScript is strict and formatted with Prettier using two-space indentation. Name React components in `PascalCase` and functions and variables in `camelCase`. Run `just fmt` before submitting changes.

## Testing Guidelines

Rust tests live beside implementation code in `#[cfg(test)]` modules and use `#[test]` or `#[tokio::test]`. TypeScript tests use Bun's test runner and the `*.test.ts` naming pattern. Add focused regression tests for changed behavior, especially HTTP payload and SSE compatibility with Mai.

## Security & Configuration

Never commit API tokens, `.env` files, keystores, built APKs, or generated native libraries. Release APKs require a local `apps/bridge/android/keystore.properties`. The client must require HTTPS for non-loopback gateway URLs and store credentials through the Android credential store.
