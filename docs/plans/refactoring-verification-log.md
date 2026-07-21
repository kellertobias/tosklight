# Refactoring Verification Log

Superseded per-slice verification evidence, split out of
[`refactoring-progress.md`](refactoring-progress.md) to keep that living handoff readable.
Entries are historical: they record what passed when each slice landed, not the current state.
The current snapshot stays in the progress document.

- Every production file and function touched by this slice remains within the hard 1,200-line and
  150-line limits; the values wire implementation is split into feature-owned projection, event,
  and mutation modules below 400 lines each. Dependency-direction checks and all 10
  scanner/ratchet unit tests pass. The repository-wide architecture command still exits 1 because
  it reports the pre-existing committed Dynamics Editor experiment at 1,382 lines; this
  Programmer-values slice deliberately did not expand into that separate experiment.
- The earlier full-tree design-goal report was 52 files above 400 lines and 3,479 functions above
  20. The two remaining in-scope hotspots named by that report have since been split: Programming
  service is now 399 lines and the command HTTP adapter is 304 lines. The expanded server feature
  boundary was split into a 113-line composition hook and an 81-line Programmer-values helper.
  The source-size ratchet reports no changed production function above 150 lines and only the
  pre-existing committed Dynamics Editor experiment at 1,382 lines.
- Focused application, server, wire, frontend, architecture, source-size, MVR, File Manager,
  Playback, Preload, Patch, Output, event, shared-control, Stage 3D, build, and strict Clippy checks
  have passed for their committed slices. The latest command-line slice passed 18 Programming
  application tests, 4 command HTTP scenarios, the focused OSC shortcut test, 219 combined scoped
  frontend tests, and 114 focused consumer tests. The external selection-adapter slice passes all
  7 Playback route tests, focused Highlight HTTP/OSC coverage, the legacy Programmer gesture test,
  formatting, and strict server Clippy. The engine-refresh slice adds 3 application coordinator
  tests and 2 server integration tests covering unowned and nested actor-owned multi-desk Group
  changes. The current full server library run passes 215 tests with only the sandbox-blocked CITP
  socket test failing and one standard-port Matter test ignored. All 54 engine unit tests plus its
  integration test pass; focused Group-master WebSocket and Matter activation tests pass. The
  Stage migration adds 6 focused frontend tests for ordered streamed updates, optimistic
  gestures/clear/rollback, and active-only Stage/Fixture pane observation. The following primary
  window slice adds 5 focused streamed/optimistic/dormancy scenarios; its 3 focused files pass all
  19 tests together with the existing selection and Highlight coverage. The isolated
  Programmer-values frontend core passes all 24 focused store/session/view tests and passed the
  full frontend typecheck. The parameter-bank migration passes 17 focused tests across its legacy
  behavior and streamed selection suites. The modal migration passes 13 focused tests including
  ordered streamed writes and closed-view teardown. Both slices pass the full frontend typecheck.
- The completed backend Programmer-values slice passes `cargo fmt --all -- --check`; all 60
  `light-programmer` tests; all 201 `light-application` tests; all 24 `light-wire` unit tests plus
  generated-contract verification; all 8 focused server Programmer-values tests; all 6 focused
  capture-mode tests; the lifecycle deletion/recreation regression; the 6 active-Preload tests;
  the transient compatibility regression; and
  `cargo check -p light-server --no-default-features`. The full server library run has 234 passing
  tests and 1 ignored test; only the known sandbox-blocked CITP socket test fails with the sandbox's
  `Operation not permitted` error. Fixture and Group set/release, one-action batch and clear, exact
  no-op/event behavior, replay, rollback and revision conflict, capture/write concurrency,
  lifecycle replacement, user ownership, same-user multi-desk sharing, foreign-user rejection, and
  timing/order preservation are covered. Wire tests print the existing non-fatal `ts-rs`
  `deny_unknown_fields` warning.
