# 24 — UI snappiness: explicit loading states, no broken-looking boot

## Context (maintainer requirement, 2026-07-23)

The desk must *feel* snappy, and every loading phase must be visibly a loading phase.
Today the app **looks broken until the show is loaded** — windows render empty/dead
during boot and show switches instead of stating what they are waiting for. The same
class of problem produced PLAYBACK-SELECT-001 (fader bank tore its grid down during
transient refetches) — that fix pattern (stay mounted, only a real scope reset returns
to a placeholder) generalizes here.

## Work

1. **Inventory the loading phases** first (this chunk's own discovery step; record the
   result in the result note): app boot (readiness → bootstrap → session → stores
   hydrating), show open/switch (blackout/fade transition + engine snapshot install +
   collection re-hydration), window mount on an already-running desk, and reconnect
   after a lost server. For each phase, note what the operator currently sees.
2. **Design one loading vocabulary** (shared component(s), not per-window inventions):
   a clear "what is happening" state — e.g. "Loading show <name>…" during show open —
   distinct from the empty states that mean "loaded, nothing here" (the existing
   `parameter-empty` / "No fixtures in this layer" pattern). Broken-looking
   intermediate renders (dead grids, blank panes, unstyled flashes) are defects.
3. Apply it to the phases from step 1, starting with the worst offender: **show
   loading**. The shell should present a deliberate loading surface until the desk is
   actually interactive; individual windows keep rendering their last valid content
   during transient refetches (PLAYBACK-SELECT-001 pattern) instead of unmounting.
4. **Snappiness pass:** while touching these paths, measure and remove obvious
   first-paint blockers (e.g. windows waiting serially on data they could render
   skeletons for; `/bootstrap` vs `/readiness` timing is already an AGENTS.md
   diagnostic). Do not micro-optimize beyond what a stopwatch can see — this chunk is
   about perceived state, not a performance project.
5. Respect the AGENTS.md UI rules: no silent long-running actions; loading states need
   visible progress where the duration is unbounded (show open, package import).

## Definition of done

- At no point between app launch and interactive desk does the UI show a
  broken-looking (empty/dead/unstyled) surface — every phase states what it is doing.
- Show switch shows a deliberate "loading show" state; open windows do not flash to
  empty during transient refetches.
- A short screen recording (or the demo test) of boot + show switch reads as
  intentional throughout; parity checked in software-only and hardware-connected
  layouts.

## Verification

```sh
npm run test:unit
npm run test:e2e-ui
npm run test:e2e   # full suite gate — PLAYBACK-SELECT-001 and SHOW-* especially
npm run open       # the real judgment call: boot + open a large show, watch every frame
```

## Decisions

None blocking. If the inventory in step 1 finds a phase whose fix needs server-side
changes (e.g. a progress event during show compile), file it as a follow-up chunk
rather than expanding this one.

Sequence: late in the queue by number, but it only depends on 13 (v2 events) loosely —
it may be claimed earlier if the boot path is stable; note deviations in the result.

Claimed on 2026-07-24 after Chunk 23 completed; no unresolved decision or
`.ATTENTION` suffix.

## Result

Completed on 2026-07-24.

### Loading-phase inventory

- **App boot:** the full-screen connection cover disappeared as soon as bootstrap
  data arrived, although session creation, resource loading, store hydration, and
  the WebSocket connection were still in progress. The shell could therefore look
  ready before it was interactive.
- **Show open/switch:** show lifecycle actions and the matching server event each
  refreshed the same collections without a shared busy state. Independent catalog,
  configuration, fixture, and media reads also ran serially.
- **Window mount:** Screen and Stage windows rendered fallback or empty content
  while their first scoped layout/data request was still pending.
- **Reconnect:** the main shell generally retained same-show state, but the
  full-screen boot cover returned and obscured that valid content.

### Changes

- Added one shared `LoadingSurface` vocabulary and desk-loading overlay, including
  accessible live status and busy semantics, for the main shell and secondary
  Screen/Stage windows.
- Kept the full boot cover until the first real connection completes. After that,
  reconnects retain the last valid desk and use a compact reconnect banner.
- Added a tokenized desk-loading controller so overlapping local show actions and
  server events cannot clear "Loading show …" prematurely. Show open, clean,
  import, revision, rollback, and show-switching save-as paths now use it.
- Kept mounted content stable during refreshes, gated a Screen window's initial
  render on actual layout hydration, coalesced identical in-flight show-object
  requests, and parallelized independent post-bootstrap reads.
- Added focused tests for boot/reconnect presentation, the shared overlay,
  overlapping activity, show lifecycle ordering, and server-event loading.

### Verification

- `npm run test:unit` passed: 281 frontend files and 1,997 frontend tests, plus the
  Rust unit suites and frontend production build.
- Focused loading and show-data coverage passed: 6 files and 51 tests.
- `npm run typecheck` and `node tools/check-source-size.mjs` passed.
- `npm run test:e2e-ui` reached 103 passed and 5 skipped; its product demo passed.
- `npm run test:e2e` reached 286 passed and 9 skipped; its product demo passed.
  Both E2E gates have one repeatable, unrelated failure in
  `COLOR-RANGE-001`, where conflict repair still reports a conflicting programmer
  outcome. Follow-up Chunk 26a records that defect rather than expanding this
  loading-state chunk.
- `npm run open` rebuilt both desktop applications, launched the current ToskLight
  bundle, and passed the build script's canonical launchd-server ownership and
  readiness check.
