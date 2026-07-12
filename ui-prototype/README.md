# UI click dummy

This standalone prototype explores the shared browser/Tauri programming interface. It contains no backend integration and intentionally uses mock show data.

Open `index.html` directly or serve the directory:

```sh
python3 -m http.server 4173 --directory ui-prototype
```

The prototype supports:

- A grid-based programming desktop demonstrating simultaneous Presets, Fixture Sheet, and Stage panes.
- A new desk starts with empty grid cells. Selecting one opens the Window picker. Pane Settings controls only grid width, grid height, full screen, and removal; pane headers intentionally have no direct close button.
- Pane selection is outlined only while Pane Settings is open. Desk Save asks for an existing desk or New Desk as its destination.
- A two-mode left Dock: saved Desks versus individual Built-ins. In Desk mode, Built-ins stays anchored at the bottom; in Built-ins mode, both mode keys are stacked at the top above the built-in windows.
- The global top status bar is intentionally removed. The Dock owns application identity and its fixed Setup block contains show/change state, a large wall clock, and Quick Setup.
- Full views for Stage, Fixture Sheet, Presets, Playback/Sequence editing, Dynamics, intensity-only Channels, DMX output, and Setup.
- Setup subsections covering shows/recovery, users/sessions, inputs, outputs, timecode, network/API, safety, and diagnostics.
- A Control Section with Programmer and playback-fader modes.
- Programmer/Playbacks is one graphical state toggle with left/right icons and stacked state labels. The command bar also owns compact two-line DMX and timecode status.
- Status sits directly left of a fixed-width Preload button. Preload aligns with the edge of the parameter/fader area; the numpad or playback timing tools occupy the full right-hand height.
- The playback Control Section fills the area left of a fixed tools column containing four speed-group keys, a Preload/programmer fade slider, and a sequence-master fade slider.
- Speed groups are stacked vertically. Both fade controls are full-height touch surfaces with value fill, not conventional horizontal sliders.
- Programmer-owned Preload and Preload Go with an explicit fade time.
- A single stateful Preload button: enter blind preload, merge to the active preload-output layer, then begin the next preload. The active merged layer is visible and releasable.
- The merged layer appears as a compact `Preload Scene / Release` control next to Fade; pressing any part of it releases the layer. It is no longer duplicated in a global status bar.
- Preload has a fixed width and cycles only `PRELOAD` → `PRELOAD GO` → `PRELOAD`.
- Quick Setup exposes show metadata, save/load/export actions, patch/setup entry, shutdown, and MIDI-controller profile selection. Touch surfaces are opened from Programmer’s Special Dialogs action.
- A Dynamics editor for time-varying attribute values, replacing the user-facing term “phaser.”
- Compact source-aware fixture cells, a dedicated Group Pool, fixed-size preset slots, selectable intensity Channels, and an inspectable/touch-expandable DMX dot monitor.
- Stage group shortcuts use ten framed, pool-sized tiles configurable from Stage settings. Fixture Sheet rows are compressed to 43 px.
- DMX tooltips can be pinned by clicking a value, operated with a diagnostic slider, and dismissed explicitly. Each universe has a dedicated Expand control.
- An expanded touch profile and manually collapsed hardware-assisted profile.
- A portrait positioning remote with fixture command entry, pan/tilt pad, touch faders, keypad, and fixture navigation controls.

Navigation, mode switching, Preload, desk save/new desk, window picking, pane sizing/full screen/removal, Quick Setup, Special Dialogs, DMX inspection, and representative touch controls are interactive. Remaining controls are visual proposals for discussion.
