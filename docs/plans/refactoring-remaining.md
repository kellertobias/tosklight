# Refactoring — remaining steps

The refactoring execution plan is complete and archived at
[`Done/major-refactoring-execution.DONE.md`](./Done/major-refactoring-execution.DONE.md)
(suite 267 passed / 19 documented skips; the only failing test is the user-dirty
`product-demo` run). The architectural intent stays in
[`major-refactoring.md`](./major-refactoring.md). This file lists what is genuinely left,
in suggested order. Each chunk lands independently.

## Working rules (carried over)

- Read `AGENTS.md` first. `docs/help/**` is operator truth; `docs/testing/**` + root `tests/`
  are the acceptance contract; read `docs/acceptance-criteria.md` before persisted-data changes.
- Small chunks: smallest relevant check first, then the full `tools/test.sh e2e` per chunk.
  Land only with no net new regressions. `cargo fmt`; do not push unless asked.
- The bench is flaky (worker crashes, "N did not run"); re-run a suspected failure in isolation
  before treating it as real. Known flaky-in-suite: FIXTURE-002 @restart, TIME-002 @ui,
  GROUP-005 @supplemental.
- When a UI action "silently no-ops", first suspect a scoped-store scope/generation guard, a
  stale show-revision `If-Match`, or a v1 mutation path that never publishes its v2 event —
  those three patterns explained almost every "unimplemented feature" skip so far.

## 1 — Facade retirement (the main remaining architecture work)

`useServer()` was retained as the sanctioned migration facade
(`major-refactoring.md` §"Remove compatibility facades") and still has **~53 non-test
consumers**; the v1 client surface (`apps/control-ui/src/api/client/*`, ~36 `/api/v1` calls)
backs it. Retire it consumer-by-consumer:

1. Inventory the 53 files by which facade fields they read (`bootstrap`, `fixtureProfiles`,
   `configuration`, session, one-shot reads). Most already sit next to an existing scoped
   feature (`features/patch`, `features/showObjects`, `features/visualizationRuntime`,
   `features/configuration`, `features/server/*` slices).
2. Migrate read-only consumers onto the narrow feature hooks; add small scoped hooks where a
   field has no owner yet (pattern: `useVisualizationRuntimeRead`, added for the last
   `readVisualization` readers).
3. Only after every caller is migrated: delete `useServer()`, then remove the now-unused v1
   REST/WebSocket compatibility routes server-side (each route deletion is its own chunk with
   a grep for remaining callers, including OSC/hardware surfaces and the desktop bridge).
4. Gate every chunk with `node tools/check-architecture.mjs`, `check-source-size.mjs`,
   `test-command-boundaries.mjs` and the full suite.

**DECIDED (maintainer, 2026-07-23):** the desk is prerelease with no external users. Any
external API may change or be deleted **except explicitly defined contracts — specifically
the OSC API**, which stays frozen (`docs/help/50-Protocols/01-osc-rest-and-websocket.md`,
OSC sections; keypad/command contract unchanged). v2 becomes the HTTP/WS API; delete v1
routes as their last caller migrates, and update the Protocols help chapter to describe the
v2 surface once v1 is gone. The e2e bench driver may keep using an endpoint only while it is
still served — migrate bench helpers alongside route deletions.

## 2 — Skipped-test residues (bugs to finish, one investigation each)

Each was re-verified 2026-07-22 after the scoped-store fixes; skip reasons in the spec files
are current and precise.

- **CUE-011 @ui / @supplemental-ui** (`tests/06-cuelist-view-and-settings.spec.ts`): after a
  retried topology save applies, the Cuelist window header keeps showing the previous
  revision, and one *silent* extra `cue_list` revision (unchanged body, no
  `show_object_changed` event) appears while the Settings/Renumber dialogs are exercised.
  Instrument the ShowObjects install path after `PlaybackTopologyWriter`'s 409-retry, and find
  the server write that bumps the object revision without an event.
- **PRELOAD-004 @ui / @supplemental-ui** (`tests/preloadVirtualPlaybackContracts/`): after
  PRELOAD GO applies the queued actions, the command bar's preload lifecycle view goes stale —
  hold-to-release finds no active scene and the label never returns to `PRELOAD`. Look at the
  preload lifecycle store after a GO issued from the same desk.
- **PBK-005 @supplemental-ui** (`tests/playbackConfiguration/pbk005.ts`): the held-Swap /
  toggled-Temp interaction never issues its playback action request (`page.waitForRequest`
  times out). Verify the press/hold gesture wiring on the playback card.
- **SHOW-005 @ui** (`tests/05-…revision-copy-tests.ts`): the on-screen revision-copy identity
  shows the group name (`"Center Spot|2"`) instead of the expected
  `destination-before-overwrite|N` label. Decide whether the UI or the expectation is wrong
  against `docs/help` and fix accordingly.
- **PLAYBACK-SELECT-001 @supplemental-ui** (`tests/28-…spec.ts`): the hardware-connected
  fader-bank slot header never becomes stably clickable, so the ownership checks cannot start.
  Layout-stability investigation in the hardware-connected layout.

## 3 — DMX-006 re-authoring + a schema decision