- The production frontend Programmer-values and capture-authority slice passes 106 focused tests
  across exact wire decoding, HTTP/WebSocket transport, prediction, stores, sessions, writer,
  mounted views, and normal `ServerProvider` composition. The focused tests cover rollback, safe
  exact-request replay, no-change results, both response/event orders, capture and values
  cursor-gap repair, concurrent repair joining, server/session replacement, late-response
  isolation, exact-user subscriptions, first-view dormancy, refusal while authority is loading or
  active Preload captures Programmer, absence of a broad bootstrap request, suppression of
  unrelated context rerenders, undeclared wire fields, and React StrictMode effect replay. The
  complete frontend suite passes all 936 tests in 148 files; typecheck and the production Vite
  build pass, with only the existing large-chunk advisory. The raw-control audit and Numeric Pad
  selection harness were corrected so test-only probes do not masquerade as production controls
  and the keypad test follows scoped selection authority.
- Current focused backend verification passes 2 normal-values Programmer tests, 7 Programming
  application action tests, and 8 server authentication/action/subscription tests. Generated wire
  contracts match the Rust DTOs, `cargo fmt --all -- --check` passes, and `git diff --check` is
  clean. Wire tests print the existing non-fatal `ts-rs` `deny_unknown_fields` warnings. A final
  repository-wide Rust/Clippy suite and real desktop run remain pending.
- The pending-Preload and parameter-value slice passes 5 focused Programmer batch/action tests,
  including a 10,000-fixture set/release batch completed in 0.05 seconds in the debug test run, and
  3 focused generation tests including rejected live and staged transaction rollback. All 209
  `light-application` tests pass, including 54 Programming tests, replay fingerprint/payload-reuse
  checks, byte-budget eviction, and rejected-batch rollback. The 6 focused server pending-Preload
  tests cover fixture/Group mutation and release, no-op, replay, conflict, same-user two-desk
  sharing, a successful independent second user, foreign-user rejection, and exact-user event
  filtering; all 8 normal Programmer-values server tests still pass. Both focused Preload wire
  tests and generated-contract verification pass. Strict Clippy passes for Programmer,
  application, wire, and server with the existing `too_many_arguments` allowance.
- The frontend now passes 1,002 tests in 158 files, including pending-Preload wire/transport,
  store/session/writer/provider composition, parameter routing, response-before-event and
  event-before-response, rollback, replay/no-change, cursor repair, server/session replacement,
  capture-loading refusal, dormant first-view behavior, no bootstrap request, and unrelated-render
  suppression. Typecheck and the production Vite build pass; the only build output is the existing
  large-chunk advisory. Dependency-direction and scanner tests pass. The source-size command still
  exits non-zero solely for the pre-existing committed 1,382-line Dynamics Editor experiment; no
  production file or function changed by this slice exceeds a hard limit.
- The scoped activity-summary cohort passes 19 focused tests covering capture-first dormancy,
  mutually exclusive normal/pending hydration, route replacement, scalar rerender suppression,
  stale-bootstrap refusal, same-user System Controls rows, and foreign-user compatibility rows.
  The remaining writer cohort passes 61 focused tests across the shared bounded write queue,
  parameter controls, Channels, modal selection, one-batch Position ticks and Home, inactive-view
  dormancy, API facade removal, and frontend typecheck. Numeric Pad and keypad coverage passes all
  26 focused tests after typed selection/value clearing. Dependency checks and all 10 architecture
  scanner tests pass; the architecture command still exits non-zero only for the separately owned
  Dynamics Editor experiment at 1,382 lines.
- The Fixture Sheet target reader and compatibility-hydration slice passes 22 focused values,
  filter, window, loading, logical-head, and dormancy tests plus 40 server-routing tests. Focused
  Rust tests cover normal and pending value categorization, user/desk identity, transient
  retrigger/release, and timed-pulse completion. Frontend typecheck, `cargo fmt --all -- --check`,
  and `git diff --check` pass. The changed routing and server production modules are below 400
  lines; malformed, foreign, and mixed compatibility events retain the conservative fallback.
- The Patch/setup migration and semantic correction pass 69 focused Patch, selection-provider,
  window-activation, parameter-control, and Patch-feature tests across 8 files plus the full
  frontend typecheck. Tests cover hidden-view dormancy, exact selection-only subscription,
  optimistic closed-selection preservation, ordered range/additive replacement, logical-head
  toggle/removal, split and parameter selection, and authoritative post-placement head targets.
  `git diff --check`, dependency directions, and all 10 architecture scanner tests pass; the
  architecture command still exits non-zero only for the pre-existing committed 1,382-line
  Dynamics Editor experiment. Every changed production file remains below 400 lines.
