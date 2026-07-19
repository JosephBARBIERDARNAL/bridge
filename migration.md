# Bridge repository split migration plan

## Goal

Split the current monorepo into two independently buildable repositories without changing runtime behavior or breaking the Android-to-gateway API:

1. The current repository remains the Android client repository.
2. A new repository named Mai contains the Bridge gateway and its macOS service tooling.

During the local migration, Mai is staged as an ignored, nested Git repository at `mai/`. It can be moved to its final location and connected to a remote without carrying the Android repository with it.

Ollama remains an external, shared service. The backend repository owns the Bridge-specific Ollama integration and configuration, but it must not install, embed, stop, or remove the Ollama daemon. Other applications can continue using the same Ollama instance directly.

This migration is a source-code and repository-boundary change. It does not require moving the SQLite database, API token, installed gateway binary, Tailscale configuration, or Ollama models.

## Target architecture

```text
Android repository                         Backend repository
------------------                         ------------------
React Native UI                            Axum gateway
Kotlin native bridge         HTTPS + SSE   SQLite persistence
Rust android-core         ----------------> Ollama integration ------> Shared Ollama daemon
Android build/release tooling               Web tools
                                            launchd/Tailscale tooling
```

The existing `/v1` HTTP and SSE interface is the boundary between the repositories. Neither repository should acquire a source-code dependency on the other.

## Decisions to make before starting

Record these choices in the migration pull request before changing either repository:

- The backend repository is named `mai`. Its remote creation is deferred until after the local repository is moved.
- Decide who will have access before publishing Mai to a remote.
- Choose a short migration window during which API and database-schema changes are paused.
- Choose the source commit that will be the common split point. Both repositories should refer to that commit in their initial migration commits.
- Confirm that the new repository will preserve relevant Git history. The procedure below uses a filtered clone and is the recommended approach.
- Decide where the canonical API contract will live. The recommended location is `docs/api.md` in the backend repository, with the Android README linking to it.

Do not combine the repository split with route changes, model changes, database migrations, crate renames, directory flattening, or deployment changes. Keeping the first commits behavior-preserving makes failures and rollback much easier to understand.

## File ownership after the split

### Android repository: current repository

Keep:

- `apps/bridge/`
- `crates/android-core/`
- `package.json`
- `bun.lock`
- `scripts/build-apk.sh`
- Android portions of `scripts/install.sh`
- Android-specific root Cargo workspace configuration and `Cargo.lock`
- Android development commands in `justfile`
- Android and client checks in `.github/workflows/ci.yml`
- Client documentation, contribution guidance, and ignore rules
- `migration.md`, at least until the split has been completed and verified

Remove after the backend repository has been created and validated:

- `crates/gateway/`
- `scripts/mac-install.sh`
- `scripts/mac-uninstall.sh`
- `scripts/status.sh`
- Gateway, Ollama, Tailscale Serve, launchd, and macOS service commands from `justfile`
- Gateway-only dependencies from the root Cargo workspace
- Gateway-only CI steps and documentation

### Backend repository: new repository

Keep or create from the current repository:

- `crates/gateway/`, including all migrations and tests
- A root `Cargo.toml` containing only the gateway workspace member and its workspace dependencies
- A gateway-only `Cargo.lock`
- `scripts/mac-install.sh`
- `scripts/mac-uninstall.sh`
- `scripts/status.sh`
- The backend portions of `scripts/install.sh`, if an install helper is retained
- A backend-only `justfile`
- A Rust-only CI workflow
- Backend documentation and ignore rules
- Repository guidance appropriate to the backend

Do not copy these into the backend repository:

- `apps/bridge/`
- `crates/android-core/`
- `package.json` or `bun.lock`
- `scripts/build-apk.sh`
- Android SDK, Gradle, Kotlin, React Native, Bun, or APK tooling

### Root files that must be split manually

The following files contain concerns from both sides and cannot simply be assigned to one repository unchanged:

