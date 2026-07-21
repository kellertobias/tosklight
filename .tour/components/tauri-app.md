---
slug: tauri-app
title: Tauri Desktop Apps
summary: "Two sibling desktop applications and the typed DesktopBridge port between frontend and native."
order: 30
---

# Tauri Desktop Apps

Two desktop applications, siblings rather than host and pane.

| App | Path | Identifier |
| --- | --- | --- |
| ToskLight | `apps/control-ui/src-tauri/` | `de.tokenet.light` |
| ToskLight Hardware Controls | `apps/hardware-controls/src-tauri/` | `de.tokenet.tosklight.hardware-controls` |

`AGENTS.md`: a dedicated Tauri surface requested as a separate app stays a sibling desktop app
launched from ToskLight, not an embedded pane.

## ToskLight host

`apps/control-ui/src-tauri/src/main.rs`, around 352 lines:

- Launches and supervises the sibling `light-server` process. The app does not embed the server.
- Owns native windows, including one per configured operator screen.
- Exposes `open_hardware_controls` to launch the sibling app.
- Exposes only the typed desktop bridge the frontend needs.

`tauri.conf.json`: productName `ToskLight`, decorationless 1440×900 window, devUrl
`http://127.0.0.1:4175`, bundles `assets/fixture-library/`. Alongside it: `Cargo.toml`, `build.rs`,
`Info.plist`, `capabilities/default.json`, `icons/`, `gen/`.

## Hardware Controls host

`apps/hardware-controls/src-tauri/src/main.rs` is 22 lines. The work is in `src/osc.rs` (231 lines):
native UDP OSC, which is why this app needs to be native at all. devUrl `http://127.0.0.1:4176`.

## DesktopBridge

`apps/control-ui/src/platform/desktop/`:

| File | Role |
| --- | --- |
| `types.ts` | The `DesktopBridge` port |
| `tauriDesktopBridge.ts` | Tauri adapter |
| `browserDesktopBridge.ts` | Browser adapter, also used by tests |
| `index.ts` | Selects the adapter by probing `__TAURI_INTERNALS__` |
| `DesktopContext.tsx`, `useScreenWindowPersistence.ts` | React binding and screen persistence |

This port is why the whole UI is testable in a plain browser. Frontend code talks to
`DesktopBridge`, never to `@tauri-apps/api` directly. A new native capability means a new method on
the port plus both adapters, including a working browser fallback.

## Screen roles

The host opens one native window per configured screen (`ScreenApp` via `?screen=`). Sessions carry
a primary or secondary role: only the primary owns session creation and destruction, so closing a
second monitor's window cannot tear down the desk. See
`apps/control-ui/src/features/session/ownership.ts`.

## Build and run

```sh
./dev                 # cargo-run server plus tauri dev with UI hot reload
./build open          # debug builds of both apps and the server, then open ToskLight
./build archive       # release bundles; `install` also installs to ~/Applications
./test desktop-smoke  # macOS packaged-app process integration
./test app-icons      # required icon set for both apps
```

`./build open` is the required path when operator-visible behaviour changed. It stops old instances,
builds both apps and the server, copies the server binary into
`ToskLight.app/Contents/MacOS/light-server`, registers the launchd job
`de.tokenet.tosklight.dev-server`, waits for readiness, and verifies the launchd PID owns that
readiness endpoint.

After launch:

```sh
curl -fsS http://127.0.0.1:5000/api/v1/readiness
```

Check `.artifacts/runtime/light-data/light-server.log` first for startup problems. If readiness is
healthy but the app looks stuck, time `/api/v1/readiness`, `/api/v1/health`, and
`/api/v1/bootstrap` separately.

If the app looks stale, verify which bundle the build script opened before reworking UI code.

`tools/write-tauri-artifact-config.mjs` emits per-app config overrides into
`.artifacts/tmp/tauri-*-artifacts.json` so Tauri writes into the artifact tree.

## Read first

1. `apps/control-ui/src/platform/desktop/types.ts`
2. `apps/control-ui/src/platform/desktop/index.ts`
3. `apps/control-ui/src-tauri/src/main.rs`
4. `apps/control-ui/src-tauri/tauri.conf.json`
5. `apps/hardware-controls/src-tauri/src/osc.rs`
6. The repository-root `build` script, `build_debug_and_open()`

## Hardware Controls frontend

| Path | Role |
| --- | --- |
| `src/App.tsx` | Composition root |
| `src/transport/oscBridge.ts` | Bridge port |
| `src/controller/feedbackReducer.ts` | Pure idempotent feedback state |
| `src/controller/useHardwareController.ts` | Subscription and lifecycle |
| `src/oscPaths.ts` | Canonical and legacy OSC paths, both of which must keep working |
| `src/surfaces/` | Playback, Programmer, grid, settings surfaces |

The app plus the desk it attaches to form one desk with one shared command line.