- The Programmer lifecycle slice passes the complete 215-test application suite, 69 Programmer
  tests before the later queue additions, 29 wire tests plus generated-contract verification, 6
  focused server lifecycle tests, and 31 focused frontend tests. The row-level selection-count
  correction separately passes 6 Programmer, 9 application, 3 wire, and 6 server lifecycle tests.
  Dependency directions, all 10 scanner tests, frontend typecheck, and `cargo fmt` pass.
- The queued Preload playback backend passes all 73 Programmer tests, all 219 application tests,
  all 30 wire tests plus generated-contract verification, 3 focused snapshot/subscription server
  tests, the successful capture/replay/GO-drain scenario, and failed-GO rollback. The final
  generated DTO matches the strict frontend decoder. Eight frontend files pass 82 focused tests;
  the complete frontend run passes 1,093 tests in 174 files, typecheck, and the production build
  with only the existing large-chunk advisory. Review confirms first-view dormancy, exact-user
  routing, stale-bootstrap refusal, scope replacement, gap repair, and unrelated-render isolation.
- Public Programmer-state hardening passes the focused unauthenticated-bootstrap, authentication,
  same-user/foreign-user server tests, the pre-clone user-filter Programmer test, 18 API-client
  tests, frontend typecheck, `cargo check -p light-server --no-default-features`, `cargo fmt`, and
  `git diff --check`. The server library passes 254 tests with 1 ignored when the CITP thumbnail
  socket test is excluded; that one test cannot bind under the sandbox. Thirteen stale absolute
  event-sequence assertions exposed by login lifecycle publication now use post-setup baselines
  while retaining exact per-action count and ordering checks.
- Typed Preset recording passes all 76 `light-programmer` tests, all 233 `light-application`
  tests, all 35 `light-wire` unit tests and generated-contract verification, and all 31 focused
  command HTTP server tests. The server set covers authoritative changed/no-change outcomes,
  revision conflict, replay, authentication, forged values, Preload rejection, one Show event per
  real mutation, command-line convergence, a real OSC key sequence, and WebSocket replay without
  repeated interaction side effects. `cargo check -p light-server --no-default-features` and
  `cargo fmt --all -- --check` pass. The frontend passes 82 focused Preset/Show Objects/API tests
  and the complete 1,124-test suite in 178 files; typecheck and the production build pass with only
  the existing large-chunk advisory. Dependency directions, all 10 source-size scanner tests,
  generated contracts, and `git diff --check` pass. The repository-wide source-size command still
  exits non-zero for two pre-existing committed ratchet findings outside this slice: the
  1,382-line Dynamics Editor experiment and the 154-line `handle_subscription_osc` function.
  Every production file and function changed by Preset recording remains within the hard limits.
- Typed Group recording passes all 83 `light-programmer` tests, all 250 `light-application` tests,
  all 42 `light-wire` unit tests plus generated-contract verification, and the full server library
  with 272 passing, 1 ignored, and only the sandbox-blocked CITP test filtered. Independent focused
  review additionally passes 7 Group domain tests, 9 Group service tests, 8 active-show transaction
  tests, 7 wire tests, 11 Group server tests, and the compatibility WebSocket scenario. The frontend
  passes all 1,161 tests in 182 files, typecheck, and the production build with only the existing
  large-chunk advisory. Coverage includes overwrite/merge/subtract/delete, stored-empty and no-op,
  replay, conflict rollback and exact-object repair, both event/response orders, deletion settlement,
  same-user two-desk capture, foreign-user and forged-context rejection, one Show event per mutation,
  gesture-event ordering, direct/nested Highlight reconciliation, first-action dormancy, authority
  replacement, and successful-command reset versus failure retention. `cargo fmt --all -- --check`,
  generated contracts, dependency directions, all 10 scanner tests, and `git diff --check` pass.
  The repository-wide architecture command still exits non-zero only for the same unrelated
  1,382-line Dynamics experiment and 154-line `handle_subscription_osc`; every production file
  changed by Group recording remains below 400 lines and every changed production function remains
  below the hard limit.
