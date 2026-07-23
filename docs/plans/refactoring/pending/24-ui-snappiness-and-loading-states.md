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
