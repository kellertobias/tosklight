# Modular Desktop Host

## Status

Finished. Plans 07–13 have recorded blocking dependencies; this was the next independent plan. Its
Rust/Tauri host modules, tests, packaging, and desktop lifecycle did not overlap the active
UI-library/Storybook component lane.

## Goal

Leave `apps/light-desktop/src-tauri/src/main.rs` as a clear composition root.

Estimated effort: 0.25–0.5 Codex day.

## Required work

1. Characterize server supervision, sibling-app launch, display/window commands, menus, quit
   policy, restart behavior, and packaged resource paths.
2. Extract cohesive modules for server supervision, windows/displays, menu/quit lifecycle, and
   Hardware Controls launching.
3. Keep Tauri command payload validation and thread/process ownership explicit.
4. Add focused pure/adaptor tests where lifecycle policy can be isolated.

## Acceptance and verification

- `main.rs` configures plugins, shared state, commands, menus, and lifecycle modules without
  containing their policies.
- Debug and packaged applications preserve sidecar paths, readiness, launch, quit, relaunch,
  multi-display, and sibling-app behavior.
- Rust tests, formatting/Clippy, desktop smoke, archive build, and real packaged launch pass.

## Result

### Changes

- Reduced `main.rs` from 379 lines to a 31-line composition root that registers commands and
  delegates setup, menu events, and runtime events.
- Extracted cohesive server-supervision, window/display, lifecycle, menu, and Hardware Controls
  launcher modules while preserving command names, payloads, event names, window labels/URLs,
  sizing, display placement, and native menu behavior.
- Replaced the process-global quit flag with Tauri-managed `QuitState`; the first Cmd+Q still emits
  `quit-requested`, cancellation disarms it, and the next armed request emits
  `app-shutting-down` and exits.
- Preserved server bind selection, the 120 ms TCP probe, 60-second startup deadline, one-second
  restart checks, log/data/resource paths, exact child arguments, external-server reuse, and the
  distinct test/debug frontend URL injection behavior.
- Added pure tests around quit policy, server address/binary selection, and console-window bounds,
  plus an architecture guard that keeps desktop policy out of `main.rs`.

### Tests

- `cargo test -p light-desktop` — 5 passed.
- `cargo clippy -p light-desktop --all-targets -- -D warnings` — passed.
- `cargo fmt --all -- --check` — passed.
- `npm test --prefix apps/light-desktop -- tauriDesktopBridge.test.ts` — 3 passed.
- `node tools/check-architecture.mjs` — passed, including the new desktop composition-root guard.
- Native `aarch64-apple-darwin` release app bundle built with the repository Tauri artifact layout;
  the already-built frontend artifact was reused to avoid reading the concurrently changing
  Storybook/frontend source tree.
- `GITHUB_ACTIONS=true LIGHT_DESKTOP_SMOKE_TARGET=aarch64-apple-darwin node
  tools/ci-smoke-built-desktop.mjs` — the packaged app with its bundled release server stayed
  alive for the five-second launch probe.

### Limitations

- The complete cross-platform `npm run bundle` archive matrix was not run locally because it also
  builds unrelated hardware, Windows, Linux, universal-server, and concurrently changing frontend
  deliverables. The native release app bundle and CI-equivalent packaged launch path passed.
- The broader `npm run test:architecture` reached and passed the dependency-direction checks, then
  failed on a new 177-line `TouchEncoder` function in the concurrently edited UI-library lane. The
  desktop architecture check itself passes and none of this plan's files participate in that size
  violation.
- Manual window placement, Cmd+Q confirmation, and Hardware Controls launch were not exercised by
  hand. Their payloads and platform paths are preserved structurally, and the isolated policy
  tests cover the extracted pure behavior.
- Server readiness intentionally remains the existing TCP-open probe rather than an HTTP readiness
  request.
- The pre-existing macOS debug Hardware Controls lookup still searches for an ancestor named
  `target`; installations under `~/Applications` remain its fallback. Correcting that path is a
  separate behavior change.

### Commit

`refactor(desktop): modularize tauri host` (this implementation and plan move).
