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

## 1 — Facade retirement (CONSUMERS DONE 2026-07-23 — useServer() deleted; v1 route removal remains)

**Progress.** Landed consumer-by-consumer chunks:

- `features/deskSnapshot` (scoped bootstrap/session store + `useActiveShow`,
  `useActiveShowId`, `useBootstrapReady`, `useHardwareConnected`, `useFrameRateHz`,
  `useActiveTimecode`, `useOutputHealth`, `useBootstrapSnapshot`, `useSessionSnapshot`)
  and migration of all read-only bootstrap/session consumers (App, LeftDock, control panes,
  faders, StageCommandControls, CommandLineStatusBoundary, fixtureSheetProjection,
  useStageVisualization, GroupsWindow/useGroupPoolModel, GroupStrip).
- Eight dead `useServer()` calls removed (ChannelsWindow, CuelistWindow, useStageLayout,
  DeskLockSettingsModal, SpecialDialogsModal, specialDialogs/color, ProgrammerFadeFader,
  PlaybackTools).
- `features/highlight` (scoped Highlight store + actions context, incl.
  `setPatchPreviewHighlight`) and migration of HighlightControls, HardwareControlSummary,
  the Fixture Sheet step presenter, and PatchWindow.
- `features/programmerActions` (scoped one-shot programmer actions: undo, clear,
  control actions, preset generation, align, storePreload) and migration of the numeric
  pad, SystemControlsModal, specialDialogs/control, PreloadStoreModal, PresetsWindow,
  and the parameter-controls projection/controller/tabs.
- `features/shellStatus` actions context (`dismissError`/`simulateError`/`readServerLogs`)
  and migration of CommandLineBar and DebugModal (output health via `useOutputHealth`).
- `features/commandHistory` scoped store (CommandLineHistoryPanel) and
  `features/dmxDiagnostics` (`readDmx`/`setDmxOverride`/`outputRoutes`: DmxWindow,
  ProductDemoApp).
- `features/mediaServers` (media fixtures/previews/refresh + Matter status:
  MediaServerSetup, MatterBridgeSettings, DeskSettingsModal path) and
  `features/soundToLight` (speed-group sound calls: useSoundCapture, useSoundToLight).
- `features/fixtureLibrary` (profiles/definitions/warnings/patch layers/unresolved MVR +
  profile save/delete/revisions/GDTF/package/patch-layer calls) and migration of
  FixtureLibrarySetup, the fixtureLibrary editor/revisions/transfers/warnings panes,
  fixturePatch controller, and PatchFeatureBoundary (attribute registry via
  `useAttributeRegistry`).

**Zero consumers remain; `useServer()` and the broad ServerContext are deleted.** The final
chunks added `features/showLifecycle` (shows list + open/save/overwrite/upload/download,
named revisions, MVR transfer, desk users incl. switchUser, shutdown) and
`features/deskConnection` (server URL/desk token setters + persisted desk layout), extended
`features/dmxDiagnostics` with the output-route save/delete, and migrated QuickSetupModal,
ShowRecoveryModal, ConnectionState, LayoutPersistence, and the setup-window
controller/General/Outputs/Network sections. `ServerProvider` still composes the internal
value that feeds the scoped providers; the compatibility read surface is gone.

**API direction (maintainer, 2026-07-23):**

1. **Intentional APIs, two shapes.** Reads load whole-object state (snapshots). Writes express
   intent: pressing GO must not rewrite the playback object, a patch edit is one change
   applied immediately. Intent does not always mean action-verb unions — a typed
   partial-update route (e.g. `patched-fixture/{id}/update` whose body carries only the
   fields to change) is an equally valid intent style.
2. **No show-scoped routes.** The desk always operates on the one loaded show; cross-show
   work is only the library (listing shows, loading objects into the show). The show guard
   becomes an **optional header**: if the desk sends it, the server checks it; absent (the
   automation case) → no check. Never a URL segment.
3. **Strict but tolerant typing.** Validate every body against the typed contract and return
   a clear 4xx on mismatch — never crash. Additional/unknown properties are **accepted** and
   **logged** server-side, not rejected.
4. **Transport split.** Simple REST command endpoints (e.g. press GO on a playback) stay for
   external integrators — fire-and-forget, no request-identity machinery needed there. The
   desk's own UI should move its actions onto the already-open WebSocket
   (request_id-correlated action frames) where retry/replay dedup lives and per-request
   handshake overhead disappears.
