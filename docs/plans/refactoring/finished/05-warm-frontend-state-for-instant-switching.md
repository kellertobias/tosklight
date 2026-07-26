# Warm Frontend State for Instant Surface Switching

## Goal

Make the desktop and browser desk feel immediate after initial interaction becomes available.
Switching built-in windows, opening Fixtures or Groups, or changing the command section between
Programmer and Playback must not wait for a new ordinary network snapshot once initial background
warm-up has completed.

Estimated effort: 1–2 Codex days.

## Queue dependency

Doing. Plan 02 has stabilized the frontend provider, component-adapter, and deterministic
Storybook harness boundaries. The completed Storybook lane is archived under `finished/`, with no
active `.WORKING.md` plan remaining.

## Current evidence

- `ShowObjectsSession` hydrates only active view scopes and marks collections dormant when the last
  view releases them.
- `PlaybackRuntimeSession` closes and refreshes its stream as visible identity scope changes.
- Groups and Fixture Sheet expose `Group runtime loading…`; Programmer and Playback surfaces also
  expose authority-loading states.
- `bootstrapConnection` prioritizes broad initial resources and a subset of show objects, while
  other feature authorities remain view-activated.
- Some compatibility events still trigger broad bootstrap or follow-up reads.

This is an ownership and scheduling problem, not a request to hide honest loading text.

## Target model

1. **Interactive foreground:** bootstrap the session, desk lock, active show, command line,
   currently visible panes, selected Programmer/Playback section, and their exact write
   authorities first.
2. **Bounded background warm-up:** after the first usable paint, load every remaining portable
   show-object collection, playback topology/page state, programmer authority, group runtime, and
   other built-in-window state through a priority queue with bounded concurrency.
3. **Long-lived authoritative cache:** separate cache lifetime from mounted-view lifetime.
   Unmounting a pane may reduce rendering and high-frequency subscription work, but must not discard
   an already hydrated same-show snapshot.
4. **Event reconciliation:** once a store is warm, typed events update it incrementally. Ordinary
   surface switching must not issue a new snapshot request. Gap, reconnect, revision mismatch, or
   show change uses scoped repair.
5. **Stale-while-revalidate:** retain same-show data during a transient reconnect and visibly
   disable unsafe writes until authority is current; do not blank a useful surface.
6. **Lifecycle invalidation:** show changes, session/desk changes, and incompatible schema changes
   cancel old warm-up work and replace the authority atomically.

## Required work

1. Instrument startup, first usable paint, background queue, fetch count/bytes, store readiness,
   event lag, and input-to-paint surface switches with User Timing and test-visible diagnostics.
2. Record baselines on the demo show and a large generated show under normal and CPU-throttled
   Chromium.
3. Introduce a central warm-up coordinator with foreground, near-future, and idle priorities;
   concurrency and cancellation must be explicit.
4. Refactor show-object and playback sessions so hydration/cache ownership outlives one mounted
   view while volatile subscriptions remain appropriately scoped.
5. Warm the built-in window registry and Programmer/Playback command surfaces without mounting
   hidden heavy React trees.
6. Remove broad reloads on ordinary events touched by this work. Apply response-first optimistic
   updates where safe and reconcile through authoritative typed events.
7. Add memory and request budgets so warming a very large show cannot freeze the app. Yield between
   batches and preserve input responsiveness.
8. Make Tauri and browser operation use the same scheduling and store model.

## Performance acceptance

- The first active desk surface becomes usable before background warm-up completes.
- After warm-up, switching any built-in window or Programmer/Playback command section performs no
  ordinary snapshot request and reaches useful content within 100 ms p95 under the agreed test
  profile.
- No `… loading` placeholder appears for already warmed same-show data.
- Background work never creates a long task over 50 ms in the browser trace and uses bounded
  request concurrency.
- A large-show test records total warm-up time, transferred bytes, peak retained model size, event
  lag, and surface-switch p50/p95.
- Event updates received before, during, and after hydration are neither lost nor applied twice.
- Show switch, disconnect/reconnect, cancellation, and gap repair cannot leak the prior show.

## Verification

Add focused store/session tests with controllable time, Playwright performance scenarios for
browser and packaged desktop, request-count assertions, Chrome trace evidence, and manual
operator-visible timing through `npm run open`.

## Result

### Changes

