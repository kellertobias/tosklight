# Cue timing on the timeline

Tap a Cue envelope or its name in the Timecode editor to expand its Cuelist lane. Cue envelopes overlap in that lane, with names along the bottom. Only the selected Cue shows its four **In delay**, **In fade**, **Out delay**, and **Out fade** points.

The expanded envelope area is twice the height of the Cue-name strip. Selecting a Cue brings the entire lane into view. Other Cuelists and Speed Groups dim while it is open. Tap another Cue to switch, or **Close cue editor** on the left to return to the compact lane.

## Place the Cue Trigger

A rounded rectangular grip with one vertical bar on each side of a boundary marks an editable interior Cue. Drag it to move that Cue's Trigger. The trigger stays between its legal neighboring Cue boundaries.

The first Cue begins with the clip. Its grip is cut through the middle at the clip's left edge, leaving the right bar and half-rectangle visible. Drag that half-grip, or change the first Trigger, to move the whole clip. Its duration and later Cues' relative positions stay unchanged.

At the right edge, the corresponding half-grip shows the left bar and half-rectangle. Drag it to change the clip end. In expanded view, **Extend last Cue** in the bottom Cue-name row also changes the end without moving earlier Cue triggers.

A manual-GO Cue's transition can be placed within the clip without changing its manual trigger for ordinary playback. Before placement, it follows its predecessor's completion. Moving the clip carries these relative placements with it. Follow, Wait, Link, and Timecode triggers remain derived from the Cuelist; selecting one does not turn it into a manual placement.

## Edit delay and fade

Drag the selected Cue's four timing points. They snap to frames and edit the same Cuelist-owned times shown in Cuelist View. The points stay on their envelope corners. Where points coincide, labeled touch regions and short connectors keep their handles independently reachable.

**Trigger** is when the Cue starts. **In delay** runs from Trigger to In start; **In fade** runs from In start to In end. **Out delay** starts when the next Cue takes over. Dragging a linked Out timing explicitly replaces the link with independent timing.

An In fade of zero inherits the desk's **Cue Fade**. An explicit Out fade of zero is immediate. [Cue timing](17-cue-timing.md) explains the shared values, timing masters, and corresponding programmer commands.

The final Cue has no later handover inside the clip. Its Cue Out timing does not run after the clip end. Set **Clip Out**, the clip's separate gain fade, to fade before the end, including for a single-Cue clip. **Hold** keeps the final state; **Release** removes the clip's ownership at its end.

## Enter exact values

The selection sidebar starts hidden. Enable **Show selection sidebar** in **Timecode Settings** for its expandable sections:

- **Cue In** shows Trigger and its source, Clip start frame, and In start/end.
- **Cue Out** shows Out start/end and Clip end frame.
- **Cuelist** shows Start/End Cue and start/end behavior.

Closing the sidebar hides only that panel. The Cue stays selected and expanded. Opening or closing it keeps timeline zoom and horizontal position. Use the [overview or encoder zoom](36-timecode-navigation.md) to change the visible range.

Each clip labels **State Start** or **Cue Start** at its left edge, and **Hold** or **Release** at its right edge. These labels identify its configured start/end behavior. Invalid references retain their **Unable** reason.
