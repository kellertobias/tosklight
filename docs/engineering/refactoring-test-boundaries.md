# Refactoring Test-Boundary Inventory

This inventory is the Stage 1 baseline for [the major architecture refactor](../plans/major-refactoring.md). It records what each root Playwright specification drives and what it observes before transport and application boundaries move. The goal is to preserve operator behavior while replacing tests that reach through production boundaries with tests against explicit public contracts.

## Boundary vocabulary

Action boundaries describe how a test causes change:

- **UI** — visible pointer, keyboard, touch, or pane interaction in the production control UI;
- **HTTP v1** — authenticated REST mutation, including show-object writes and test-only control routes;
- **legacy WS** — `ApiDriver.command()` and the string-plus-JSON WebSocket command interface;
- **OSC** — the public `/light/...` hardware and feedback contract;
- **bench** — virtual time, process lifecycle, receiver, or fault-injection controls supplied by the E2E bench; and
- **desktop** — the packaged Tauri process boundary.

Observation boundaries describe where a test decides whether behavior is correct:

- **UI projection** — roles, labels, geometry, styling, screenshots, or recorded video;
- **HTTP projection** — REST reads, bootstrap state, revisions, diagnostics, or audit history;
- **wire event** — WebSocket or private event payloads;
- **OSC feedback** — the documented hardware feedback stream;
- **output wire** — captured Art-Net or sACN packets; and
- **durability/process** — SQLite, files, logs, restart behavior, PIDs, or process ownership.

Many scenarios intentionally cross several surfaces. The table names the primary action and observation boundaries; additional boundaries that are essential to the contract are included in the same cell.

## Root Playwright specifications

The root suite currently contains 35 specifications and 17,372 lines. It is a system acceptance suite, not a single transport suite.

