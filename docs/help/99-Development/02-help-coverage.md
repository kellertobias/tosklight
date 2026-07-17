# Help Coverage

This matrix is the completeness contract for operator Help and the generated manual. New built-in windows or major workflows must add or update a row and, where visual explanation helps, extend `tests/02-help-screenshots.spec.ts`.

| Application area | Help coverage | Screenshot |
| --- | --- | --- |
| Shell, Show menu, Desktops, panes, window settings | [Application Layout and Window Manager](../01-application-layout.md) | `default-desk-overview.png` |
| Every operator Open Window pane and its pane-specific settings | [Pane Reference](../05-Pane-Reference/index.md) | `panes/*.png`; common-only settings dialogs are not embedded |
| Installation, desktop/server start, LAN token | [Installation and First Start](../02-installation.md) | Not required |
| Screens and playback page modes | [Screens and Desktop Layouts](../10-Desk-Setup/01-screens-and-layouts.md) | `desk-setup-screens.png`; native additional-screen card requires desktop QA |
| OSC, MIDI, RTP-MIDI, REST, WebSocket | [OSC, MIDI, and Network Control](../10-Desk-Setup/02-osc-midi-and-network.md) and [Protocol Reference](../50-Protocols/01-osc-rest-and-websocket.md) | `desk-setup-inputs.png`, `desk-setup-network-api.png` |
| Output engine, DMX, Art-Net, sACN, overrides | [DMX Output and Universe Routes](../10-Desk-Setup/03-dmx-output.md) | `desk-setup-output-engine.png` with the Outputs route editor, plus selected-channel DMX pane |
| Users, sessions, recovery | [Users, Sessions, and Recovery](../10-Desk-Setup/04-users-sessions-and-recovery.md) | Users, Change User, recovery, and load/revision workflow images |
| Native shows, autosave, revisions, MVR | [Shows, Revisions, and MVR](../20-Show-Setup/02-shows-revisions-and-mvr.md) | Show menu, revisions, new-MVR, and export-MVR images |
| GDTF, Fixture Share files, local fixture creation/revisions | [Fixture Library](../20-Show-Setup/03-fixture-library.md) | Library, Import GDTF, and Create fixture images |
| Patch, unpatched fixtures, multi-patch, multi-head | [Fixtures and Patch](../20-Show-Setup/01-fixtures-and-patch.md) | `show-patch.png`, `patch-add-fixture.png` |
| 2D/3D Stage, position setup, scenery/models | [Stage Positions and Scenery](../20-Show-Setup/04-stage-positions-and-scenery.md) | `stage-setup-2d.png`, `stage-settings.png`; 3D scenery requires desktop QA |
| Groups and Presets | [Groups and Presets](../20-Show-Setup/05-groups-and-presets.md) | `default-desk-overview.png` |
| Programmer selection, PREV/NEXT/ALL stepping, Fixture Sheet remembered-base/current-step treatment, independent HIGH, top-layer errors, clear, undo, multiple users | [Selecting and Setting Values](../30-Programmer/02-selecting-and-setting-values.md) | `fixture-sheet-programmer.png` plus exact HIGH state in `software-keypad.png` |
| Command line, fixed Highlight-key columns, no Highlight status panel, software 2×2 Programmer Fade, simulator RECORD/Preload and adjacent faders, keyboard shortcuts | [Command Line Reference](../30-Programmer/01-command-line.md) | `software-keypad.png` and `help-command-line.png`; simulator geometry requires desktop QA |
| Cue record/edit/timing/triggers | [Programming Cues](../30-Programmer/03-programming-cues.md) | `cuelist-playback.png` |
| Stage, Fixtures, Groups, Presets, and planned Dynamics | [Programming Windows](../30-Programmer/04-programming-windows.md) | Programming and fixture images |
| Channels intensity bank and paging | [Channel Faders](../30-Programmer/05-channel-faders.md) | `panes/channels.png` |
| Cuelists, Playbacks, buttons/faders, page behavior | [Cues and Playbacks](../40-Running-a-Show/01-cues-and-playbacks.md) | `cuelist-playback.png` |
| HTP, LTP, priorities, tracking, source ownership | [HTP, LTP, and Ownership](../40-Running-a-Show/02-htp-ltp-and-ownership.md) | Fixture sheet source cells |
| Preload, capture domains, Preload GO/release | [Preload and Preload GO](../40-Running-a-Show/03-preload.md) | Capture switches plus existing Stage/Fixture comparison images |
| Follow/timecode, Chasers, Speed Groups | [Triggers, Chasers, and Speed Groups](../40-Running-a-Show/04-triggers-chasers-and-speed.md) | `desk-setup-timecode.png` |
| Virtual Playbacks | [Virtual Playbacks](../40-Running-a-Show/05-virtual-playbacks.md) | Pane and pane-settings images |
| File Manager and Text Editor | [File Manager and Text Editor](03-file-manager-and-text-editor.md) | Covered by dedicated UI E2E |
| Help and generated PDF | [Manual and Help Screenshots](04-manual-and-help-screenshots.md) | `help-command-line.png` plus rendered-PDF QA |
| Development diagnostics and plans | [Development and Future Features](index.md) | Not required |
