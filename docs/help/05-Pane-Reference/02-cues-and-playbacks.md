# Cue and Playback Panes

## Cuelist Pool

The Cuelist Pool is numbered storage for sequences and Chasers. A populated tile shows its Cuelist number and name, whether it is running, its master percentage, and any playback-page assignments. Tap a populated tile to open its Cues. Record plus a pool position creates or updates a Cuelist; Set workflows use the selected Cuelist as an assignment target.

The full window can search the 1,000-position pool by number or name using the shared [window search bar](../01-application-layout.md#search-bars). Holding a populated tile opens Cuelist configuration: Sequence or Chaser mode, priority, HTP/LTP intensity mode, wrap behavior, restart behavior, timing overrides, Cue renumbering, and Chaser speed, multiplier, and crossfade. **Chaser X-fade** is stored from `0%` to `100%` of the effective step: `0%` snaps, `50%` fades for half the interval, and `100%` fades for the complete interval. Changing the Speed Group BPM or multiplier changes the live fade duration without changing the stored percentage. These settings change the Cuelist itself and must not be confused with Pane Settings.

**Pane configuration:** only common size and removal controls. Search and Cuelist configuration belong to the full content window.

![Cuelist Pool pane](../assets/screenshots/panes/cuelist-pool.png)

## Cues - Cuelist

This pane shows the Cues of one Cuelist. Rows show the optional Stage preview, Cue number, Cue name, trigger type, and fade time. Running and next Cues receive status highlighting. Selecting a row changes the current row selection but does not execute it.

The compact editor at the right stays inline with the selected Cue. Its Stage preview uses the full sidebar width and carries a readable selected-Cue label over the image. The Cues pane settings can hide this sidebar when the Cue table needs the full pane width; the preference is stored with that pane. Under the preview, closely spaced frameless rows edit Title, Fade, Delay, GO/FOLLOW/TIME trigger, and trigger time; text and numeric rows provide the desk keyboard or number pad at the right. If all rows cannot fit below the preview, the sidebar does not scroll: it keeps the preview and replaces the form with the current attribute values and **Press SET, then press an attribute value to edit it**. Physical or software SET followed by Title, Fade, Delay, Trigger, or Trigger time opens the corresponding keyboard, number pad, or choice modal. Selecting another row updates this same editor without executing either Cue, and the Cue table remains visible. Cue deletion is deliberately not a button in this editor; use the explicit Delete Cue command with a complete Cue address.

The full window also provides navigation back to the Cuelist Pool and **Cuelist Settings**. Cuelist Settings opens as a modal over the view. Its title bar contains Save, Renumber Cues, and a two-line Mode menu showing the current Sequence or Chaser mode. The body groups numeric/intensity priority, wrap/restart behavior, and timing into three explained columns. Chasers additionally expose a typed Speed multiplier and a `0–100%` Chaser X-fade fader. Close leaves clean settings immediately; if settings are dirty, choose **Save changes**, **Discard changes**, or **Stay** explicitly. The pool's hold shortcut opens the same modal.

The compact pane starts fixed to Cuelist 1 or the first available list. In Pane Settings, **Displayed Cuelist** can keep the pane **Fixed** to any available Cuelist or **Follow selection**. Follow selection resolves the desk's explicitly selected playback and shows its Cuelist; selecting a Group playback or having no selected playback leaves the pane empty instead of switching to an unrelated list. While that Cuelist runs, the selected row and inline Cue editor follow its actual current Cue, including automatic Chaser steps. The fixed Cuelist choice and display mode are stored with that pane, so different Cues panes can remain on different lists while another follows the desk selection and active Cue.

**Pane configuration:** **Displayed Cuelist** selects Fixed or Follow selection, **Cuelist** chooses the fixed list, and **Show Cue sidebar** controls the inline selected-Cue editor. The common size and removal controls also apply.

![Cues pane](../assets/screenshots/panes/cues.png)

## Cuelists (tabs)

This pane currently opens the Cuelist Pool and then replaces it with the selected list's Cue table. Despite its legacy label, the current implementation does not display multiple tabs. In compact mode the full-window Back control is hidden, so returning to the Pool requires reopening or replacing the pane. Treat this as a current interface limitation rather than as a multi-tab workspace.

Use **Cuelist Pool** for a permanent pool surface and **Cues - Cuelist** for a permanent Cue overview. Use the full Cuelists built-in when the operator must move freely between pool, Cue editing, and Cuelist configuration.

**Pane configuration:** only common size and removal controls.

![Cuelists pane](../assets/screenshots/panes/cuelists.png)

## Virtual Playbacks

Virtual Playbacks create a touch-button surface without consuming a physical playback fader position. Every cell uses the playback assignment at the same position on the control desk's current page; that playback-pool definition in turn targets a Cuelist or another supported playback function. Changing page keeps the cell positions and displays the new page's assignments.

A cell displays its cell number, assigned playback name, and action. When that playback is active it also shows the current Cue and receives active styling. An unassigned cell is inert during normal operation but remains available as a Set/assignment target.

**Pane configuration:**

- **Rows** and **Columns** independently accept 1-12, allowing 1 to 144 cells.
- Resizing the pane does not change its logical row/column count.

Assignment uses the same Set workflow as any other playback. Use **Set Source**, **Add Target**, or press `[SET]`, select the source, and select the Virtual Playback. Pressing `[SET]` and then a Virtual Playback opens the standard Playback Configuration modal for that one-button, faderless target. Virtual Playbacks additionally support an icon or image background.

Virtual actions identify themselves as coming from the virtual surface. During Preload, **Preload virtual playback actions** in Desk Setup decides whether they execute immediately or are captured for Preload GO. This is independent from the switches for physical playback controls and programmer changes.

![Virtual Playbacks pane](../assets/screenshots/panes/virtual-playbacks.png)

![Virtual Playbacks pane settings](../assets/screenshots/panes/virtual-playbacks-settings.png)