The D3 (derive-only) schema work landed: identity checks compare the raw profile, SHOW-004
virtual-dimmer-metadata is green, and an engine guardrail pins the intensity×colour one-way
multiply. DMX-006 (`tests/03-network-output-protocols.spec.ts`) remains skipped because the
scenario still mutates the *derived* heads (16-bit component layouts, byte order, parameter
defaults), which the schema-v2 engine deliberately ignores in favour of the raw profile
snapshot. To finish:

1. **DECIDED (maintainer, 2026-07-23): no byte-order concept.** The schema model stands as
   built: the channel is the attribute's *coarse* (most significant) byte; explicit
   `secondary_slots` are its fine → ultra bytes and may be non-adjacent; secondary slots are
   exclusive (never reusable by another channel) and coarse-slot derivation renumbers around
   them when channels are reordered. A fixture whose fine byte sits at a lower DMX address
   than its coarse byte is expressed purely by slot assignment (fine at the lower slot,
   coarse deriving to the higher one) — the legacy `byte_order: lsb_first` flag on derived
   definitions is v1 vocabulary, not a schema gap. Internally the engine keeps computing at
   full resolution (u32 raw / float); resolution only matters at DMX encoding. While
   re-authoring, verify profile validation accepts a secondary slot numerically below the
   coarse slot (the derivation already supports it).
2. Re-author `installSixteenBitMatrix` to build proper schema-v2 profile snapshots (u16
   resolution, secondary-slot placement for both byte layouts, `invert`, `default_raw`)
   instead of mutating derived heads, then unskip DMX-006.
3. **DMX-008** stays separate: its minimum-universe-size padding/defaults output contract
   (idle zeros to `minimum_slots`, patched-default inclusion, disable-without-delete) is a
   genuinely unimplemented engine/output feature, not a schema issue.

## 4 — Playback runtime telemetry: sampled push at ~10 Hz + client-retained store

**Requirement (maintainer, 2026-07-23).** Fetching playback data currently takes too long on
the client; fast-changing runtime values should be *sampled server-side and pushed* to the
desk instead of request/response loading. Scope:

- The server samples the playback section on **every Nth completed render frame, with N
  chosen as the divider of the configured output rate nearest ~10 Hz** (44 Hz → N=4 ≈ 11 Hz;
  40 Hz → 10 Hz; 100 Hz → 10 Hz; 120 Hz → 10 Hz). Deriving the tick from the frame counter —
  not a wall-clock timer — keeps samples frame-coherent, avoids beating against the output
  clock, adds no timer wakeups, and keeps the telemetry rate stable across output-rate
  configurations. It pushes only the **changing** values: fade progress (how far into a
  fade), the playback's dimmer/master value, the
  current cue step, and button pressed state (so the UI can light the button). Static
  topology (names, slot layout, configuration) stays on the existing snapshot + revisioned
  event path — the telemetry lane carries only volatile runtime samples and must not touch
  the timing-critical render loop (derive samples from already-published engine state).
- Delta-oriented: a sample tick carries only playbacks whose sampled values changed; no
  change → no message.
- The client keeps a desk-lifetime **store** of this state (extend
  `features/playbackRuntime`'s store/session): a window that mounts subscribes and renders
  from the retained store immediately, applying updates — it must not reload the full
  snapshot on every open. Snapshot only on first hydration or repair.
- Transport: a lane on the existing v2 events WebSocket (`/api/v2/events`) fits; a separate
  sampled-telemetry subprotocol is fine too if backpressure demands it. Follow the existing
  scoped store/session/transport pattern (store + reference-counted session + typed
  transport), and keep the wire types in `crates/wire` with regenerated contracts.
- Verify with a paced test: activate two playbacks, run a fade, assert the client store
  observes progress samples at roughly 10 Hz without polling requests, and that a
  freshly-mounted window renders from the retained store without a snapshot fetch.

## 5 — Deferred UI features (D1: build when prioritized, not refactoring debt)

Skipped-with-reason, engine/`@api` green; these are feature builds:

- MANUAL-019 reworked surfaces: Cues responsive editor, Outputs route editor, DMX monitor
  rework, Stage scenery model, Shows & recovery browser.
- SOUND-001: browser audio analyzer driving Sound source selection.
- COLOR-RANGE-001: shift-drag Color range apply.
- On-screen Speed Group per-group stack controls beyond the current BPM buttons, and PRELOAD
  controls on virtual-playback cells.

## 6 — Housekeeping candidates (verify before starting; likely small)

- Pre-existing failures unrelated to the refactoring, failing identically at the branch base:
  `active_object_undo_is_lossless_atomic_contextual_and_failure_safe` (server unit; PUT
  returns 409 where 200 is expected) and one `VirtualPlaybacksWindow` vitest ("does not render
  a seeded desk before scoped runtime authority is ready"). Diagnose or record as accepted.
- `major-refactoring.md` §8 lists "move giant inline server tests into feature-local unit
  tests" — `crates/server/src/runtime/tests/` still holds ~80 modules; migrate opportunistically
  when touching a feature, not as a big-bang.
- Source-size goals (non-blocking ratchet): 144 files > 400 lines, 5922 functions > 20 lines.
