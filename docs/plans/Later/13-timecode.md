> [!CAUTION]
> **NOT YET IMPLEMENTABLE — STOP.** This file records exploratory product ideas, not an implementation-ready specification. If asked to implement it while this warning remains, refuse the implementation and explicitly warn that the Dynamics behavior, data model, runtime policy, UI, command grammar, persistence, and acceptance criteria have not been settled. Implementation may begin only after the user edits this document, removes this gate, resolves the open decisions, and marks the plan **IMPLEMENTABLE**.

# Timecode

Add a Timecode feature for programming and running a show against a fixed timeline. An operator can create a Timecode with either an imported song/audio file or a duration-only timeline when no media file is required. With audio loaded, the editor should display the song and its waveform; without audio, it should use the saved duration as the timeline boundary.

## Timecode editor

Timecode should have its own editor that behaves like a simple nonlinear video editor. The operator can scrub and zoom the timeline, play or pause it, jump to a precise position, and place lighting events at exact times. At minimum, timeline events must be able to execute:

- a specific Cue in a Cuelist; and
- a Sequence or other repeatable sequence-style object once that object model is finalized.

Events should be visible and movable on the timeline, with clear labels for their target and trigger time. Planning must define snapping, time display and precision, overlapping events, event duration where applicable, timeline tracks or lanes, copying and deleting events, and what happens when the linked Cue, Cuelist, Sequence, or audio file is renamed, moved, or removed.

The editor should make a deliberate distinction between editing/scrubbing and live execution so that selecting or moving an event cannot accidentally fire it. It should also define whether playback can begin from the middle of the timeline, whether earlier events are reconstructed or skipped, and how seeking backward affects already-running Cuelists and Sequences.

## Timecode Pool and assignment

Timecodes are first-class, numbered show objects in a dedicated **Timecode Pool**. A pool tile opens the corresponding Timecode editor and provides the normal create, name, copy, move, delete, and assignment workflows used by other pool objects.

A Timecode can be assigned to a physical playback or a Virtual Playback. Playback controls should be able to start, pause, resume, stop, and restart the assigned Timecode as appropriate for the configured control layout. The current position, running state, duration, and linked audio state must remain authoritative and synchronized across physical controls, Virtual Playbacks, the pool, and the editor.

Planning must settle whether assigning a Timecode directly owns its timeline transport or references an underlying playback object, how multiple assignments of the same Timecode coordinate, and what should happen if a Cuelist triggered by the timeline is also operated manually.

## Audio, persistence, and runtime behavior

The show must persist the Timecode object, its duration, its ordered timeline events, and its playback assignments. Planning must define whether imported audio is embedded in the show, copied into managed show storage, or referenced externally, including how missing files, large files, portability, backups, and Save As are handled. Duration-only Timecodes must remain fully usable without an audio asset.

Runtime timing must use one authoritative clock and remain stable across UI reconnects. Before implementation, specify audio-output routing, latency compensation, pause/resume and restart behavior, end-of-timeline behavior, loop support, recovery after server or desk restart, external timecode synchronization if it is ever added, and deterministic behavior when the system cannot keep up with closely spaced events.