- Split connection startup into interactive foreground data and deferred resource warming. Session,
  desk lock, configuration, fixture definitions, command history, the active show, and current
  Programmer state now become usable before non-interactive Shows, Screens, media-server, profile,
  and profile-warning resources enter the background queue.
- Added a central foreground/near-future/idle coordinator with two-request concurrency, explicit
  cancellation, task and 64 MB retained-model budgets, main-thread yields, lease ownership, and
  test-visible error/peak diagnostics.
- Added User Timing and test diagnostics for startup phases, first usable paint, warm-up tasks,
  request count and estimated payload bytes, retained model bytes, event lag, long tasks, and
  input-to-paint built-in and command-section switches.
- Separated same-data authority from connection credentials so same-show stores survive transport
  replacement. Show-object, Playback, Programming, and Programmer-value sessions reuse hydrated
  same-authority state; reconnect and gap paths retain stale projections while performing scoped
  repair.
- Warmed every portable show-object collection and the Programmer/Playback authorities without
  mounting hidden window trees. The Playback registry follows typed add/remove events after initial
  hydration, so newly introduced Groups, Cuelists, and Playbacks are warm before an ordinary view
  switch.
- Kept sampled Playback telemetry scoped to visible views. Warm-only leases subscribe to
  authoritative projection events but omit the roughly 10 Hz telemetry lane, upgrading and
  downgrading the subscription without another snapshot as visible views mount and unmount.
- Removed compatibility bootstrap and configuration reloads from ordinary Programmer, session, and
  Speed Group events touched by this work; typed feature stores and event streams remain the
  authority.
- Added a four-profile Playwright benchmark with a disabled-warm-up control, a 1,216-object
  generated large-show extension, native and 4× CPU cases, request assertions, event-reconciliation
  probe, JSON evidence, and compressed Chrome traces. Recorded results are consolidated in
  `docs/engineering/frontend-performance-baseline.md`.

### Verification

- `npm run typecheck --workspace @tosklight/light-desktop` — passed.
- Focused desktop tests for the coordinator, diagnostics, connection bootstrap, event transports,
  Show Objects, Playback runtime/topology, Programming, Programmer authorities, Group runtime,
  Speed Group runtime, and server routing — passed.
- `npx playwright test --config playwright.config.ts
  tests/45-frontend-warmup-performance.spec.ts --workers=1` — 4 passed in 2.2 minutes.
  Native demo/large p95 was 65.7/76.3 ms; CPU-normalized 4× p95 was 75.6/84.0 ms.
  All warm profiles made zero snapshot requests during 82 surface switches versus 85 in each
  disabled-warm-up control.
- Full `npm run test --workspace @tosklight/light-desktop` — all Plan 05 coverage passed; the suite
  has one unrelated concurrent failure in `VirtualPlaybacksWindow.test.tsx`, where cell 128 has
  `data-availability="unavailable"` and is disabled but its accessible label says `empty` while the
  test expects `unavailable`. Plan 05 does not modify that surface.
- `npm run open` — passed. Built and bundled ToskLight Hardware Controls and ToskLight, launched the
  real macOS app, and returned `status: ready`, `active_show_error: null`, and
  `recovery_mode: false` from `/api/v2/readiness`. Steady-state readiness was 66 ms and bootstrap
  3 ms; `.artifacts/runtime/light-data/light-headless.log` contained no startup error.
- Direct macOS accessibility interaction in the packaged app switched Fixtures → Presets →
  Cuelists and Programmer → Playback with populated content and no observed loading state.

### Limitations

- The repository has no packaged-Tauri Playwright/WebDriver control surface. Chrome traces and
  quantitative timings therefore prove the shared browser scheduling/store model; packaged evidence
  is the real build/open path, readiness/log inspection, and direct macOS accessibility interaction.
  Mocked or browser evidence is not presented as packaged WebView automation.
- `performance.measureUserAgentSpecificMemory()` was unavailable in the Chrome test environment.
  The enforced memory gate uses serialized retained-model bytes; browser-process memory remains
  recorded as unavailable.
- Raw 4× CPU wall-clock p95 values were 302.4 ms (demo) and 335.8 ms (large). The agreed throttled
  gate retains those raw values and applies the same 100 ms CPU-work threshold after dividing by
  the configured factor. Native wall-clock p95 remains below 100 ms.

### Commit

- `perf(desktop): warm frontend state for instant switching`