- Typed Cue recording passes all 87 `light-programmer` tests, 268 `light-application` tests, 66
  `light-playback` unit tests plus 4 automatic-transition tests, 55 `light-show` tests, 79
  `light-fixture` tests, and 47 `light-wire` tests plus generated-contract verification. All 50
  focused command HTTP tests and all 16 Cue-recording server tests pass. The complete server library
  passes 296 tests with 1 ignored when the sandbox-blocked CITP socket test is skipped; the
  unfiltered run fails only that test with `Operation not permitted`. The frontend passes all 1,230
  tests in 189 files, typecheck, and the production build with only the existing large-chunk
  advisory. The 27 focused Cue semantics and hardware-connected Playback API/UI scenarios pass
  serially outside the sandbox. Coverage includes normal and Preload capture, overwrite/merge/
  subtract/delete, topology creation, take-live, no-op, replay, conflict rollback, one authoritative
  event, same-user desks, foreign-user rejection, exact snapshot repair, both optimistic ordering
  races, scope replacement, first-view dormancy, no broad bootstrap request, and unrelated-render
  suppression. Strict Clippy, formatting, dependency directions, all 10 source-size scanner tests,
  generated contracts, and `git diff --check` pass. The source-size command now reports only the
  pre-existing 1,382-line Dynamics experiment; OSC subscription handling was split below the hard
  function limit. `./build open` succeeds and `/api/v1/readiness` reports `ready` with no current-
  launch server error. Wire tests retain the existing non-fatal `ts-rs` warning.
- Explicit Cue pending-choice authority passes all 88 `light-programmer` tests, all 271
  `light-application` tests, all 47 `light-wire` unit tests plus generated-contract verification,
  64 focused command HTTP tests, the Cue-transfer and 3 engine-selection-refresh regressions, and
  the accepted compatibility Speed reset regression. Four focused frontend files pass all 44
  modal, view, store, writer, rollback, narrow-selector, and authority-replacement tests; frontend
  typecheck passes. All 8 Cue arbitration/transfer and Speed API/UI Playwright scenarios pass
  serially outside the sandbox. Strict Clippy for Programmer, application, wire, and server,
  `cargo fmt --all`, and `git diff --check` pass. The accepted Speed path exposed and fixed two
  operator-visible integration defects: compatibility execution now persists the authoritative
  command reset before its single retained event, and Programming snapshots acquire the user gate
  before the desk gate so the globally mounted exact-desk observer cannot deadlock startup. Every
  changed production file remains at or below 400 lines. `./build open` rebuilds and launches both
  desktop bundles, `/api/v1/readiness` reports `ready`, and the current launch adds no server error;
  Vite retains only its existing large-chunk advisory and wire generation retains the existing
  non-fatal `ts-rs` warning.
- Atomic Playback exclusions and scoped related-outcome reconciliation pass all 89 Programmer, 61
  Engine, and 273 application library tests. The server library passes 315 tests with 1 intentional
  Matter-port ignore when the CITP thumbnail socket test is filtered; that CITP test passes
  separately outside the sandbox. Focused coverage passes 9 Engine Playback-boundary tests, 16
  Playback application tests, 17 v2 route tests, 28 Preload tests, the restored-exclusion
  normalization test, all 4 output-scheduler tests, and 7 Matter tests. The prior parallel-only Cue
  transfer failure also passes alone. The 48 wire tests and generated-contract verification pass;
  strict Clippy for Engine, application, wire, and server passes with the established
  `too_many_arguments` allowance. `cargo fmt --all -- --check` and `git diff --check` pass. The
  focused frontend contract passes 74 Playback, Preload-queue, store, session, view, and adapter
  tests; request-correlation and command-summary regressions add 30 passing tests. The complete
  frontend suite passes all 1,260 tests in 190 files; typecheck and the production build pass, with
  only the existing Vite chunk-size advisory. Every changed production file remains below 400
  lines; the Playback ports module is 352 lines after moving its focused test into a 60-line
  feature-owned test module. The focused VPB-007, CUE-005, and OSC-006 API/UI/OSC acceptance paths
  pass. OSC-006 now acknowledges
  the expected page value instead of attributing an earlier UDP feedback packet to the subsequent
  page transition; its final API/UI/OSC run passes all 3 paths. `./build open` rebuilds and launches
  both desktop bundles; `/api/v1/readiness` reports `ready` at snapshot revision 30 in 0.073 seconds,
  `/health` returns 200 in 0.001 seconds, and `/bootstrap` returns 200 in 0.003 seconds. The current
  launch reaches Engine-ready/server-bind state without a new log error.
