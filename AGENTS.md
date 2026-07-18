# Repository Guidelines

## Project Structure & Module Organization

Bridge is a Bun and Rust monorepo for a private Android chat client.

- `apps/bridge/src/` contains the React Native/TypeScript Android UI and native client.
- `apps/bridge/android/` contains the native Android project and Kotlin bridge code.
- `crates/gateway/` is the Axum gateway, SQLite persistence layer, and Ollama integration. Database migrations live in `crates/gateway/migrations/`.
- `crates/android-core/` provides the Rust networking core exported to Android through UniFFI.
- `scripts/` contains development, installation, status, and packaging helpers.
- Android `res/` directories contain visual assets.

## Build, Test, and Development Commands

Use the root `justfile` as the main command interface:

- `just install` installs Bun dependencies and Rust Android tooling.
- `just dev` starts the Rust gateway locally.
- `just test` runs all Rust workspace tests and Bun tests.
- `just check` runs Rust compilation and formatting checks plus TypeScript type-checking.
- `just fmt` formats Rust, TypeScript, JavaScript, JSON, and Markdown.
- `just android-build` assembles a debug APK without installing it.

Always check for lint, format and test issues before submitting your changes.

## Coding Style & Naming Conventions

Rust uses edition 2024 and standard `rustfmt`; use `snake_case` for modules/functions and `PascalCase` for types. TypeScript is strict and formatted with Prettier using two-space indentation. Name React components in `PascalCase` and functions and variables in `camelCase`. Run `just fmt` before submitting changes.

## Testing Guidelines

Rust tests live beside implementation code in `#[cfg(test)]` modules and use `#[test]` or `#[tokio::test]`. TypeScript tests use Bun’s test runner and the `*.test.ts` naming pattern. Add focused regression tests for changed behavior. No coverage threshold is currently enforced.

## Security & Configuration

Never commit tokens, `.env` files, databases, keystores, or built APKs. Release APKs require a local `apps/bridge/android/keystore.properties`.
