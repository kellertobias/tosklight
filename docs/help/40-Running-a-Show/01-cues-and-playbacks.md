# Cues and Playbacks

A Cuelist contains ordered Cues. A playback is an operator control assigned to a Cuelist, Group, or specialized master.

![Cuelist and assigned playback controls](../assets/screenshots/cuelist-playback.png)

## Assign controls

Arm **SET**, then choose a playback button or fader. Select the target, fader behavior, button actions, color, GO activation, auto-off, and crossfade time. Button actions include GO, GO minus, pause, release, flash, and temporary behavior where applicable. Assignment persists in the show and is page-aware.

## Run Cues

GO advances to the next Cue and applies its tracking state with configured timing. GO minus reconstructs the previous Cue rather than relying on programmer residue. Pause freezes a transition; GO continues. Release removes the playback's ownership and permits lower-priority sources to become visible.

The active playback is an explicit operator selection. Running another playback must not silently steal that selection. Cuelist View shows current/next state, Cue detail, and playback configuration.

## Restart behavior

First and Continue policies determine how a Cuelist starts after release or restart. Looping and chaser modes change end-of-list behavior. Test the exact production policy after a real server/app restart.
