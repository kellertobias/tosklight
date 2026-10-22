# Cue and Playback Windows

## Cuelist Pool

Tap a Cuelist to open its Cues. Hold or right-click its tile for settings. Tiles show number, name, Cue count, and a ▶ when running; tap a preview image to enlarge it.

Record plus a pool position creates or updates a Cuelist. With Set armed, outlined populated tiles are assignment targets. Otherwise, an ordinary tap opens the list.

The full window searches by number or name. Cuelist settings include Sequence/Chaser mode, priority, HTP/LTP intensity, wrap/restart behavior, and timing. **Chaser X-fade** is a percentage of each step: 0% snaps, 50% fades for half the step, and 100% fades throughout it.

**Pane configuration:** Type colors or Individual colors, plus common layout controls. Cuelist settings change the show object rather than this pane.

![Cuelist Pool](../../assets/screenshots/panes/cuelist-pool.png)

## Cues - Cuelist

The table shows one Cuelist's Cues, previews, triggers, and In/Out delays and fades. Current and next Cues are marked. Selecting a row does not run it.

Touch a value to edit it. **Name** opens text entry; numeric timing opens the number pad; trigger and jump fields offer their relevant choices. **Out Fade > Use Release** follows the desk's Release timing master, and **Out Delay > Use In Fade** follows the Cue's In Fade. Enter a number to make a linked field independent.

Timing cells fill as their real trigger, delay, or fade runs. Incoming and outgoing phases progress independently. Pause freezes the indicators; the completed fill remains until that Cue runs again.

Copy, Move, and Delete outline eligible Cue rows. Touch a row to address that complete Cuelist/Cue; navigation and scrolling remain available. Set outlines editable values rather than executing the Cue.

### Previews

Stage previews are stored pictures of the Cue, not live renderers. They travel with the show and refresh when the relevant programming or rig changes.

A media-only Cue can use a ToskLight Media preview. One layer shows that layer over a transparency checkerboard; a master or multi-layer Cue shows the composed Program. Lighting/media mixtures keep a Stage preview. Media previews require the corresponding server and library.

| Preview message | Action |
| --- | --- |
| **Loading media preview** | Wait for the server to load it. |
| **Empty media** | Check selected media, layer level, and master. |
| **Media output missing** | Update or repatch the output. |
| **Media Server offline · Retry** | Check connectivity and tap to retry. |

### Settings and following

The full window returns to the Pool and opens **Cuelist Settings**. Choose mode, priorities, wrap/restart, and timing, then **Save**. Closing unsaved settings offers Save, Discard, or Stay.

In Pane Settings, **Displayed Cuelist** can be **Fixed** or **Follow selection**. Fixed keeps one chosen list. Follow selection shows the explicitly selected Playback's Cuelist; a non-Cuelist Playback leaves it empty. The row follows its actual current Cue, including Chaser steps.

**Compact Cue rows** reduces row height and hides preview pictures. Common position, size, and removal controls also apply.

![Cue table](../../assets/screenshots/panes/cues.png)

## Cuelists (tabs)

**Cuelists** integrates the Pool, Cue table, and settings. It currently replaces the Pool with the chosen list rather than showing several tabs. In a compact pane the full-window Back control is hidden; reopen or replace the pane to return to the Pool.

Use **Cues** for a permanent fixed/following Cue overview.

![Integrated Cuelists pane](../../assets/screenshots/panes/cuelists.png)

## Running

Running lists Cuelists, standalone Dynamics, Timecodes, and Macros. Several controls operating the same object produce one row. A Dynamic inside a Cuelist remains part of that Cuelist rather than a second stoppable row.

Each **Off** stops exactly its named object. Cuelist rows show the current Cue; other objects show **Cue —**. Filter with **All**, **Cuelists**, **Dynamics**, **Timecodes**, or **Macros**.

Running is pane-only: add it through **Open Window**. **Running kind** saves that pane's filter.

## Macros

Tap a Macro to run it; right-click or use **SET** to edit it. The editor provides **Run Macro**, **Run line**, and **Undo last run** where available. See [Command Line Reference](../20-Programmer-and-Cues/01-command-line.md) for syntax.

## Timecode

Use the Timecode Pool and editor to place Cuelist, Speed, and Audio lanes against one timeline. Markers label times but do not execute output. Linked main audio stays pinned below the ruler while other lanes scroll. All lanes share the same horizontal scrolling and zoom.

