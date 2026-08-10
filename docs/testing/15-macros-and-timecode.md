# Macros and Timecode

These scenarios are the operator acceptance contract for TL-71 Macros and TL-70 Timecode. Use one authenticated desk session, one second session on the same desk, one physical Playback, one Virtual Playback, an OSC control path, and—where named—an actual audio output and external Timecode source.

## MACRO-001 — Pool gestures and authoritative validation

Create a Macro containing comments, blank lines, state-dependent ordinary commands, and a later invalid command. A normal pointer click and touch tap run the Macro. Right-click and **SET**, then click/tap, open the editor without running. Save and Run reject the invalid line with its exact line number before any live mutation. After correction, every line appears in ordinary command history with one Macro execution identity and Macro provenance.

## MACRO-002 — Serialization, failure, cancellation, and Run-line Undo

Start a multi-line Macro and attempt a manual command and a second Macro from another session. No partial command interaction interleaves. Cancel between lines and confirm accepted earlier commands remain. Make one undoable **Run line** change and confirm **Undo last run** uses authoritative Undo. Then intervene with another accepted command or change the Macro revision and confirm Undo is disabled with an explanation.

## MACRO-003 — One-shot Playback convergence

Assign one Macro to physical and Virtual Playbacks. Start it from pool, physical Playback, Virtual Playback, OSC, HTTP, and WebSocket. Every path creates the same one-shot execution model and authenticated provenance. Cue, GO-minus, Pause, fader, tracking, and persistent-runtime controls are absent or inert with an explanation.

## TIMECODE-001 — Deterministic edit, seek, and loop

Create a duration-only Timecode with ordered same-frame Cuelist and Speed Group actions, markers, and a loop. Place and move keyframes with frame snapping and marker snapping. Compare continuous play, direct seek, and a loop into the same frame: the authoritative reconstructed Cuelist, Speed Group phase/BPM, and continuous values are identical, while discrete triggers are neither lost nor duplicated.

Zoom and scroll the production editor, scrub without changing live output, then explicitly seek the runtime to the editor playhead. Copy and delete each supported timeline item and use editor Undo/Redo. Import marker CSV once with **Append** and once with **Replace**, using both frame-number and `HH:MM:SS:FF` positions; malformed or out-of-range rows must reject the complete import.

## TIMECODE-002 — Cuelist clips and safe Preload

Create State Start and Cue Start clips for one Cuelist, including Hold and Release endings and one missing Cue reference. Confirm State Start reconstructs the intended tracked state, Cue Start executes the chosen Cue, Hold persists until superseded, Release ends the clip, and the missing reference remains visible but is skipped. Scrubbing and editing in Preload do not reach live output; Preload Play begins only on Preload GO.

## TIMECODE-003 — Shared transport and Cue graph

Assign one Timecode to physical and Virtual Playbacks and also start it standalone. Pool, editor, physical, Virtual, OSC, HTTP, and WebSocket controls all address one shared position and running state. A Cue start action begins at its explicit stored position and runs independently; stop ends it. Follow waits for the latest Cue fade or started Timecode completion. Direct and indirect Cuelist-to-Timecode recursion are rejected without mutation.

## TIMECODE-004 — External source, restart, and audible audio

Import PCM WAV and MP3; confirm MP3 is normalized into the managed portable format and the original path is not required after Save As/export. Select one exact external source, an audio output, and a per-output latency trim. Verify audible GO, Pause/Resume, seek, Stop, Rewind, and loop on the real device. Exercise each configured source-loss policy and immediate re-lock. Restart the server: internal Timecodes remain stopped, while an armed externally synchronized Timecode reconstructs from the current external position. Unplugging or changing the selected audio device reports an actionable error without stopping the lighting output engine.

Confirm the editor waveform follows actual decoded samples immediately after import and after closing and reopening the saved Timecode. Configure **ArtTimeCode UDP bind**, send valid Art-Net ArtTimeCode, and select its exact normalized identity; malformed packets and a different sender must not take authority. Do not use CITP as a Timecode source: validate CITP/MSEX separately through the Media pane scenario.