| Specification | Primary action boundary | Primary observation boundary |
| --- | --- | --- |
| `00-generate-show-files.spec.ts` | HTTP v1 show operations plus visible Group-property UI | HTTP show identity/revision and UI projection; generated show files |
| `01-foundational-dimmers-and-groups.spec.ts` | legacy WS command execution, UI keypad/touch, HTTP v1 setup, bench time | HTTP Programmer/show projections, UI projection, Art-Net and sACN output |
| `02-cue-semantic-contracts.spec.ts` | legacy WS and HTTP cue/programmer operations with UI playback gestures | HTTP cue/runtime projections, UI projection, output wire at virtual-time checkpoints |
| `02-cues-tracking-and-arbitration.spec.ts` | paired legacy WS and visible UI Move/Copy workflows | HTTP show revisions and runtime state, UI status/dialogs, virtual-time output |
| `02-help-screenshots.spec.ts` | visible UI navigation and keypad setup | deterministic screenshots and UI projection |
| `03-network-output-protocols.spec.ts` | HTTP v1 route/patch mutation, legacy WS/UI programming, receiver and fault bench controls | Art-Net/sACN packets, route diagnostics and audit, UI route state |
| `04-osc-api-and-cross-surface.spec.ts` | OSC hardware input alongside legacy WS, HTTP v1, and visible UI | OSC feedback, HTTP authoritative state/events, Art-Net/sACN output, UI desk isolation |
| `05-desktop-process-integration.spec.ts` | packaged desktop launch/termination | desktop and child-server PID ownership/termination |
| `05-virtual-time-persistence-and-recovery.spec.ts` | HTTP v1/legacy WS/UI actions plus virtual time, stop/start, crash and fault bench controls | durability/process, HTTP runtime state, logs, first output frame, UI recovery state |
| `06-cuelist-view-and-settings.spec.ts` | visible Cuelist/Cue UI backed by HTTP v1 and legacy WS setup | UI projection, HTTP revisions/runtime identity, reload/restart behavior |
| `06-preload-modes-and-virtual-playbacks.spec.ts` | HTTP v1 and legacy WS operations, visible physical/virtual controls, OSC, virtual time and restart | HTTP runtime/events, UI pending state, OSC feedback, output timing, durability |
| `07-move-in-black.spec.ts` | HTTP v1 show/runtime mutation, visible UI edits, virtual time | resolved/output state at timing boundaries, HTTP projection, UI state |
| `07-playback-configuration.spec.ts` | HTTP v1 and legacy WS setup, visible controls, OSC, virtual time | HTTP playback projections, UI feedback, OSC LEDs/faders/actions, reload behavior |
| `08-file-manager-and-text-editor.spec.ts` | HTTP v1 file operations and visible File Manager/Text Editor actions | confined file HTTP contract and UI projection |
| `09-cue-go-to-load.spec.ts` | legacy WS, HTTP v1, visible Cue keys, OSC | HTTP loaded/next runtime, UI projection, OSC feedback, output timing |
| `09-desk-lock.spec.ts` | visible lock UI, HTTP v1 and OSC attempts while locked | UI lock coverage and HTTP/output proof that rejected inputs do not mutate state |
| `11-update-highlight-fixture-profiles-and-matter.spec.ts` | HTTP v1/legacy WS and visible UI, OSC, restart, SQLite fixtures, fault routes | HTTP/runtime projections, UI errors and geometry, OSC, SQLite and restart durability |
| `14-sound-to-light.spec.ts` | paired HTTP v1 and visible UI with a simulated browser audio source | authoritative HTTP speed-group/configuration state and UI audio feedback |
| `15-text-editor.spec.ts` | visible multi-editor and picker workflows with HTTP v1 file setup | UI dirty/conflict/read-only/Markdown state and durable file association |
| `16-file-manager.spec.ts` | HTTP file APIs and visible manager/picker workflows, including OSC-attached layout | HTTP auth/range/revision contract, UI state/geometry and confined filesystem behavior |
| `19-manual-review-software-corrections.spec.ts` | visible operator workflows with HTTP setup and attached-hardware state | UI terminology, placement, geometry and captured request destinations |
| `21-completion-coverage.spec.ts` | production hardware-simulator pointer gestures on a synthetic host page | simulator geometry and emitted `send_control` payload sequences |
| `22-client-history-and-removal.spec.ts` | HTTP v1 client operations, visible UI, server stop/start | HTTP client/history state, UI presence/removal, restart durability and desk isolation |
| `22-fixture-sheet-color-dot-outline.spec.ts` | visible Fixture Sheet in software and OSC-attached hardware layouts | UI color/outline geometry and screenshot evidence |
| `25-return-home-position-special-dialog.spec.ts` | paired legacy WS batch mutation and visible modal action; OSC attaches hardware layout | HTTP Programmer values/audit, UI enablement and undo/clear behavior |
| `26-color-special-dialog-alignment.spec.ts` | paired legacy WS batch mutation and visible pointer/Shift range gesture; OSC Shift input | HTTP Programmer values/audit count and UI preview/alignment/gesture state |
| `28-hardware-connected-playback-selection.spec.ts` | visible controls and legacy WS setup with OSC-attached hardware | HTTP ownership/page state and UI selected-control projection |
| `30-command-line-history-panel.spec.ts` | legacy WS setup and visible command-line/history input with OSC-attached layout | UI history/unfinished-input state across dismissal, reconnect and reload |
| `31-hardware-connected-encoders.spec.ts` | visible hardware encoder modal, legacy WS setup, OSC attachment, virtual time | UI encoder targets, HTTP Programmer projection and output wire |
| `33-record-and-update-menu-colors.spec.ts` | visible Record/Update workflows with HTTP setup and OSC-attached layout | computed UI colors and menu state in both layouts |
| `34-active-playback-colors.spec.ts` | HTTP playback setup and visible UI with OSC-attached layout | computed UI active/configured/selected color projection |
| `35-fixture-address-screen.spec.ts` | visible address-screen navigation and keyboard input | UI completeness, reachability and geometry |
| `36-cuelist-and-cue-settings-layout.spec.ts` | HTTP v1 Cue/Playback setup followed by visible settings actions | inline Cue settings and structured Cuelist modal UI geometry/content |
| `product-demo.spec.ts` | narrated visible UI workflow with HTTP setup, virtual time and output receivers | UI screenshots/video plus Art-Net/sACN and HTTP state |
| `visual-recording.spec.ts` | visible UI plus OSC hardware input and HTTP setup | recorded video, UI state, OSC behavior and Art-Net/sACN output |

## Coupling and migration pressure

### Playwright