5. **Virtual playback exclusion zones are show-level.** The storage already is
   (`read_virtual_playback_exclusion_store` keys by show only); the desk segment in the
   current route is an authentication artifact and must not suggest desk scoping — drop it
   whenever the route is next touched.

**DECIDED (maintainer, 2026-07-23): REST and WebSocket are not customer-facing for now.**
The Protocols help chapter and all operator documentation describe OSC only; the HTTP/WS
API is an internal application transport until explicitly re-published. This supersedes
"update the Protocols chapter to describe the v2 surface" below — no v2 REST/WS
documentation chapter is required.

**Remaining for §1: server-side v1 route removal.** Remove unused v1 REST/WebSocket routes
one route per chunk with a grep for remaining callers (client `api/client/*`, OSC/hardware
surfaces, desktop bridge, bench helpers under `apps/control-ui/e2e/bench` and root
`tests/`), then update the Protocols help chapter to describe the v2 surface. Note the v1
client modules (`apps/control-ui/src/api/client/*`) are still called by the composed
`ServerProvider` internals (polling, show data, actions) — each route deletion therefore
starts by migrating the corresponding internal call to v2 or deleting it with its feature.

**Deferred remainder (2026-07-23, updated):** only the per-route v1 removal sweeps and the
Protocols-chapter update remain. Each route deletion is its own chunk per the decided
policy below; the OSC API stays frozen.

### Plan

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

## 2 — Skipped-test residues (RESOLVED 2026-07-23, one deferred residue)

Four of the five scenarios are fixed and unskipped; one narrowed residue remains deferred:

- **CUE-011 @ui / @supplemental-ui — partially fixed, residue deferred.** The client-side half
  is fixed: `installAuthoritativeObjects` now installs a response carrying a strictly newer
  object revision even when a concurrent collection re-hydration already stamped the kind
  floor at the response's event sequence (unit-covered in `ShowObjectsStore.test.ts` and
  `writer.test.ts`). The remaining residue is **server-side**, narrowed by trace inspection:
  during the trigger-choice step the `cue_list` object revision advances with *no client
  request* and *no `show_object_changed` event*, so no UI can learn the new revision. The
  topology responses also echo a legacy `chaser_xfade_millis: 0` the request never sent
  (`lossless_json` merge of the stored raw body), pointing at a cue_list body-normalization
  write. Deferred: needs a dedicated server-side investigation of that silent write
  (start at `crates/application/src/playback_topology/candidate.rs` `cue_list_body` /
  `lossless_json`, and the active-show mutation path's unconditional `next_revision` in
  `crates/application/src/active_show/objects.rs` whose `show_object_changed` emit is
  caller-supplied and omittable). **Sharper lead (2026-07-23, from the §6 undo-test fix):**
  the active document runs a *normalize-once write-back* for legacy-shaped objects (groups
  provably bump one revision with no `show_object_changed` when a document read finds a
  non-normalized stored body — that is exactly the silent-revision signature). The stored
  cue_list body carries `chaser_xfade_millis: 0` from the lossless response merge while the
  serde model skip-serializes zero, so the next normalize pass rewrites the body
  byte-differently and bumps the revision silently. Reproduce by storing a cue_list with
  `chaser_xfade_millis: 0` and triggering a document read; fix candidates: emit
  `show_object_changed` from the normalize write-back, or stop the topology response echo
  from persisting the skip-serialized field.
- **PRELOAD-004 — fixed.** A playback-only Preload GO left the lifecycle projection reporting
  no active preload (queued Playback actions deliberately did not count), so hold-to-release
  never armed. A `preload_playback_active` marker (set on GO, cleared on release, outside
  undo) now feeds `has_active_preload` and the lifecycle summary. Unskipped and green.
- **PBK-005 — fixed.** The gesture wiring was correct; the test waited on the retired v1
  per-cuelist button endpoint. It now asserts the v2 desk `playback-actions` request shape.
  Unskipped and green.
- **SHOW-005 — fixed.** The recovery-backup content was correct; the test's backup selection
  was wrong: its loose `startsWith` prefix filter and lexicographic sort usually picked a
  per-mutation backup (`<name>-<show id>-show-object-<millis>-….show`, taken *before* the
  tracked edit) instead of the overwrite backup (`<name>-<millis>.show`). The test now matches
  the overwrite naming exactly and sorts numerically. Unskipped and green.