| File                       | Android repository                                                      | Backend repository                                                                                |
| -------------------------- | ----------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| `Cargo.toml`               | `crates/android-core` and its dependencies                              | `crates/gateway` and its dependencies                                                             |
| `Cargo.lock`               | Regenerate for the client workspace                                     | Regenerate for the gateway workspace                                                              |
| `justfile`                 | Install, Android build/install, APK, client test/check/format/clean     | Dev, gateway, test/check/format, macOS install/uninstall, status/logs/clean                       |
| `scripts/install.sh`       | Bun, Rust Android target, cargo-ndk, Java/NDK and adb checks            | Rust plus Ollama and Tailscale prerequisite checks                                                |
| `.github/workflows/ci.yml` | Rust client, TypeScript, and Android jobs                               | Rust gateway checks and tests                                                                     |
| `.gitignore`               | Node, Gradle, Android, generated UniFFI/JNI, APK and keystore entries   | Rust target, environment files, SQLite files, and local service artifacts                         |
| `README.md`                | Android build, installation, configuration, and a link to backend setup | Gateway setup, configuration, service management, storage, security, API, and Ollama prerequisite |
| `AGENTS.md`                | Android, React Native, Kotlin, and `android-core` guidance              | Axum, SQLite, gateway migrations, tools, and service-script guidance                              |

## Compatibility contract to freeze

Before splitting, record the existing interface in the backend repository. At minimum, document:

- Bearer-token authentication on the `/v1` routes.
- `GET /v1/health`.
- `GET` and `POST /v1/chats`.
- `GET`, `PATCH`, and `DELETE /v1/chats/{chat_id}`.
- `POST /v1/chats/{chat_id}/messages`.
- `POST /v1/chats/{chat_id}/messages/{message_id}/retry`.
- JSON representations for chat summaries, chat details, messages, health, and errors.
- The `web_search` request field.
- SSE event names and payloads: `message_started`, `thinking_delta`, `delta`, `tool_call`, `tool_result`, `message_completed`, and `error`.
- The behavior expected when a stream is cancelled or ends without a terminal event.

The initial split must retain `/v1`, all payload field names, status codes, SSE event names, and authentication behavior. The Android client already duplicates this contract across Rust/UniFFI and TypeScript types, so an accidental gateway change will not be caught by a shared compiler after the split.

## Phase 1: establish a clean baseline

1. Pause feature work that changes the gateway API or database schema.
2. Ensure the working tree contains no unrelated changes.
3. Record the source commit SHA that will be used as the split point.
4. Tag that commit with a clearly named pre-split tag, after agreeing on the tag name.
5. From the source commit, run the current repository's full verification:

   ```bash
   just fmt
   just check
   just test
   just android-build
   ```

6. Record any pre-existing failures. Do not silently treat a pre-existing failure as a result of the split.
7. Back up the local runtime data before exercising the new backend installer:
   - `$HOME/Library/Application Support/Bridge/bridge.db`
   - `$HOME/Library/Application Support/Bridge/token`

   The database and token must remain outside both Git repositories.

## Phase 2: create the backend repository with history

Perform history rewriting only in a disposable clone, never in the current working copy.

1. Add `/mai/` to the Android repository's `.gitignore` while the new repository is staged inside it.
2. Make a fresh clone of the current repository at `mai/`.
3. Create a dedicated migration branch at the recorded split commit. The history pushed to the new repository must end at that commit; do not accidentally include later client or gateway work from another branch.
4. Use `git filter-repo` in that clone to retain the backend and the mixed root files that will be edited afterward. The retained paths should be:

   ```text
   crates/gateway/
   scripts/mac-install.sh
   scripts/mac-uninstall.sh
   scripts/status.sh
   scripts/install.sh
   Cargo.toml
   Cargo.lock
   justfile
   README.md
   AGENTS.md
   .gitignore
   .github/workflows/ci.yml
   ```

   An equivalent history-preserving filtering method is acceptable, but it must run in the new clone only.

