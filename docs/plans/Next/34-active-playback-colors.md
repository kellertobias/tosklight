# Active Playback Colors

## Status and scope

Make the configured playback color the authoritative visual identity of a running playback while keeping “running” distinct from the explicitly selected playback context.

## Visual states

An inactive assigned playback shows a subdued version of its configured color. When its runtime contribution is On, temporary-active, or otherwise running, the card, applicable button feedback, and fader accent use a stronger version of that same configured color. Do not fall back to a generic green running treatment.

The explicit selected-playback state remains a separate outline/marker with the normal desk selection semantics. Loaded-next Cue, pickup required, paused, Swap, Record target, Update target, exclusion-zone membership, Blackout, and error states must remain readable when combined with light, dark, or highly saturated playback colors. Empty playbacks receive no configured-color state.

Apply the same state vocabulary to software Playback controls, Hardware-Connected Playbacks, and Virtual Playbacks wherever they render the same underlying playback definition. Runtime feedback, not the last pointer action, determines whether the playback is active.

## Acceptance criteria

1. GO/On/Temp and Off/release transitions update the configured-color treatment from authoritative runtime state.
2. Refresh, reconnect, page changes, auto-off, and restart restore the correct active color.
3. Explicit selection remains distinguishable from running state, including when selected but Off or running but not selected.
4. Light and dark palette colors preserve readable text and all combined state indicators.
5. Software, hardware-connected, and Virtual Playback surfaces agree for the same playback.