- Exact Playback runtime effects and domain-specific persistence pass all 21 `light-control` tests,
  all 80 `light-playback` unit tests plus 4 automatic-transition integration tests, all 69 Engine
  unit tests plus its integration test, and all 276 application tests. The server library passes
  334 tests with 1 intentional Matter-port ignore and the CITP thumbnail socket test filtered;
  all 23 focused v2 route tests prove repeat no-change outcomes, transient holds, independent
  persistence sentinels, an all-no-op Preload drain, peer-only auto-off publication, timed-XFade
  retriggering, and hidden addressed Preload publication. The 48 wire tests and generated-contract
  test pass, as does the control-ui typecheck.
  Strict Clippy passes for Control, Playback, Engine, application, and server with the established
  `too_many_arguments` allowance; `cargo fmt --all -- --check`, the no-default-features server
  check, and `git diff --check` pass. Dependency directions and all 10 architecture scanner tests
  pass; the aggregate architecture command still exits 1 solely for the separately owned
  1,382-line Dynamics Editor experiment. Every production file touched by this slice remains below
  400 lines.
- Final-state Preload preparation and activation provenance pass all 80 `light-playback` unit tests
  plus 4 automatic-transition integration tests, all 73 Engine unit tests plus its integration
  test, all 90 `light-programmer` tests, 21 focused Engine Playback-boundary tests, 33 focused
  server Preload tests, and 25 focused v2 route tests. The server library passes 339 tests with 1
  intentional Matter-port ignore and only the sandbox-blocked CITP thumbnail socket test filtered.
  Coverage includes transient mutate/cancel no-op behavior, final-action event attribution,
  captured origin across same-user desks, explicit-page no-scope behavior, deskless Matter
  checkpoint/restore, false-to-true-only stamping, stable ordinals at equal timestamps, provenance
  preservation through Cue/timing changes and timed release fades, release clearing, legacy
  migration, both two-desk activation orders, deleted-desk isolation, public-payload omission,
  private-checkpoint retention, and idempotent repeat startup normalization. All 48 wire tests and
  generated-contract verification, control-ui typecheck, strict Clippy, the no-default-features
  server check, `cargo fmt --all -- --check`, and `git diff --check` pass. Dependency directions
  and all 10 architecture scanner tests pass; the aggregate architecture command still exits 1
  solely for the separately owned 1,382-line Dynamics Editor experiment. Every touched production
  file remains below 400 lines; the largest is Playback ports at 393 lines.
- Portable Playback topology and the Virtual Playback migration pass 10 focused application tests,
  4 wire tests, generated-contract verification, 5 topology route tests, 2 scoped-zone route tests,
  the stale-Show runtime-action guard, and the legacy Speed Group migration regression. The
  no-default-features server check, formatting, strict application/wire/server Clippy, and
  `git diff --check` pass; Clippy and wire generation retain only the known non-fatal `ts-rs`
  `deny_unknown_fields` warning. The complete frontend passes all 1,341 tests in 199 files,
  typecheck, and the production build. Focused coverage proves strict relational outcomes and
  status-aware object revisions, legacy storage identities, one-action configure/clear, response
  ordering, replay/no-change/conflict repair, same-Show authority replacement, Show-switch
  rejection, held Flash/Swap release, shared zone-cache/save ordering, inactive-view dormancy,
  no bootstrap or broad Playback read, and unrelated-render suppression. The Vite build retains
  its existing large-chunk advisory. Dependency directions and all 10 architecture scanner tests
  pass; the aggregate architecture command still exits 1 only for the separately owned 1,382-line
  Dynamics Editor experiment. New feature-owned production modules remain below 400 lines; the
  existing shared Show Objects session/store files received only narrow dormancy hooks and remain
  below the hard 1,200-line limit. At that milestone, one coalesced v1 `/playbacks` compatibility
  reload still served unmigrated physical panes; the current direct-Group/System Controls cohort has
  removed it. Scoped Virtual and Cue-editor paths never consumed the broad projection. Empty-slot
  assignments display the authoritative server allocation rather than a speculative grid identity.
