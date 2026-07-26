# Backend Highlight Application Service

## Status

Finished. This backend-only prerequisite was extracted from former plan 07 required work 6. It ran
concurrently with plan 02 because it owned Rust application/domain adapters and backend tests only;
it did not change frontend providers, stores, components, stories, generated TypeScript, or the
Storybook verification runtime.

## Goal

Create one transport-independent Highlight application service and route the existing HTTP,
WebSocket, OSC, persistence/output synchronization, and feedback paths through it without changing
operator behavior or wire contracts.

Estimated effort: 0.3–0.6 Codex day.

## Required work

1. Characterize the existing HTTP, WebSocket, and OSC Highlight paths, including ownership,
   selection stepping, persistence, output projection, semantic/facade events, feedback, and
   rejection behavior.
2. Add a typed application command/result and a narrow port owned by `light-application`.
3. Make the service the single owner of Highlight transition orchestration and ordered effects.
4. Keep transport parsing, HTTP status mapping, OSC press/dedupe handling, and wire serialization
   in the headless adapter.
5. Route status reconciliation and live actions through the service while preserving existing
   API URLs, WebSocket action names/payloads, OSC addresses/aliases, and stored programmer
   compatibility.
6. Add focused application-service and adapter regression tests.

## Acceptance and verification

- HTTP, WebSocket, and OSC invoke the same application service and produce one transition,
  selection persistence decision, output synchronization, semantic event, and feedback update.
- Ownership conflicts and malformed transport input leave state unchanged and retain their current
  surface-specific response behavior.
- Highlight remains independent of programmer values, actual selection stepping remains
  authoritative, unpatched fixtures remain selectable, and Blind/Preview suppression is preserved.
- Run focused application and headless Highlight tests, wire tests, formatting/Clippy,
  architecture checks, API acceptance, and proportionate broader backend verification.

## Result

### Changes

- Added a transport-independent `HighlightService`, typed `HighlightCommand`, result, environment,
  compatibility-publication policy, and narrow `HighlightPorts` boundary to `light-application`.
- Made the application service own the ordered Highlight transition, optional programmer-selection
  persistence, internal revision acknowledgement, combined output synchronization, event
  publication decision, and feedback decision.
- Routed the HTTP action/status endpoints, typed WebSocket action, legacy WebSocket mode adapter,
  OSC action adapter, bootstrap projection, selection reconciliation, and OSC feedback projection
  through the service.
- Preserved the existing HTTP URL/body and conflict mapping, WebSocket action and response shape,
  OSC `/previous`/`/prev` alias and 150 ms dedupe behavior, source-specific facade event payloads,
  programmer persistence, Patch Preview output union, ownership rules, and Blind/Preview
  suppression.
- Kept transport concerns at the headless boundary: HTTP status mapping, WebSocket decoding,
  OSC press/source/dedupe validation, compatibility payload serialization, and the once-per-input
  OSC feedback broadcast.
- Added an adapter boundary test that rejects production Highlight actions which call
  `HighlightRegistry::action_guarded` directly.

### Tests

- `cargo test -p light-application highlight` — 4 passed.
- `cargo test -p light-headless-runtime --no-default-features highlight` — 8 passed.
- `cargo test -p light-application` — 408 passed.
- `cargo test -p light-headless-runtime --no-default-features` — 470 passed and 1 ignored before
  the sandbox denied the local CITP socket; the one affected test passed when rerun with local
  socket permission.
- `cargo test -p light-wire` — 87 passed, including generated-contract freshness.
- `cargo fmt --all -- --check` — passed.
- `cargo clippy -p light-application -p light-headless-runtime --lib` — completed successfully;
  only pre-existing warnings outside this chunk were reported.
- `node tools/check-architecture.mjs` — passed.

### Limitations

- The strict `cargo clippy ... -- -D warnings` gate remains blocked by pre-existing warnings in
  Programmer values, group-management wire mapping, object APIs, playback target helpers, request
  context, and tolerant JSON. The ordinary Clippy run reports no warning in this chunk.
- Root Playwright/API commands were not run because they build and launch the concurrently changing
  frontend/Storybook runtime. Rust route tests cover the real HTTP, WebSocket, and OSC Highlight
  adapters without crossing that ownership boundary.
- OSC feedback deliberately remains broadcast once at the end of the existing OSC input loop. The
  service requests feedback directly for HTTP/WebSocket and reconciliation, while the OSC adapter
  preserves its established once-per-input transport cadence.
- Raw registry access remains for lifecycle clearing and focused characterization tests. Production
  live actions and authoritative status/feedback projections use `HighlightService`; plan 13 will
  remove the remaining raw `AppState` capability access after plan 07b establishes final event
  ownership.

### Commit

`refactor(output): centralize highlight application service` (this implementation and plan move).