5. Leave the filtered clone without an origin remote so it cannot accidentally push to the Android repository.
6. Move `mai/` to its final location after local validation. Add and verify its remote only when it is ready to publish.

The filtering step preserves history for the gateway and service scripts. It is expected that the retained root files still contain client material at this point; that material is removed in the next phase.

## Phase 3: make the backend repository independent

### Rust workspace

1. Change the root workspace membership to only `crates/gateway`.
2. Remove workspace dependencies used only by `android-core`: `thiserror` and `uniffi`. Retain every dependency referenced by `crates/gateway/Cargo.toml`.
3. Keep the crate under `crates/gateway` during the migration. Flattening or renaming it can happen later as a separate change.
4. Regenerate `Cargo.lock` from the backend-only workspace.
5. Verify that no path dependency or build script reaches into the Android repository.

### Commands and scripts

1. Reduce the backend `justfile` to backend commands:
   - `install`, if the backend keeps an installation prerequisite checker
   - `dev` or `gateway`
   - `test`
   - `check`
   - `fmt`
   - `mac-install`
   - `mac-uninstall`
   - `status`
   - `logs`
   - `clean`

2. Make `test`, `check`, `fmt`, and `clean` Rust-only. Remove Bun, TypeScript, Gradle, Android, APK, and `apps/bridge` references.
3. Keep `mac-install.sh`, `mac-uninstall.sh`, and `status.sh` with the backend because they operate the gateway on the Mac.
4. If `scripts/install.sh` is retained, rewrite it as a backend prerequisite check. It should not install or manage the Android toolchain.
5. Preserve the current runtime identities and locations during this split:
   - Binary name: `bridge-gateway`
   - launchd label: `app.bridge.gateway`
   - Bind address and port: `127.0.0.1:8787`
   - Data directory: `$HOME/Library/Application Support/Bridge`
   - Log directory: `$HOME/Library/Logs/Bridge`
   - Token and database filenames
   - Tailscale Serve mapping

   Keeping these stable means reinstalling from the new repository reuses the existing database, token, URL, and Android configuration.

### Ollama boundary

1. Keep `ollama-rs` and the gateway's Ollama adapter in the backend repository.
2. Document Ollama as an external prerequisite reachable through `BRIDGE_OLLAMA_HOST` and `BRIDGE_OLLAMA_PORT`.
3. Document `BRIDGE_MODEL` and the requirement that the configured model already exists in Ollama.
4. The backend installer may check whether `ollama` is available and report its status. It must not assume exclusive ownership of Ollama.
5. The backend uninstaller must not stop Ollama, uninstall it, or delete its models. The current uninstaller already has the correct ownership behavior and should retain it.

### CI and documentation

1. Replace the combined CI workflow with a Rust-only workflow that runs, at minimum:

   ```bash
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   cargo audit
   ```

2. Remove Java, Android SDK, Bun, TypeScript, Gradle, and APK setup from backend CI.
3. Rewrite the README around backend concerns:
   - Architecture and the fact that Ollama is shared and external
   - Rust, Ollama, and Tailscale prerequisites
   - Model preparation
   - Local development
   - All supported `BRIDGE_*` environment variables
   - launchd installation, status, logs, and uninstall behavior
   - Runtime data and backup locations
   - Tailscale and bearer-token security model
   - Android repository link
   - API contract link

4. Add the canonical API contract document chosen in the decision phase.
5. Update `AGENTS.md` and `.gitignore` so they describe only the backend.

### Backend acceptance gate

Before pushing the backend default branch, run:

```bash
just fmt
just check
just test
```

Also verify the CI-equivalent Clippy and audit commands if `just check` does not include them. Confirm that `rg` finds no references to `apps/bridge`, Gradle, Android SDK, Kotlin, React Native, Bun, APKs, `android-core`, or UniFFI outside historical documentation that intentionally describes the client.

## Phase 4: reduce the current repository to the Android client