- **PLAYBACK-SELECT-001 — fixed.** Two silent UI failures: the fader bank tore its grid down
  during transient projection refetches (now stays mounted once rendered; only a
  topology-scope reset returns to the loading placeholder), and the Playback pages menu gated
  its close on operation-scope currency, staying open after a successful page change and
  intercepting later clicks (a successful change now always closes). Unskipped and green.

## 3 — DMX-006 re-authoring + a schema decision (RESOLVED 2026-07-23)

**Done.** `installSixteenBitMatrix` now authors complete schema-v2 profile snapshots (u16
resolution, secondary slots for both byte layouts, `invert`, `default_raw`); validation
accepts a secondary slot numerically below the coarse slot (fine-first layouts derive the
coarse slot around the reserved fine slot, as designed). The sunstrip virtual-dimmer
expectation was updated to the schema-v2 engine's committed math (colour quantizes to raw
first, then the virtual-intensity scale multiplies once in f64 — a 1-LSB difference vs the
legacy scale-then-quantize path at f32 half-way points, already pinned by the engine
guardrail). DMX-006 @api and @ui are unskipped and green. DMX-008 remains a separate,
genuinely unimplemented output feature (unchanged below).

### Original notes

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

## 4 — Playback runtime telemetry: sampled push at ~10 Hz + client-retained store (BUILT 2026-07-23)

**Done.** Server: `PlaybackTelemetrySampler` (crates/server/src/runtime/playback_telemetry.rs)
counts completed render frames in both the real output scheduler and the test bench, and on
every Nth frame (`telemetry_frame_divider`, the divisor of the configured rate nearest 10 Hz;
44→4, 40/100/120→10 Hz, unit-tested) samples `Engine::playback_telemetry_at` — a read of
already-published playback runtime state (fade progress from activated_at ÷ cue completion,
master, current cue, flash/temporary/swap/enabled) computed in crates/playback
(`telemetry_samples_at`). Delta detection (`PlaybackTelemetryDeltas`) drops unchanged
playbacks; no change → no message. Ticks publish as `EventClass::Telemetry` /
`DeliveryPolicy::Replaceable` drafts on the v2 events bus through the render unit-of-work
(never touching the timing-critical render path itself), routed via the shared
`playback:telemetry` object. Wire types live in crates/wire v2 playback with regenerated
contracts. Client: the playback events subscription adds the telemetry class+object; decoded
ticks land in a desk-lifetime `telemetry` map on the PlaybackRuntimeStore (retained across
window mounts, no snapshot fetch — unit-covered), exposed via `usePlaybackTelemetry` /
`usePlaybackTelemetryMap`. Paced acceptance: `tests/29-playback-telemetry.spec.ts` activates
two playbacks with 4 s fades, drives one simulated second of 44 Hz frames, and asserts ≈11
sampled WS ticks with monotonically rising mid-fade progress, zero snapshot polling after
hydration, and full silence once the fades settle.

Consuming the retained store from concrete playback surfaces (lighting buttons, fade bars)
remains ordinary feature work on top of `usePlaybackTelemetry`.

### Original requirement

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

- ~~Pre-existing failures~~ **RESOLVED 2026-07-23** — both were stale tests, not bugs:
  `active_object_undo…` seeded legacy-shaped groups directly into the show file *before*
  opening it, so the deliberate normalize-once-on-open pass (covered by SHOW-004) bumped
  their revisions past the test's hard-coded If-Match values; the test now seeds after
  open and writes the already-normalized group shape. The `VirtualPlaybacksWindow` vitest
  asserted no runtime subscription while authority loads, contradicting the component's
  documented design (the subscription *is* the activation mechanism; gating it deadlocked
  the pane) — only rendering waits for authority. Both unit suites are now fully green
  (vitest 1981/1981, cargo 0 failures).
- `major-refactoring.md` §8 lists "move giant inline server tests into feature-local unit
  tests" — `crates/server/src/runtime/tests/` still holds ~80 modules; migrate opportunistically
  when touching a feature, not as a big-bang.
- Source-size goals (non-blocking ratchet): 144 files > 400 lines, 5922 functions > 20 lines.
