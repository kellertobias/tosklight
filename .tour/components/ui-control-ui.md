---
slug: control-ui
title: Control UI
summary: "The main operator interface: React 19 + Vite + TypeScript, running as both the Tauri desktop app and in a browser."
order: 10
---

# Control UI

`apps/light-desktop/` — React 19, Vite, TypeScript. Around 866 source files. Runs as the Tauri desktop
app and in a browser.

It presents authoritative desk and show state and turns operator gestures into typed actions. It is
not an authority: every value comes from a server projection, and every change goes through a typed
action reconciled against an authoritative event.

## Entry points

| Path | Role |
| --- | --- |
| `src/main.tsx` | React root. Picks the variant from the URL: default `App`, `?screen=` → `ScreenApp`, `?ui-kit=1` → `UiKitCatalog`, `?demo=product` → `ProductDemoApp`. Creates the `DesktopBridge`, mounts `DesktopProvider`, imports global CSS. |
| `src/App.tsx` | `ServerProvider` → `AppProvider` → `AppShell` plus overlays. |
| `src/ScreenApp.tsx` | Secondary-monitor root, one native window per screen. |
| `src/types.ts` | `AppState`, `BuiltInWindow`, pane and desk types. |

## src/api — transport boundary

The only place allowed to touch generated wire DTOs, enforced by `tools/check-architecture.mjs`.

- `api/generated/light-wire.ts` — generated, checked in, self-contained. Do not hand-edit.
- `api/client/` — HTTP bindings per capability (`desk.ts`, `playback.ts`, `programming.ts`,
  `showObjects.ts`, `selectiveImport.ts`, and others), assembled by `api/LightApiClient.ts`.
- `api/*Wire.ts`, `api/*Projection.ts`, `wireValidation.ts` — decode and validate untrusted
  responses, then map to view models.
- `api/types/` — hand-owned frontend view types, not aliases of wire DTOs.
- `api/*Transport.ts` — typed WebSocket and HTTP event transports per feature.

## src/features — bounded contexts

The slice shape is `contracts.ts` / `store.ts` / `session.ts` / `transport.ts` / `*View.tsx` /
`testFixtures.ts`.

Read `features/showObjects/` first. It is the reference: authoritative cache with revisions and
watermarks, optimistic mutations, hydration independent of socket readiness, gap recovery.

Migrated slices: `playbackRuntime/`, `playbackTopology/`, `patch/`, `programmerValues/`,
`programmerPreloadValues/`, `programmerLifecycle/`, `programmerCaptureMode/`,
`programmerPreloadPlaybackQueue/`, `programmingInteraction/`, `cueRecording/`, `groupRecording/`,
`presetRecording/`, `virtualPlaybackZones/`, `selectiveImport/`, `files/`, `screens/`,
`session/ownership.ts`.

## Composition and shared connection

`src/api/ServerRuntime.tsx` and `src/features/server/` compose the stable connection, explicit API
capabilities, and focused providers. There is no production `useServer()` consumer.
`src/api/ServerContext.ts` is retained only as a contract for legacy test mocks.

`useServerFeatureStores.ts` instantiates external stores outside broad React render paths, so a
scoped event does not rerender unrelated consumers. Shared connection infrastructure belongs here;
new domain authority still gets its own feature slice.

## Components, windows, state

- `components/shell/` — `AppShell`, `DeskGrid`, `Pane`, `LeftDock`, `WorkspaceView`,
  `ScreenWindowManager`, `LayoutPersistence`. Workspace layout only, never an authoritative show or
  runtime store.
- `components/control/` — command line, numeric pad, faders, parameter controls, playback fader
  bank, sound-to-light.
- `components/setup/`, `components/modals/`, `components/files/`, `components/input/`.
- `windows/` — one module per pane: `CuelistWindow`, `FixtureSheetWindow`, `GroupsWindow`,
  `PresetsWindow`, `PatchWindow`, `DmxWindow`, `ChannelsWindow`, `StageWindow` (with `stage3d/` and
  `stage3dScene/` three.js rendering), `FileManagerWindow`, `TextEditorWindow`, `SetupWindow`,
  `VirtualPlaybacksWindow`, `HelpWindow`, `DevelopmentWindow`. `WindowRegistry.tsx` maps kinds to
  components.
- `state/` — UI and workspace state only. `AppContext.tsx`, `appReducer.ts` composing seven
  `reducers/*`, pane geometry persisted to `localStorage`.

## Rules

- **Mounted views activate work.** An inactive pane performs no hydration, opens no socket,
  subscribes to no selectors, and does no visualization polling or hardware-listener work.
- **No stale fallback.** While scoped authority loads, show loading. Never present stale bootstrap
  values as authoritative empty state.
- **Optimistic then reconcile.** Overlay keyed by request identity, rollback on failure, narrow
  repair rather than broad refresh. Handle either response or event arriving first.
- **Reject late work** after a server, session, or show authority replacement.
- **Immediate or explicit.** Update visible state promptly, show a pending state, or open a progress
  modal.
- **Literal acceptance criteria.** Wording, geometry, placement, sizing, alignment, and visibility
  are specified behaviour. Preserve parity between software-only and hardware-connected layouts.
- Touch targets suit a desk surface; hover is never required to complete an action.

## Read first

1. `src/main.tsx`
2. `src/App.tsx`
3. `src/features/showObjects/{contracts,store,session,transport}.ts`
4. `src/api/client/transport.ts`
5. `src/api/ServerRuntime.tsx` — capability composition
6. `src/features/server/useServerFeatureStores.ts` — shared store activation
7. `src/components/shell/AppShell.tsx`
8. `src/windows/WindowRegistry.tsx`
9. `src/state/appReducer.ts`
10. `src/platform/desktop/types.ts`

## Testing

Vitest unit and component tests sit next to sources (`*.test.ts(x)`, jsdom and Testing Library).
Operator behaviour is proven at the Playwright layer.