Do this on a migration branch created from the same recorded split commit. Do not remove the backend from the current repository until the new backend branch has passed its acceptance gate.

### Remove backend-owned files

1. Remove `crates/gateway/`, including its migrations.
2. Remove `scripts/mac-install.sh`, `scripts/mac-uninstall.sh`, and `scripts/status.sh`.
3. Do not remove or modify anything under `$HOME/Library/Application Support/Bridge`; repository cleanup must not touch runtime data.

### Rust and JavaScript workspaces

1. Change the root Rust workspace membership to only `crates/android-core`.
2. Remove gateway-only workspace dependencies. Retain dependencies referenced by `crates/android-core/Cargo.toml`, including the development dependency on Axum.
3. Regenerate the client-only `Cargo.lock`.
4. Keep the Bun workspace and `apps/bridge/package.json` unchanged unless a path reference requires adjustment.
5. Verify the Gradle `repoRoot` calculation and these paths still resolve from the current layout:
   - `crates/android-core/src/bridge_core.udl`
   - `crates/android-core/src`
   - `apps/bridge/android/app/src/main/jniLibs`

   The planned layout retains all three paths, so no Gradle restructuring should be necessary.

### Commands and scripts

1. Remove `dev`, `gateway`, `mac-install`, `mac-uninstall`, `status`, and `logs` from the client `justfile`.
2. Keep client commands:
   - `install`
   - `android`
   - `android-build`
   - `android-install`
   - `apk`
   - `test`
   - `check`
   - `fmt`
   - `clean`

3. Update `test`, `check`, `fmt`, and `clean` so their descriptions and behavior mention only `android-core` and the Android app.
4. Rewrite `scripts/install.sh` to check only client development requirements. Remove Ollama and the Mac-side Tailscale CLI from its optional command loop; keep `adb` and the Android toolchain checks.
5. Keep `scripts/build-apk.sh` unchanged unless repository-relative paths fail validation.

### CI and documentation

1. Keep Rust setup because `android-core` is compiled and tested in this repository.
2. Keep Bun, TypeScript, Java, Android SDK, NDK, cargo-ndk, and APK assembly jobs.
3. Remove gateway tests and backend-specific dependency setup.
4. Continue running:
   - Rust formatting, Clippy, tests, and audit for `android-core`
   - Prettier, TypeScript checking, Bun tests, and audit
   - Debug APK assembly

5. Rewrite the README as the Android client README:
   - Explain that the app requires a separately deployed Bridge backend.
   - Link to the backend repository's installation and API documentation.
   - Keep Android SDK, phone, Tailscale app, build, install, and release instructions.
   - Remove instructions that build or install the gateway from this repository.
   - Explain where users enter the backend URL and API token.

6. Update `AGENTS.md` and `.gitignore` to remove backend-only guidance and patterns while retaining credential, keystore, APK, Gradle, Node, and generated native-library protections.

### Client acceptance gate

Run:

```bash
just fmt
just check
just test
just android-build
```

Confirm that `rg` finds no active references to `crates/gateway`, `bridge-gateway`, `mac-install`, `mac-uninstall`, launchd, gateway logs, or gateway database paths except links or documentation that intentionally points users to the backend repository.

## Phase 5: cross-repository integration test

Use the migration branches from both repositories for one end-to-end test before merging either branch.

1. Confirm the shared Ollama daemon is running and the configured model is available.
2. Back up the Bridge database and token.
3. From the backend repository, build and run the gateway locally with the existing configuration.
4. Call `/v1/health` with the bearer token and confirm that gateway, database, Ollama, and model status values are returned.
5. Exercise every non-streaming route with a temporary chat: create, list, fetch, rename, and delete.
6. Exercise both streaming routes and confirm the Android core accepts the full SSE lifecycle, including thinking, text deltas, optional tool events, completion, errors, retry, and cancellation.
7. Build and install the Android debug APK from the client repository.
8. Configure it with the existing Tailscale URL and token. Verify:
   - Connection testing succeeds.
   - Existing chat history remains visible.
   - A new chat can be created.
   - A response streams to completion.
   - Web search still works.
   - Retry, rename, and delete still work.
   - Restarting the app does not lose backend configuration.