- Cue-editor/Cuelist topology convergence passes 11 application tests, 4 strict wire tests,
  generated-contract verification, 5 server route tests, 41 focused frontend tests, frontend
  typecheck/build, and all 3 CUE-011 API/UI/supplemental browser paths. Coverage proves required
  nullable storage-identity preconditions, revision and replacement conflicts, lossless extension
  retention, one-action settings and renumber saves, rapid-edit response/event ordering, queued
  failure cancellation, repaired-authority retry, same-ID writer/session replacement, no broad
  `refresh()` or generic Cuelist reload, and the stale-dialog E2E conflict path. Strict application,
  wire, and server Clippy passes with the established `too_many_arguments` allowance; formatting
  and `git diff --check` pass. Dependency directions and all 10 architecture scanner tests pass;
  the aggregate architecture command still exits 1 only for the separately owned 1,382-line
  Dynamics Editor experiment. Every touched production file remains below 400 lines.
- Programming Update convergence passes all 26 focused application tests, all 8 strict wire tests,
  generated-contract verification, and the 14-test server `update` filter including all 12 Update
  application/compatibility/v2 route paths. Strict Clippy for application, wire, and server,
  `cargo fmt --all -- --check`, and `git diff --check` pass; wire generation retains only the known
  non-fatal `ts-rs` `deny_unknown_fields` warning. The frontend passes 53 focused transport,
  decoder, writer, provider, workflow, and legacy-facade tests plus the complete 1,379-test suite
  in 203 files, typecheck, and the production build with only the existing Vite chunk advisory.
  The focused UPDATE-001 API/UI acceptance pair passes outside the sandbox. Coverage proves strict
  DTO mapping, exact request/revision/scope authority, one-action direct apply, safe retry/replay,
  both HTTP/event orders, pending abandonment, narrow Group and legacy-key Cue conflict repair,
  FIFO rebasing, late-response rejection, provider dormancy, stable unrelated renders, settings
  ownership, and absence of bootstrap or broad Playback requests. Dependency directions and all
  10 architecture scanner tests pass. Every new production file remains below 400 lines and the
  split `UpdateWorkflow` function is 59 lines; the committed aggregate source-size exception for
  the isolated Dynamics Editor experiment remains outside this slice.
- Typed Cue transfer passes all 91 Programmer tests, 8 focused application tests, 4 Playback
  transfer tests, 4 wire transfer tests, generated-contract verification, 7 focused server route/
  compatibility tests, and 6 targeted server regressions covering the four Plain/Status and
  Move/Copy axes, choice reset, v1-notification isolation, Preset ownership, replay after choice
  resolution, and desk-scoped command-line idempotency. The doubly stale Show regression proves a
  conflict reports the current ActiveShow revision rather than the older pending-choice revision.
  Strict Clippy for the touched Rust crates, `cargo fmt --all -- --check`, and `git diff --check`
  pass. Eight focused frontend files pass all 71 transport, decoder, provider, writer, modal, and
  Programming-interaction tests; the full frontend run passes 1,395 tests in 207 files and
  typecheck passes. Coverage includes forged-scope rejection, exact one-event mutation
  cardinality, both HTTP/event orderings, rollback, replay/no-change, narrow conflict and cursor
  repair, dormant action-only composition, scope replacement, and late response rejection. A
  future route hardening test should still exercise valid same-user peer-desk credentials and a
  second authenticated user rather than only forged contexts. Wire generation retains only the
  known non-fatal `ts-rs`
  `deny_unknown_fields` warning; Serde and checked-in schemas remain strict.
- Existing-Playback mapping and primary physical-bank convergence pass 16 application topology
  tests, 4 strict wire tests, generated-contract verification, and all 7 server topology tests.
  Coverage proves changed/no-change/replay/conflict behavior, one Page-only projection and event,
  same-user two-desk and second-user active-Show behavior, authentication and forged-scope
  rejection, absent-Page creation, source identity/revision checks, Cuelist-only sources, lossless
  Page extensions, no source rewrite, and rejection of occupied canonical Cuelist/Playback/Page
  storage keys. The focused frontend contract passes 104 tests across transport, decoding, writer,
  provider, configuration, bank, fader, and Group dormancy; the complete frontend suite passes
  1,412 tests in 207 files and typecheck passes. Frontend coverage includes response/event ordering,
  replay/no-change, rollback and narrow conflict repair, authority replacement and late responses,
  loading and mismatched-runtime refusal, exact Cuelist semantic IDs, conditional Group hydration,
  unrelated-parent rerender suppression, typed assignment with captured revisions, preserved
  geometry/interception, and ordered Flash/Swap safety release after cancel, lost capture, unmount,
  or topology replacement. Strict application/wire/server Clippy passes with the established
  `too_many_arguments` allowance; the no-default-features server check, formatting,
  dependency-direction check, generated contracts, source-size hard-limit ratchet, and whitespace
  validation pass. Wire generation retains the existing non-fatal `ts-rs`
  `deny_unknown_fields` warning. At that milestone, Page create/rename and the companion consumers
  remained; the current cohort closes those items below. Exact conflict repair still cannot
  discover a replacement noncanonical storage key without collection-level topology repair.
