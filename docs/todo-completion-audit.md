# TODO completion audit

This audit maps every `TODO.md` area to authoritative implementation and executable evidence. It is
updated together with the final production gates; prose alone is not considered completion proof.

| Area | Implemented evidence | Automated evidence |
| --- | --- | --- |
| Group masters | `GroupDefinition` fader relationship; normalized, per-logical-head scaling before encoding; HTP maximum across assigned masters; transient flash state is combined with the fader by `max` and never changes the fader value | Engine tests for 50/75% arbitration, unassigned groups, sibling-head isolation, transient 100% flash and restored output; Playwright verifies 25% fader, held flash output and unchanged fader |
| CITP media servers | Fixture-profile capability, parent-owned patch endpoint, inherited heads, MSEX 1.0–1.2 negotiation, GETh/EThn, RqSt/StFr, fragment validation, bounded LRU caches, REST status/media, Setup preview/offline UI | Media protocol fixtures for modern/legacy negotiation, thumbnails, preview, fragments, malformed data and eviction; server endpoint/cache test |
| Empty/template groups | Empty `GroupDefinition`, portable `programming`, session `group_values`, preset `group_values`, cue `group_changes`, dynamic `group_ids` | Engine tests prove stored and session group intent becomes active after later membership |
| Membership changes | Live resolution for group programmer/preload/presets/cues/dynamics, logical-head expansion, revision history/undo, explicit detach and replacement confirmation | Active cue survives recompilation and gains members; show undo test; selection refresh tests |
| Selection macros | Ordered vectors, odd/even/every-Nth with explicit zero-based offset, live derived expressions, recursive cycle rejection | Programmer deterministic-order, derivation-update and cycle tests; invalid-rule snapshot validation |
| Frozen groups | Static expression, repeated `GROUP id GROUP id` grammar, long-press menu, persisted frozen metadata and explicit refresh | Server grammar test proves frozen membership remains fixed and live membership refreshes |
| Alignment | Left/right/center/out over ordered normalized values, discrete rejection, unsupported list, physical metadata, shortest wrap, inversion and curves at encoding | Server wrap/order tests and fixture encoder inversion/curve tests |
| Preload storage | Separate pending/active fixture and group values, arbitrary revisioned preset/cue targets, active-preload fallback when Store is pressed with an empty pending programmer, explicit store modes, idempotent pending clear | Programmer isolation test, server preset revision tests, exact 4-dimmer/6-profile/16-RGBW template-show scenario, and Playwright active-scene/store/clear-to-DMX path |
| Patch and control surface | Equal 20-key numpad, patch `Set` flow, fixture-name/address/location/rotation editors, intensity identify and lamp cycling, one merged patch header, six encoder slots, full-height programmer/cue fade masters, page controls and five narrow speed groups | Playwright patch/Store/speed workflow, stable control-frame and desktop/tablet touch-size paths; frontend encoder and fixture-library tests |
| Pools, desks and stage | Groups are a regular pool rather than a built-in; presets/stage/fixture shortcuts share visible empty 142x94 cells; empty group taps are inert until Store; desk empty-cell picker, whole-tile title dragging, touch selects and confirmed desk deletion; 2D/3D live/preload display and selected 3D outline | Playwright built-in reachability, 40-cell pool, Stage gesture, responsive layout and preload paths; unit tests for stage selection, 3D outline and live/default fixture visualization |
| Store, live input and dirty state | Short/long Store behavior, Escape/Clear cancellation, family-filtered presets, group merge/overwrite, cue append/create/active merge, absolute revision-free live commands with one-second user locks, and persisted-object-only yellow dirty state | Server command tests prove stale live revisions, same-user cross-session ownership and other-user rejection; Playwright proves Store/Escape, successful preset persistence, dirty/Save lifecycle and output behavior |
| Operator and diagnostics | User-owned programmer shared across sessions, default Operator login, Change User/create user flow, unified icon/time/show button, Debug event log and hardware/error simulation | Programmer and show-store user/session tests; Playwright Debug/hardware simulation and dock/menu paths |
| Virtual Playback exclusion zones | Inert Shift selection; named per-show/per-desk/per-surface zone storage; editable and hidden cell memberships; overlapping-zone union; serialized activation; deterministic restart normalization; independent full-override release | `VPB-007` paired UI/API Playwright coverage exercises touch, F-key, REST, OSC, concurrent activation, DMX, audit, restart, reload, and different-desk isolation; focused React and engine LTP tests |
| Manual-review software corrections | Desktop/desk terminology split; production pane-header and picker contracts; Cues/Help/DMX/Stage responsibility boundaries; versioned output routes; MIB selected-cell editing; diagnostic-only Development access; safe-blackout recovery; legacy pane defaults; refreshed screenshots and manual source | `MANUAL-019`, `FILE-001`, `FILE-002`, `FILE-016`, `TEXT-001`, `TEXT-015`, and `LOCK-001` Playwright coverage; focused File Manager, Text Editor, picker, output-route, recovery, and reducer unit tests; rebuilt PDF manual |
| Fixture channel configuration | Desk-wide atomic schema-v2 fixture-profile revisions; portable embedded show snapshots; shared Create/Edit and nested mode editor; ordered modes, heads, splits, and channels; exact multi-byte encoding; prioritized functions and typed actions; calibrated color and geometry; independent split/multipatch behavior; explicit legacy migration and recovery | `FIXTURE-001` paired API/UI Playwright coverage; focused `light-fixture`, engine, server, show, fixture-model/editor, library-revision, patch-control, Stage, screenshot, manual, and desktop-smoke coverage |
| Highlight corrective interaction | PREV/NEXT/ALL operate on the real ordered programmer selection with wrap and live-source restoration; HIGH is independent; Fixture Sheet preserves subdued base and prominent current state; software keypad geometry and the restored hardware simulator geometry match the contract; capture paths and status panels are removed; alerts remain top-layer | `HIGHLIGHT-001` through `HIGHLIGHT-003` paired API/UI Playwright coverage; focused programmer, fixture, engine, server REST/OSC/WebSocket, control component, Fixture Sheet, hardware-layout, help-screenshot, manual, and desktop-smoke coverage |

## Release gates

The completion gate is all of the following from a clean invocation:

```sh
cargo fmt --all --check
cargo test --workspace --no-fail-fast
cargo clippy --workspace --all-targets -- -D warnings
cargo check -p light-control-ui
cd apps/control-ui
npm run typecheck
npm test -- --run
npm run build
npm run test:e2e
```

The 64-universe desktop benchmark must also report `PASS`. Raspberry Pi capacity remains a
hardware-specific measured profile, not a software TODO that can be truthfully certified on macOS.

Last full audit: 2026-07-12. All listed gates passed: 80 Rust tests, 28 frontend tests, seven
Playwright end-to-end paths, strict Clippy, production web build, desktop application check, and the
64-universe benchmark. The end-to-end programmer path registers the audit receiver, verifies the
typed command event, confirms rendered DMX values, and proves a group flash does not move its fader.

The template-show scenario patches four named front dimmers, six 16-bit pan/tilt RGB profile movers,
and sixteen RGBW pars without native dimmers. It verifies empty group-relative preset/cue intent,
assigned group masters, virtual-dimmer white output, blind preload isolation, immediate Preload GO,
active-preload cue storage, cue recall while another preload is pending, SQLite backup/reopen, and
automatic cue expansion after two profiles are patched and added to the live group.