**Speed Groups** appear first below the main audio, followed by **Cuelists** and **Audio Players**. Tap a section heading to fold its lanes into one compact preview row. **Expand** reveals the lanes; tap a Speed Group name to enlarge it for editing. **Close** on the left restores its compact height.


Scrubbing the ruler or dragging the playhead inspects a position without seeking live output. The **Playhead** encoder shows that editor position; press the title's transport position button to seek explicitly.

Edits autosave while transport runs. Check **Saving…**, **Saved**, or **Not saved**; use **Retry autosave** after a failure. Stop remains available. Duration and offset use `HH:MM:SS.FF` at 44 fps; Escape restores an unfinished field.

Select a clip to edit its Cue range, start/end behavior, or audio folder/file, repeat, and volume. Speed keyframes expose BPM and phase; audio-volume keyframes expose fades. Copy/Delete controls operate on the selected clip, keyframe, or marker. Editor Undo/Redo recovers changes.

### Cue timing on the timeline

All Cues of a Cuelist share one lane as overlapping envelopes, with their names at the bottom. Tap an envelope or its name to expand that lane downward. Only the selected Cue's four **In delay**, **In fade**, **Out delay**, and **Out fade** points appear. Tap another Cue to edit it; **Close cue editor** on the left restores the compact lane. Exact settings are on the right. Opening or closing these settings keeps the timeline's zoom and left-hand position unchanged; use the overview or explicit Fit action to change the visible range.

Drag the timing points to change the Cuelist's actual delays and fades, snapped to frames. Out delay begins at the handover to the next Cue. When timing points meet, short connectors separate their handles so each remains draggable. Dragging a linked Out timing explicitly replaces its link with independent timing. An In fade of zero inherits the desk's **Cue Fade**; an explicit Out fade of zero is immediate.

The labelled **Trigger** control and exact time show when a Cue advances. The first Cue starts with the clip. A manual-GO Cue's Trigger can be placed inside the clip without changing its manual trigger for ordinary playback. Until placed, it follows its predecessor's completion. Moving the clip carries the placement with it. Follow, Wait, Link, and Timecode triggers remain derived from the Cuelist; selecting a Cue does not convert them to manual placements. Trigger movement stops at legal neighbouring Cue boundaries.

The final Cue has no later handover inside the clip. Its shared Cue Out timing does not run after clip end. Use the separate **Clip Out** gain fade to fade the clip before its end, including a single-Cue clip. **Hold** retains the final state and **Release** removes the clip's ownership at its end.

New Cuelist lanes start with a clip; Speed lanes start with a keyframe. Select a marker and use **Place Marker** for the playhead position or **Move To** for an exact time.

### Running

Playing drives the same Cuelist runtime as other controls. Pause holds timing and output; seek and loop restore the state appropriate to their destination. **Hold** keeps a clip's final Cue until superseded; **Release** relinquishes it at the end. Missing Cuelists or Cues remain marked for repair.

Configure input source, frame rate, loss behavior, audio device, and latency in Desk Setup. See [Triggers, Chasers, and Speed Groups](../20-Programmer-and-Cues/13-triggers-chasers-and-speed.md).

## Virtual Playbacks

A Playbacks pane operates the same show assignments as screens and hardware. Pages 1–99 have positions 1–1000. An explicit `page.position` always addresses that assignment, regardless of the displayed page.

Configure rows, counts, buttons/faders/encoders, and an **Own**, **Pinned**, or **Follow** page source. **Take from Screen** copies a saved screen arrangement into a draft: review it and press **Apply**. This copies layout, not assignments, and does not follow later edits.

Right-click a control or use **SET** for Playback Configuration. Actions, colors, icons, and images belong to that Playback. Pane Settings controls layout and **Solo Regions**, including their colors and addresses across pages.

For full-window tile sizing and the current Cue insert, see [Virtual Playbacks](../20-Programmer-and-Cues/14-virtual-playbacks.md). Preload eligibility is configured separately; Flash and fader movement remain Live. See [Preload](../20-Programmer-and-Cues/12-preload.md).

![Eight-Playback pane](../../assets/screenshots/panes/virtual-playbacks.png)

![Playback pane settings](../../assets/screenshots/panes/virtual-playbacks-settings.png)
