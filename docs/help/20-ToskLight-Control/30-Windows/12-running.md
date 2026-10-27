# Running objects

The Running window collects one row for every independently stoppable runtime object. It shows running Cuelists, standalone Dynamics, Timecodes, and Macros by default. Several software, hardware, OSC, API, or playback controls that address the same shared runtime still produce one row.

A Cuelist row uses the Cuelist's own number and name and shows its current Cue number. It does not use a playback page or fader-assignment number. A Dynamic contained by that Cuelist is subordinate activity and does not receive another row; **Off** on the Cuelist row releases the containing Cuelist. A standalone Dynamic, running Timecode, or active Macro execution receives its own row, its own stable identity where defined, and the explicit **Cue —** value.

Every **Off** button targets exactly the object named by its row: release Cuelist, stop standalone Dynamic, stop Timecode, or cancel Macro execution. The button reports progress while that action is pending and cannot submit a duplicate action. Start, current-Cue, pause, resume, completion, release, cancellation, and stop changes reconcile from authoritative runtime state while the window remains open.

Use **All**, **Cuelists**, **Dynamics**, **Timecodes**, or **Macros** to restrict the list. An empty filtered list says which kind is not running instead of appearing broken.

Add Running to a Desktop with **Open Window**. It is a pane-only window and does not appear in the BUILT-INS dock.

**Pane configuration:** **Running kind** stores All, Cuelists, Dynamics, Timecodes, or Macros for this pane. Common size and removal controls also apply.
