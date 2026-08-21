# Cue and Playback Windows

## Cuelist Pool

The Cuelist Pool is numbered storage for sequences and Chasers. A populated tile shows its Cuelist number and name, whether it is running, its master percentage, and any playback-page assignments. Tap a populated tile to open its Cues. Right-click it, or hold it on a touch surface, to open that exact Cuelist's settings. When a tile has a preview image, tap the image itself to inspect a larger modal view. Record plus a pool position creates or updates a Cuelist; Set workflows use an ordinary tile tap as the explicit assignment target. While Set is waiting for that source, every populated Cuelist tile receives the literal Set outline and the complete tile is the target; empty tiles remain ordinary and unoutlined.

The full window can search the 1,000-position pool by number or name using the shared [window search bar](01-desk-interface-and-windows.md#search-bars). Holding a populated tile opens Cuelist configuration: Sequence or Chaser mode, priority, HTP/LTP intensity mode, wrap behavior, restart behavior, timing overrides, Cue renumbering, and Chaser speed, multiplier, and crossfade. **Chaser X-fade** is stored from `0%` to `100%` of the effective step: `0%` snaps, `50%` fades for half the interval, and `100%` fades for the complete interval. Changing the Speed Group BPM or multiplier changes the live fade duration without changing the stored percentage. These settings change the Cuelist itself and must not be confused with Pane Settings.

**Pane configuration:** **Type colors / Individual colors** chooses the configured lime Cuelist treatment or explicit item colors for this pane. Search and Cuelist configuration belong to the full content window. Common size and removal controls also apply.

![Cuelist Pool pane](../../assets/screenshots/panes/cuelist-pool.png)

## Cues - Cuelist

This pane shows the Cues of one Cuelist. By default every row shows these columns, in order: **Preview**, **No.**, **Name**, **Trigger**, **Trigger Time**, **In Delay**, **In Fade**, **Out Delay**, and **Out Fade**. All nine columns remain available without changing the pane settings. Running and next Cues receive status highlighting. Selecting a row changes the current row selection but does not execute it.

Stage previews are pictures, not live 3D views. Each one is drawn once — when the Cue is recorded, when it is edited, when a fixture moves on the Stage, or when a Group it uses changes — and is then stored in the show alongside the Cue. Opening a Cuelist therefore draws nothing: the desk shows the stored pictures. Because they travel inside the show file, a Cuelist opened on another desk arrives with its previews already drawn, and a desk without 3D rendering still shows them. A Cue recorded from a desk that cannot draw has no preview until a desk that can opens that Cuelist.

The Cue table is the editing surface. Activate **Name** or **Info** to open the desk keyboard directly. **Jump** and **Trigger** open the same full grouped-choice model used for Playback button functions; LINK choices display the destination number and name while storing its stable identity. **Jump Count**, **Trigger Time**, **In Delay**, **In Fade**, **Out Delay**, and **Out Fade** open the number model directly. Out Fade also offers **Use Release**, linking it to the desk Release timing master; Out Delay offers **Use In Fade**, linking it to the Cue's effective In Fade. Linked cells show the source and effective time. Entering an explicit number makes the field independent again without discarding its previously stored explicit value. Committing the direct editor applies that one value through the authoritative Cuelist update; closing it leaves the stored value unchanged. Out timing controls decreasing or released Intensity independently, so an old look can overlap the entering look or leave an intentional black gap. Existing Cues without a selected link retain their stored explicit or inherited timing behavior. Selecting a row does not execute its Cue, and property editing no longer takes table width for a selected-Cue sidebar. Cue deletion remains an explicit Delete Cue command with a complete Cue address.

The timing cells use their background as a live progress display: empty means not started and filled means complete. **Trigger Time** starts at the authoritative beginning of the actual trigger interval: a TIME trigger begins when the preceding Cue receives GO, while FOLLOW and LINK begin after their source Cue finishes. **In Delay**, **In Fade**, **Out Delay**, and **Out Fade** each start when that phase actually starts and fill independently until that phase completes, including when incoming and outgoing work overlap. Pausing freezes every running fill; resuming continues it from the same point. A completed cell stays filled after the transition settles and while the operator changes rows or panes. It resets only when that Cue is triggered again as a new authoritative transition. These displays follow playback runtime rather than a browser-local timer, so reopening the view does not lose or restart progress.

Bare Copy or Move outlines every visible Cue row as a source; touching anywhere in one outlined row records its complete Cuelist-and-Cue address. The same rows become destinations after AT, including after navigating normally through the Cuelist Pool to another list. Bare Delete outlines each Cue row and touching anywhere inside the row executes the complete Delete Cue command. Set does not turn a Cue row into a target: it keeps ordinary row selection and, in the constrained-height editor, outlines only the exact editable value buttons with a literal SET badge. Cuelist navigation, Pane Settings, and table scrolling remain ordinary navigation while a target command is active.

The full window also provides navigation back to the Cuelist Pool and **Cuelist Settings**. Cuelist Settings opens as a modal over the view. Its title bar contains Save, Renumber Cues, and a two-line Mode menu showing the current Sequence or Chaser mode. The body groups numeric/intensity priority, wrap/restart behavior, and timing into three explained columns. Chasers additionally expose a typed Speed multiplier and a `0–100%` Chaser X-fade fader. Close leaves clean settings immediately; if settings are dirty, choose **Save changes**, **Discard changes**, or **Stay** explicitly. The pool's right-click and hold shortcuts open the same modal without starting assignment.

The compact pane starts fixed to Cuelist 1 or the first available list. In Pane Settings, **Displayed Cuelist** can keep the pane **Fixed** to any available Cuelist or **Follow selection**. Follow selection resolves the desk's explicitly selected playback and shows its Cuelist; selecting a Group playback or having no selected playback leaves the pane empty instead of switching to an unrelated list. While that Cuelist runs, the selected row and inline Cue editor follow its actual current Cue, including automatic Chaser steps. The fixed Cuelist choice and display mode are stored with that pane, so different Cues panes can remain on different lists while another follows the desk selection and active Cue.

**Pane configuration:** **Displayed Cuelist** selects Fixed or Follow selection, **Cuelist** chooses the fixed list, and **Compact Cue rows** reduces table-row height and hides Preview-column pictures. The common size and removal controls also apply. With the exact bare Delete command active, the complete Pane title receives a red DELETE target outline and removes that Pane after its normal confirmation; the Pane body, Settings control, resizing, and view navigation are never Delete targets.

![Cues pane](../../assets/screenshots/panes/cues.png)

## Cuelists (tabs)

This pane currently opens the Cuelist Pool and then replaces it with the selected list's Cue table. Despite its legacy label, the current implementation does not display multiple tabs. In compact mode the full-window Back control is hidden, so returning to the Pool requires reopening or replacing the pane. Treat this as a current interface limitation rather than as a multi-tab workspace.

Open **Cuelists** for the integrated Pool, Cue editing, and Cuelist configuration workflow. Open Window still offers **Cues** for a permanent Cue overview. Older saved Desktops can contain pool-only panes, but the catalog does not duplicate the integrated Cuelists workflow with a separate Cuelist Pool choice.

**Pane configuration:** only common size and removal controls.

![Cuelists pane](../../assets/screenshots/panes/cuelists.png)

## Running

The Running window collects one row for every independently stoppable runtime object. It shows running Cuelists, standalone Dynamics, Timecodes, and Macros by default. Several software, hardware, OSC, API, or playback controls that address the same shared runtime still produce one row.

A Cuelist row uses the Cuelist's own number and name and shows its current Cue number. It does not use a playback page or fader-assignment number. A Dynamic contained by that Cuelist is subordinate activity and does not receive another row; **Off** on the Cuelist row releases the containing Cuelist. A standalone Dynamic, running Timecode, or active Macro execution receives its own row, its own stable identity where defined, and the explicit **Cue —** value.

Every **Off** button targets exactly the object named by its row: release Cuelist, stop standalone Dynamic, stop Timecode, or cancel Macro execution. The button reports progress while that action is pending and cannot submit a duplicate action. Start, current-Cue, pause, resume, completion, release, cancellation, and stop changes reconcile from authoritative runtime state while the window remains open.

Use **All**, **Cuelists**, **Dynamics**, **Timecodes**, or **Macros** to restrict the list. An empty filtered list says which kind is not running instead of appearing broken.

Add Running to a Desktop with **Open Window**. It is a pane-only window and does not appear in the BUILT-INS dock.

**Pane configuration:** **Running kind** stores All, Cuelists, Dynamics, Timecodes, or Macros for this pane. Common size and removal controls also apply.

## Macros

The Macro Pool stores portable, show-owned command sequences. Tap to run; right-click or use
**SET** to open the editor. The editor validates command text, numbers lines, and provides
**Run Macro**, **Run line**, and the available safe **Undo last run** action. Use the
[Command Line Reference](../20-Programmer-and-Cues/01-command-line.md) for the grammar and
[Triggers, Chasers, and Speed Groups](../20-Programmer-and-Cues/13-triggers-chasers-and-speed.md)
for playback use.

## Timecode

The Timecode Pool stores portable timelines. The editor has transport, seek, a complete overview,
markers, and Cuelist, Speed, and Audio lanes; select an item to edit its timing and properties.
Markers label positions but do not execute output. The timeline and its optional audio run from one
authoritative clock, so reopening the editor restores the live position.

A Cuelist clip contains the individual Cues in its selected Cue range. Each Cue shows its start
position, its **In fade** across the top of the lane, and its **Out fade** across the bottom. The
start and end handles of each range snap to Timecode frames. Moving a start handle changes the
corresponding delay boundary; moving an end handle changes when that fade completes. The desk saves
the resulting delay and fade duration on the authoritative Cue in the Cuelist. The clip retains only
its Timecode placement, Cue range, and State Start or Cue Start plus Hold or Release behavior; it
does not keep a second private copy of Cue timing. A linked timing range names its source; dragging
one of its handles is the operator's explicit choice to replace that link with the displayed
independent timing.

A new Cuelist lane opens with one clip already placed. On an empty lane that clip takes the rest
of the timeline from the playhead; where the lane already has clips it copies the length, Cue range,
and behavior of the last clip left of the playhead; and a Cuelist whose Cues all schedule themselves
supplies its own length. A new Speed lane likewise opens with one keyframe at the start of the
timeline. Select a keyframe and enter its **BPM** directly or through the number pad.

A Cue that waits for a manual **GO** has no timing of its own, so the Timecode lane owns its
transition point. Such a Cue shows a movable transition handle inside the clip; drag it, or use the
arrow keys, to place the frame where the clip advances to that Cue. Until a point is placed the Cue
follows its predecessor's completion, and the handle is drawn dashed to show the position is still
the default. The transition is stored on the clip as an offset from the clip start, so moving the
clip carries its transitions with it and the Cuelist keeps its manual trigger for live use.

With a marker selected, **Place Marker** moves it to the current playhead and **Move To** opens a
modal for typing an exact timecode.

Playing Timecode drives that shared Cuelist runtime and shows which clip and Cue are executing.
Pause freezes Cue timing and output. Seeking, moving backwards, resuming, and looping reconstruct the
same current Cue, tracked state, timing progress, and output that uninterrupted playback produces at
the same frame. **Hold** retains the clip's final Cue until it is superseded; **Release** removes the
clip's Cuelist ownership at its end frame. A missing Cuelist or Cue remains visible with a concrete
recovery cause; the editor otherwise offers only the edits that keep a clip valid, so Cue-order and
fade-boundary mistakes cannot be made in the first place. The editor never shows
a clip as executing merely because the Timecode transport is Playing.

Configure the source, frame rate, loss behavior, audio device, and latency trim in Desk Setup. See
[Triggers, Chasers, and Speed Groups](../20-Programmer-and-Cues/13-triggers-chasers-and-speed.md)
for timing and trigger behavior.

## Virtual Playbacks

Virtual Playbacks create a touch-button surface without consuming a physical playback
fader position. Every page owns a stable bank of 300 show-wide Virtual Playback
numbers: page 1 starts at 1001, page 2 at 1301, and page 3 at 1601. Cell position `n`
on page `p` addresses `1000 + 300 × (p - 1) + n`. Every desk that displays the same
number operates the same Playback.

A cell displays its cell number, assigned playback name, and action. When that playback is active it also shows the current Cue and receives active styling. An unassigned cell is inert during normal operation but remains available for Playback Configuration.

**Pane configuration:**

- **Rows** and **Columns** are positive integers whose product may not exceed 300. A
  20×15 surface displays one complete Virtual Playback page.
- **Page mode** is **Follow Main** or **Pinned**. Follow Main uses the control desk's current main page. Pinned stores one fixed page from 1 through 127.
- Resizing the pane does not change its logical row/column count.

Configure a cell exactly like a regular Playback: right-click the Virtual Playback, or press `[SET]` and then press the cell. Both paths open the standard Playback Configuration modal for that one-button, faderless target. Virtual Playbacks additionally support an icon or image background.

Pane Settings keeps grid and page configuration in **Virtual Playbacks** and zone management in **Exclusion Zones**. Playback colors remain part of the assigned Playback rather than a separate pane-level color mode.

Virtual actions carry their stable playback number and validated page qualifier. During
Preload, **Preload virtual playback actions** in Desk Setup decides whether they
execute immediately or are captured for Preload GO. This is independent from the
switches for physical playback controls and programmer changes.

![Virtual Playbacks pane](../../assets/screenshots/panes/virtual-playbacks.png)

![Virtual Playbacks pane settings](../../assets/screenshots/panes/virtual-playbacks-settings.png)