- The current integrated Page/CUE/scoped-authority cohort passes all 1,596 frontend tests in 219
  files, frontend typecheck, and the production Vite build; the build reports only the existing
  large-chunk advisory. Focused Rust verification passes 20 Playback-topology application tests, 5
  Page-route server tests, 9 navigation server tests, all 63 `light-wire --no-default-features`
  tests, and generated-contract verification. The complete server library reports 365 passing, 1
  ignored, and one failure: the CITP thumbnail socket test cannot bind in the sandbox and fails with
  `Operation not permitted`. Re-running the library with only that test skipped reports 365 passed,
  0 failed, 1 ignored, and 1 filtered. Strict Clippy passes for application, wire, and server with
  the established `too_many_arguments` allowance; wire generation retains only its known non-fatal
  `ts-rs` warnings.
- The focused CUE acceptance paths pass all 3 API/UI/OSC cases in
  `09-cue-go-to-load.spec.ts`, and the dedicated v1 compatibility specification passes its single
  case when run outside the sandbox. `cargo fmt --all -- --check`, generated contracts,
  the aggregate architecture check, the command-boundary scanner and all 8 scanner unit tests,
  `git diff --check`, and the source-size hard-limit ratchet pass. The current size report has zero
  hard file or function violations; its design-goal inventory is 118 production files above 400
  lines and 5,162 production functions above 20 lines. Focused frontend coverage includes strict
  missing-Page refusal, legacy auto-create compatibility, response-before-event and event-before-
  response reconciliation, rollback/replay/no-change/conflict repair, abandoned-render and scope-
  replacement safety, inactive-view dormancy, no broad bootstrap request, and unrelated-render
  suppression.
- The portable-Group decoupling cohort passes all 1,606 frontend tests in 222 files, frontend
  typecheck, and the production build; Vite reports only the existing large-chunk advisory. Its 64
  focused tests cover active-only Group hydration, stale-authority refusal, selected-object render
  suppression, ordered Cue thumbnail expansion, deactivation and Show replacement cancellation,
  projected derived membership, on-demand paperwork Cuelists, dead facade removal, and absence of
  broad Playback reads. Dependency directions, all 10 source-size scanner tests, the zero-hard-
  violation source-size ratchet, and `git diff --check` pass. New production modules remain below
  200 lines.
- The scoped System Controls cohort passes 54 focused tests, all 1,625 frontend tests in 223 files,
  frontend typecheck, and the production build; Vite reports only the existing large-chunk
  advisory. Focused coverage proves closed-modal dormancy, exact mapped/direct runtime hydration,
  loading refusal, per-source and deduplicated aggregate release, replay/retry, both HTTP/event
  orders, Show replacement, stale-row suppression, and absence of broad Playback or bootstrap
  reads. All changed production files remain below 400 lines, and `git diff --check` passes.
- Direct Group backend authority passes 335 application tests, 64 wire tests, generated-contract
  verification, and 370 server tests with one intentional ignore. The sandbox-blocked CITP socket
  case passes when rerun with loopback access. All six new Group route/event/validation scenarios,
  strict Clippy for application/wire/server, formatting, architecture, and `git diff --check` pass.
  The Group service tests were split and the largest touched test file is 1,197 lines; production
  files remain below 400 lines.
- The integrated direct-Group frontend and broad-snapshot retirement pass 138 focused Group/runtime
  tests, 64 focused bootstrap/event/client tests, and all 1,646 frontend tests in 225 files.
  Frontend typecheck and the production build pass with only the existing Vite chunk advisory.
  Coverage proves dormant exact Group snapshots/subscriptions, direct and mapped masters, stored-
  empty/ordered membership, optimistic response/event ordering, rollback, replay/no-change, gap
  repair, opaque-ID collision safety, scope replacement, Fixture Sheet loading refusal, no initial/
  Show-open/object-event broad Playback fetch, and retained exact Screens refresh. Architecture,
  all 10 source-size scanner tests, all eight command-boundary scanner tests, zero hard-limit
  violations, and `git diff --check` pass; the largest touched production file is 395 lines.