- `ApiDriver.command()` appears at 192 source call sites in root specifications. Another occurrence is the shared `tests/support/catalog.ts::command` helper, for 193 source occurrences under `tests/`.
- Scenario helpers expand that source count into more actions: 77 calls use the local `command()` helper in `01-foundational-dimmers-and-groups.spec.ts`, and 4 use the shared catalog helper in `04-osc-api-and-cross-surface.spec.ts`. Together with 63 direct `programmer.execute` call sites, at least 144 scenario actions execute command-line text. This is the highest-leverage first migration seam.
- The largest direct `api.command()` specifications are `02-cue-semantic-contracts.spec.ts` (48), `06-preload-modes-and-virtual-playbacks.spec.ts` (28), `01-foundational-dimmers-and-groups.spec.ts` (27 plus 77 local-helper calls), `11-update-highlight-fixture-profiles-and-matter.spec.ts` (21), `05-virtual-time-persistence-and-recovery.spec.ts` (15), `03-network-output-protocols.spec.ts` (12), and `04-osc-api-and-cross-surface.spec.ts` and `09-cue-go-to-load.spec.ts` (9 each). Four further shared-helper calls are in `04-osc-api-and-cross-surface.spec.ts`.
- Several tests import implementation code instead of a public contract: the color-range scenario imports the frontend color-assignment oracle; fixture-profile coverage imports a frontend profile builder; and `tests/support/plannedDemoState.ts` imports frontend model/types.
- Some UI integration tests observe private host machinery through Tauri globals, `light:*` browser events, or local-storage session records. These are characterization tests today and should move to public projections or explicit test adapters before the private implementation is removed.

### Rust

- The Rust workspace had no crate-level `tests/` integration-test directories at baseline. Its coverage was concentrated in 355 inline test functions across 28 `#[cfg(test)]` modules; new pure reducer tests may remain inline as Stage 1 proceeds.
- `crates/server/src/main.rs` alone owns 82 test functions. Its server tests build routers and then frequently reach through the same module into shared implementation state: the baseline audit found 28 router constructions/usages, 46 direct lock accesses, 34 direct `AppState` accesses, and 7 direct OSC-internal accesses in the test section.
- Inline unit tests should remain adjacent to pure reducers and codecs. Router, authentication, application-service, persistence, OSC-adapter and lifecycle behavior should move to crate-level integration tests as those public boundaries appear.
- Tests must stop treating direct mutex access as proof of behavior. Application services should return immutable query projections and typed outcomes; adapter tests should assert only public wire contracts and domain events.

## Measured baseline

These results were recorded before the Stage 1 transport migration:

| Command | Baseline result |
| --- | --- |
| `cargo fmt --all -- --check` | passed |
| `cargo clippy --workspace --all-targets -- -D warnings` | passed |
| `npm run build` in `apps/hardware-controls` | passed |
| `npm test` in `apps/control-ui` | passed: 83 files, 512 tests after repairing one stale Groups-window accessibility expectation |
| `./test unit` | Rust suites passed; the first control-UI run exposed the stale Groups-window expectation recorded above |
| `./test e2e-api` | 75 passed, 7 failed |

The seven E2E failures were present at the baseline and are not evidence of a refactor regression:

1. `GROUP-004` — the unpatched output slot was `128` where the scenario expected `0`.
2. `PROG-002` — spread output contained `178`/`229` where the scenario expected `179`/`230`.
3. `MERGE-001` — merged output contained `178` where the scenario expected `179`.
4. `DMX-005` — patch-overlap fixture setup failed before the atomic-conflict assertion.
5. `DMX-006` — the schema-v2 fixture snapshot identity was inconsistent.
6. `DMX-008` — the schema-v2 fixture snapshot identity was inconsistent.
7. `HIGHLIGHT-001` — the expected Group object was missing during setup/assertion.

Every later full-suite result must be compared with this named list. A changed failure, a newly failing scenario, or a passing baseline failure requires investigation; a raw failure count is not sufficient.

## First migration order

1. Add characterization coverage for revisioned command-line state and keep one focused legacy WebSocket compatibility test.
2. Introduce the authenticated, desk-scoped HTTP command-line adapter with typed requests, outcomes, errors, request identity and compare-and-set revision handling.
3. Change both command helpers—the local helper in `01-foundational-dimmers-and-groups.spec.ts` and `tests/support/catalog.ts::command`—to use that adapter. This moves 81 scenario call sites without changing intent.
4. Migrate direct `programmer.execute` callers, starting with the command-heavy specifications listed above. Preserve visible UI and OSC paths; they are independent acceptance surfaces, not alternate test shortcuts.
5. Move remaining `ApiDriver.command()` families to bounded Programming, Playback, Show, Desk and Output application APIs. Retain only deliberately named v1 compatibility coverage.
6. Replace frontend implementation imports and private Tauri/event/local-storage observations with generated wire DTOs, authoritative projections, or narrow explicit test adapters.
7. As each Rust service boundary becomes public, move router/auth/persistence/OSC/lifecycle coverage out of `main.rs`; keep only pure unit tests inline and prohibit direct service-lock access from integration tests.
8. Run focused migrated specs first, then `./test e2e-api`, comparing scenario identities and outcomes with the measured baseline above before removing any facade.
