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
