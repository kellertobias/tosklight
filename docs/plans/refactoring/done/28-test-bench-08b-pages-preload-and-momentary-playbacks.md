# 08b — Pages, Preload, and momentary Playback gestures

## Outcome

Complete parent step 08's page addressing, mapping, independently paged screens, Preload, and
temporary Playback gesture surface.

## Public helpers

- `playback.flash/temp/swap(...)` with explicit press/release and readable hold forms;
- `playback.map({ page, slot, playback })`;
- typed current-page slot and explicit page-plus-slot targets;
- `page.select/next/previous/create/rename(...)`;
- `preload.start/commit/clear(...)`;
- normalized page and Preload assertions.

## Helper-contract scenarios

1. Address the same slot before and after a page change and prove current-page semantics.
2. Map and configure Playback controls through typed definitions.
3. Prove Flash, Temp, and Swap down/up behavior through UI and OSC.
4. Move a Playback fader and assert HTP/runtime behavior without generalizing it to Programmer
   LTP.
5. Start, inspect, commit, and clear Preload.
6. Change an independently paged browser screen and prove Main remains unchanged.

## Done gate

- Current-page, explicit-page, concrete Playback, and independent-screen targets are distinct
  types.
- Temporary controls retain gesture phases on every declared route.
- Preload helpers expose authoritative pending and committed state.

## Result

- Added typed concrete, current-Page, explicit-Page, and independent-screen Page targets.
- Added Page creation, naming, mapping, UI/API selection, and dedicated secondary-screen
  assertions.
- Added authoritative Preload lifecycle and pending-value helpers.
- Added Flash, Temp, and Swap press/release/hold helpers with real browser pointer gestures and
  subscribed OSC routes.
- Added focused scenarios proving Page authority boundaries, Preload pending/commit behavior,
  and browser-touch plus OSC momentary phases.

Verification:

- `npm run test:e2e -- tests/testBench/08b-pages-preload-and-momentary-playbacks.spec.ts`
  — 2 passed.
- `npm run test:e2e` — 323 passed, 9 skipped.
- `npm --prefix apps/control-ui run typecheck` — passed.
- `npm run test:bench-types` — passed.
- `npm run test:architecture` — passed.
- `node tools/check-source-size.mjs` — passed.
- `git diff --check` — passed.
