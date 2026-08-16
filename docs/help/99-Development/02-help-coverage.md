# Help Coverage

This matrix is the completeness contract for operator Help and the generated manual. New built-in windows or major workflows must add or update a row and, where visual explanation helps, extend `tests/02-help-screenshots.spec.ts`.

| Application area | Help coverage | Screenshot |
| --- | --- | --- |
| Shell, Show menu, Desktops, panes, window settings | [Application Layout and Window Manager](../10-Desk/30-Windows/01-desk-interface-and-windows.md) | `default-desk-overview.png` |
| Every operator Open Window pane and its pane-specific settings | [Pane Reference](../10-Desk/30-Windows/index.md) | `panes/*.png`; common-only settings dialogs are not embedded |
| Installation, desktop/server start, LAN token | [Installation and First Start](../00-Quick-Start/01-installation-and-first-start.md) | Not required |
| Screens and playback page modes | [Screens and Desktop Layouts](../10-Desk/10-Show-Setup/02-screens-and-layouts.md) | `desk-setup-screens.png`; native additional-screen card requires desktop QA |
| OSC, native extensions, REST, WebSocket | [OSC, Extensions, and Network Control](../10-Desk/10-Show-Setup/03-inputs-extensions-and-network.md), [Native Extension Development](05-native-extension-development.md), and [Protocol Reference](../90-Protocols/01-osc.md) | `desk-setup-inputs.png`, `desk-setup-network-api.png` |
| Output engine, DMX, Art-Net, sACN, overrides | [DMX Output and Universe Routes](../10-Desk/10-Show-Setup/04-dmx-output.md) | `desk-setup-output-engine.png` with the Outputs route editor, plus selected-channel DMX pane |
| Users, sessions, recovery | [Users, Sessions, and Recovery](../10-Desk/10-Show-Setup/05-users-sessions-and-recovery.md) | Users, Change User, recovery, and load/revision workflow images |
| Native shows, autosave, revisions, MVR | [Shows, Revisions, and MVR](../10-Desk/10-Show-Setup/10-shows-revisions-and-mvr.md) | Show menu, revisions, new-MVR, and export-MVR images |
| GDTF, Fixture Share files, local fixture creation/revisions | [Fixture Library](../10-Desk/10-Show-Setup/11-fixture-types-and-gdtf.md) | Library, Import GDTF, and Create fixture images |
| Patch, unpatched fixtures, multi-patch, multi-head | [Fixtures and Patch](../10-Desk/10-Show-Setup/12-patch-fixtures-and-scenery.md) | `show-patch.png`, `patch-add-fixture.png` |
| 2D/3D Stage, position setup, scenery/models | [Stage Positions and Scenery](../10-Desk/10-Show-Setup/13-stage-positions-and-scenery.md) | `stage-window-2d.png`, `stage-settings.png`; 3D scenery requires desktop QA |
| Groups and Presets | [Groups and Presets](../10-Desk/20-Programmer-and-Cues/03-groups-and-presets.md) | `default-desk-overview.png` |
| Programmer selection, PREV/NEXT/ALL stepping, Fixture Sheet remembered-base/current-step treatment, independent HIGH, top-layer errors, clear, undo, multiple users | [Selecting and Setting Values](../10-Desk/20-Programmer-and-Cues/02-selecting-and-setting-values.md) | `fixture-sheet-programmer.png` plus exact HIGH state in `software-keypad.png` |
| Command line, fixed Highlight-key columns, no Highlight status panel, software 2×2 Programmer Fade, simulator RECORD/Preload and adjacent faders, keyboard shortcuts | [Command Line Reference](../10-Desk/20-Programmer-and-Cues/01-command-line.md) | `software-keypad.png` and `help-command-line.png`; simulator geometry requires desktop QA |
| Cue record/edit/timing/triggers | [Programming Cues](../10-Desk/20-Programmer-and-Cues/04-programming-cues.md) | `cuelist-playback.png` |
| Stage, Fixtures, Groups, Presets, and planned Dynamics | [Programming Windows](../10-Desk/30-Windows/06-programming-windows.md) | Programming and fixture images |
| Channels intensity bank and paging | [Channel Faders](../10-Desk/30-Windows/07-channel-faders.md) | `panes/channels.png` |
| Cuelists, Playbacks, buttons/faders, page behavior | [Cues and Playbacks](../10-Desk/20-Programmer-and-Cues/10-cues-and-playbacks.md) | `cuelist-playback.png` |
| HTP, LTP, priorities, tracking, source ownership | [HTP, LTP, and Ownership](../10-Desk/20-Programmer-and-Cues/11-htp-ltp-and-ownership.md) | Fixture sheet source cells |
| Preload, capture domains, Preload GO/release | [Preload and Preload GO](../10-Desk/20-Programmer-and-Cues/12-preload.md) | Capture switches plus existing Stage/Fixture comparison images |
| Follow/timecode, Timecode Pool/editor/audio, Macros, Chasers, Speed Groups | [Triggers, Chasers, and Speed Groups](../10-Desk/20-Programmer-and-Cues/13-triggers-chasers-and-speed.md), [Cue and Playback Panes](../10-Desk/30-Windows/03-cues-and-playbacks.md#timecode), and [Macros](../10-Desk/30-Windows/03-cues-and-playbacks.md#macros) | `desk-setup-timecode.png`; focused Timecode/Macro operator acceptance required |
| Virtual Playbacks | [Virtual Playbacks](../10-Desk/20-Programmer-and-Cues/14-virtual-playbacks.md) | Pane and pane-settings images |
| Live Media operating surface and authoritative Running objects | [Programming and Visualization Panes](../10-Desk/30-Windows/02-programming-and-visualization.md#media), [Cue and Playback Panes](../10-Desk/30-Windows/03-cues-and-playbacks.md#running) | Focused UI acceptance; no external-server screenshot required |
| File Manager and Text Editor | [File Manager and Text Editor](03-file-manager-and-text-editor.md) | Covered by dedicated UI E2E |
| Help and generated PDF | [Manual and Help Screenshots](04-manual-and-help-screenshots.md) | `help-command-line.png` plus rendered-PDF QA |
| Development diagnostics and plans | [Development and Future Features](index.md) | Not required |
