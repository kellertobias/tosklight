# Warm Frontend State for Instant Surface Switching

## Goal

Make the desktop and browser desk feel immediate after initial interaction becomes available.
Switching built-in windows, opening Fixtures or Groups, or changing the command section between
Programmer and Playback must not wait for a new ordinary network snapshot once initial background
warm-up has completed.

Estimated effort: 1–2 Codex days.

## Queue dependency

Pending, blocked until plan 02 stabilizes frontend provider, component-adapter, and deterministic
Storybook harness boundaries. Do not run it concurrently with the active Storybook lane: it changes
the same application contexts, stores, provider composition, window activation paths, and
frontend verification runtime.

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