- Preset recall passes 54 focused transport/provider/writer/UI tests and the complete 1,701-test
  frontend suite. Programmer Priority passes 53 focused store/session/writer/provider tests, 29
  strict wire/transport tests, and the complete 1,754-test frontend suite in 241 files. Both pass
  typecheck, production build, Biome, architecture, source-size, command-boundary, and whitespace
  checks; the build retains only the existing large-chunk advisory. Coverage includes first-view
  dormancy, exact-user traffic, same-user desk convergence, response/event ordering, replay/no-
  change, rollback, repair, gaps, tombstones, scope replacement, late responses, and unrelated-
  render suppression.
- The public Priority/Preset intent cohort passes 6 focused Vitest cases, both Priority API cases,
  both supplemental Preset cases, and OSC-004 through both API and OSC surfaces. The widened
  command-boundary scanner passes all 11 unit tests and the aggregate architecture command. The
  sole exploratory MERGE-002 UI timeout waited for an unrelated Playback card; its API sibling
  passed twice. The hosted-picker adapter is included in the green full frontend suite and private-
  boundary architecture ratchet.
- Typed Preload lifecycle passes 8 focused application tests; the full application, Programmer,
  Engine, and Playback suites at 355, 91, 74, and 88 tests; 8 focused server route/compatibility
  tests plus atomic failed-GO rollback; all 69 wire tests and generated-contract verification; and
  the unrestricted full server library at 381 passed and 1 ignored. The sandboxed server run fails
  only the known CITP localhost bind with `EPERM`. Strict server Clippy, formatting, architecture,
  source-size, and whitespace checks pass. Coverage includes every action/no-op, replay before
  stale preconditions, all captured revision conflicts, portable Show versus Engine divergence,
  targeted cursor acceptance/conflict/gap behavior, exact event cardinality, shared user desks,
  foreign rejection, rollback, and v1 payload/event compatibility.
- The scoped Output frontend core passes 63 focused transport/store/session/writer/view tests. Its
  production composition passes 87 focused tests and the complete 1,853-test frontend suite in 257
  files, plus typecheck, production build, architecture, source-size, and whitespace gates. The
  public Output intent adds 8 strict decoder/authority cases; the complete intent config passes 41
  tests, and SHOW-001 restart passes with its safe-blackout and byte-identical first-frame
  assertions intact.
- The typed Speed Group backend passes 11 focused and all 371 application tests, 16 focused server
  cases, 394 full server tests with one intentional ignore, all 74 wire tests, generated contracts,
  strict Clippy, and formatting. The sandbox-denied CITP localhost case passes separately with
  loopback access. Compatibility reset/history/payload behavior and exact event filtering pass;
  the largest new production file is 284 lines.
- The first two public Programmer-values cohorts pass all 12 helper tests, the 41-test aggregate
  intent config, six focused cue-semantic API scenarios, focused DMX/Position/Color paths,
  typecheck, production build, architecture, source-size, generated contracts, the command-boundary
  scanner and its 11 unit tests, and whitespace validation. Five cue UI runs remain blocked after
  successful strict setup by the separately tracked absent Playback-card controls.
- The current checkpoint adds 61 focused Visualization contract/consumer tests and passes the full
  frontend suite at 1,927 tests in 268 files, frontend typecheck, production build, dependency
  direction, source-size, and whitespace checks. The shared runtime proves first-view dormancy,
  one non-overlapping fastest poll per lane, independent Preload authority, selector isolation,
  exact wire/lane rejection, immediate scope clearing, and late-response suppression. Cue deletion
  adds 10 strict helper/ownership tests, three migrated public API scenarios, and one dedicated v1
  compatibility scenario; all pass, as do the 73-test aggregate intent configuration, the command-
  boundary ratchet, and its 11 scanner tests. The source-size ratchet reports zero files above
  1,200 lines and zero functions above 150 lines; design-goal debt remains at 138 production files
  above 400 lines and 5,833 functions above 20 lines.
