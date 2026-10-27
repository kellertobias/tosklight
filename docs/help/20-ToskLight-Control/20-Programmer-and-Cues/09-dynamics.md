# Dynamics

The Dynamics pool holds numbered animated values. Tap a tile to toggle it for the current ordered
selection, or Shift-click to edit it; use `[^SET]`, choose the Dynamic, then touch a Playback to assign it. The editor
shows **Lanes**, **Phase**, and **Speed** for attribute curves, target spreading, and BPM/Speed
Group control. **Take Selection** stores a target (a single Group stays live; other selections are
frozen); a stored target takes precedence over any active selection. **Clear Selection** removes
it. A Dynamic without a stored target never runs on every fixture by itself: with nothing
selected, the first tap selects every fixture that can run it (a fixture with a channel one of
its lanes drives) and starts nothing; the second tap starts it on that selection. Use a Stage pane for Live or Follow Preload visualization.

**Pane configuration:** only common size and removal controls.

![Dynamics editor](../../assets/screenshots/panes/dynamics.png)

## Speed and run mode

In the Dynamics Speed view, the large tempo button taps the chosen Speed Group. While the Dynamic follows a Speed Group, the circle in the button's upper-right corner lights at the start of every beat of that group as the desk runs it, so it follows taps, tempo changes, and changes made on another surface or over OSC. It stays amber while the group is paused, and turns to a dashed outline when the desk is not answering. The Programmer's Dynamics Speed page shows no lane chooser, because Speed applies to every lane of the Dynamic alike.

In the Dynamics Speed view, choose **Loop** to repeat until Off or **One-shot** to run one complete effective cycle and stop automatically. Run Mode is separate from Start now, Join sync now, and Next boundary, which determine when and where the cycle begins. A completed one-shot does not restart merely because its Cue, Programmer, or Playback value remains active; trigger it again with a new deliberate activation.

## Projection and phase

[Projection and phase](29-dynamic-projection-and-phase.md) order targets spatially and distribute the animation. Live Group targets can inherit their Group mapping.

## Dynamic lanes

[Dynamic lanes](28-dynamic-lanes.md) animate individual attributes. Each lane has its own units, curve, and range. The tile symbol shows which families the Dynamic contains.

## Apply from the command line

Select fixtures, then use `[^5]` to open Dynamics or address a Dynamic preset. A Dynamic with a stored Group uses that target; a targetless Dynamic uses the current selection. Use `[^5] 29 [AT][^TIME] 2 [ENT]` to assign Dynamic 29 to Speed Group B.

[FixAT](01-command-line.md#dynamics-and-fixat) holds a static Programmer value above the Dynamic while its phase keeps running. Clearing FixAT reveals the Dynamic at that current phase.