9. Run both repositories' CI workflows on their migration branches.

The integration test must use the same protocol and runtime data locations as before the split. If it requires an API compatibility workaround, fix the repository boundary first rather than introducing a new API version during migration.

## Phase 6: cutover

1. Merge and publish the backend repository first while the old monorepo still contains a compatible client and gateway.
2. Record the backend repository's initial commit and the source split commit in its README or migration commit message.
3. Install the gateway from the new backend repository. This should replace only the installed binary and launchd definition while reusing the existing database and token.
4. Repeat the health and streaming smoke tests against the installed service.
5. Merge the client-repository cleanup after the new backend installation has been verified.
6. Update repository descriptions, topics, links, and clone instructions so each repository points to the other.
7. Enable the desired branch protections and required CI checks in both repositories.
8. Move or recreate open issues only after deciding which repository owns each issue. Issues involving the API boundary should link both repositories rather than duplicating independent work.
9. Announce that future backend installation and service commands run from the backend repository, while Android build and APK commands remain in the current repository.

## Rollback plan

### If the backend repository fails before cutover

- Keep running and installing the gateway from the pre-split current repository.
- Do not merge the client cleanup branch.
- Fix the backend migration branch and repeat its acceptance gate.

### If the newly installed gateway fails

1. Stop or replace only the `app.bridge.gateway` launchd job using the established service procedure.
2. Check out the recorded pre-split tag in the original repository.
3. Reinstall the previous gateway binary from that tag.
4. Restore the database backup only if a database migration ran or data validation fails. The repository split itself must not introduce a migration.
5. Confirm `/v1/health` and a complete message stream before resuming normal use.

### If the Android repository fails after cleanup

- Revert the cleanup commit or build the app from the recorded pre-split tag.
- The installed backend and runtime data can remain in place because the API is intentionally unchanged.

Do not delete the old remote, pre-split tag, backup, or migration branches until both repositories have passed CI and the end-to-end test from their default branches.

## Ongoing coordination after the split

- Treat the backend `/v1` contract as versioned public behavior even while both repositories remain private.
- Make backward-compatible backend changes first, release them, and only then update the Android client to use them.
- For breaking changes, add a new API version and keep the previous version until the deployed Android client has migrated.
- Keep database migrations entirely in the backend repository.
- Keep UI, Kotlin, UniFFI, and Android release changes entirely in the client repository.
- Keep Ollama lifecycle independent of both repositories. The backend may depend on its API but does not own the daemon or its model storage.
- Tag client and backend releases independently. When diagnosing compatibility, record both versions.
- Add contract fixtures or an end-to-end compatibility job later if manual synchronization of Rust, UniFFI, TypeScript, and gateway types becomes error-prone. This is a follow-up improvement, not part of the initial repository split.

## Completion checklist

The migration is complete only when all of the following are true:

- [ ] The backend repository builds, tests, and runs without the Android repository present.
- [ ] The client repository builds, tests, and assembles an APK without the backend source present.
- [ ] Each repository has an accurate lockfile, CI workflow, `justfile`, README, ignore rules, and contributor guidance.
- [ ] No repository contains secrets, databases, keystores, APKs, Ollama models, or other runtime artifacts.
- [ ] The gateway continues using the existing launchd label, data paths, token, port, and Tailscale URL.
- [ ] Ollama is documented and operated as an external shared dependency.
- [ ] The `/v1` JSON and SSE contract is documented and unchanged.
- [ ] Existing chat history is available from the newly installed backend.
- [ ] The Android app connects and completes an end-to-end streamed chat.
- [ ] Both default branches pass their independent CI workflows.
- [ ] Both repositories link to each other and clearly state their ownership boundaries.
- [ ] The pre-split tag and runtime backup are retained until the new arrangement has been used successfully.
