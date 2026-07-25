# Modular Desktop Host

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
