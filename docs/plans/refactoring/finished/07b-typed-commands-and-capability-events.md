# Typed Commands and Capability Events

## Goal

Remove the remaining generic WebSocket command/facade event compatibility path after the backend
Highlight application service extracted as plan 07a is complete.

Estimated effort: 1.3–2 Codex days.

## Queue dependency

Doing. Plans 02 and 05 have stabilized the desktop provider/store consumers, generated-client
contracts, architecture checks, warm authoritative caches, and frontend verification. Plan 07a
completed the backend Highlight boundary, and the Storybook lane has no active owner.

## Required work

1. Inventory every production `WsCommand { command: String, payload: Value }` caller/handler and
   every `FacadeNotification { kind, payload }` publisher/consumer.
2. Add generated typed command DTOs, runtime decoders, and capability event variants.
3. Migrate live-control actions to the typed established WebSocket plus their required HTTP action
   forms under `docs/engineering/api-rules.md`.
4. Migrate facade events capability by capability; translate typed events to compatibility output
   only while a deliberate external consumer remains.
5. Replace broad frontend bootstrap/follow-up reads with typed store updates and scoped repair.
6. Consume the plan 07a Highlight service from the final typed command/event boundary without
   reintroducing transport-specific Highlight policy.
7. Remove the second facade event bus, generic command dispatch, obsolete client methods, and
   compatibility tests after callers migrate.

## Acceptance and verification

- Malformed frames fail boundary validation without state mutation.
- Each action produces one semantic outcome/event across HTTP, WebSocket, and OSC.
- Highlight behavior and feedback are identical across surfaces.
- Ordinary events do not trigger unrelated bootstrap or catalog reads.
- Generated-contract, architecture, event gap/replay, API, OSC, UI, and desktop tests pass.

## Result

### Changes

- Replaced the generic `WsCommand` command/payload envelope with generated, exhaustive
  `LiveActionFrame` and `LiveAction` contracts. Every migrated live action now has a typed
  WebSocket decoder and the HTTP peer required by the API rules; the new live HTTP request DTOs
  contain no replay identity, while Generate Fixture Presets is correctly classified as a
  replay-safe object intent.
- Removed `dispatch_ws_command`, the compatibility WebSocket modules, `FacadeNotification`, the
  second facade event bus, and their production/test callers. Capability-scoped application
  events now carry typed authoritative Programming, Playback, Output, Show, Desk, Hardware,
  Highlight, and operator projections.
- Routed Highlight through the plan 07a application service and installed authoritative Highlight
  and hardware projections directly in the desktop state. Ordinary rename, Programmer, Highlight,
  and hardware events no longer trigger broad bootstrap or catalog reads; full bootstrap is
  reserved for show open and rollback.
- Migrated the desktop runtime, stores, API bench, OSC/UI acceptance helpers, and backend route
  tests to typed actions and capability events. Removed the retired direct compatibility specs and
  tightened the architecture ratchet to an empty generic-command baseline.
- Brought the touched Speed Group and Output Runtime URLs into the header-scoped desk-context
  contract, accepted and logged unknown request fields, and preserved omitted/null/value capture
  context semantics.
- Kept repeated relative Programmer steps aligned with their displayed operator value by
  normalizing floating-point accumulation at the application boundary.

### Tests

- `cargo test -q -p light-wire` — 90 passed, including generated-contract freshness and tolerant
  request decoding.
- `cargo test -q -p light-application event::tests` — passed; the focused normalized relative-step
  regression also passed.
- `cargo test -q -p light-headless-runtime --lib` — 480 passed and 1 ignored; the sole sandbox
  socket failure passed when the exact CITP test was rerun with loopback binding permission.
- Focused headless tests for malformed frames, HTTP/WS action peers, request replay, Highlight,
  Output Runtime, Speed Groups, generated presets, command/selection events, OSC recording, page
  addressing, and gap/event routing — passed.
- `npm --workspace apps/light-desktop test -- src/api/client src/features/server` — 17 files and
  63 tests passed.
- `npm --workspace apps/light-desktop run typecheck` — passed.
- `npm run test:e2e-api` — 21 passed.
- `tests/55-semantic-osc-api-and-cross-surface.spec.ts --workers=1` — 11 passed across OSC, API,
  UI, desk isolation, current-page addressing, typed external reconciliation, and relative
  encoder behavior.
- `cargo fmt --all -- --check` and `git diff --check` — passed.
- `npm run open` — built and bundled both macOS applications and launched ToskLight. The real
  headless process returned `status: ready`, `active_show_error: null`, and
  `recovery_mode: false`; readiness took 68 ms and bootstrap 3 ms.

### Limitations

- The Plan 07b architecture violations are resolved: generated wire DTOs stop at the API boundary,
  and the generic-command baseline is empty. The repository-wide architecture command still
  reports unrelated concurrent CSS duplication, Fixture Placement modal-stack, stale bench
  migration inventory, and stale semantic-catalog findings; those files were not changed or
  overwritten by this plan.
- The Rust contract generator continues to print its pre-existing ts-rs warnings for serde
  attributes it does not interpret. Contract generation and freshness tests pass.

### Commit

- `feat(api): replace generic live command and event boundaries`
