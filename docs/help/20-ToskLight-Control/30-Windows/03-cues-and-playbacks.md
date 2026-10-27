# Cuelist and Cue windows

## Cuelist Pool

The Cuelist Pool is numbered storage for sequences and Chasers. A populated tile shows its Cuelist number and name, whether it is running, its master percentage, and any playback-page assignments. Tap a populated tile to open its Cues. Right-click it, or hold it on a touch surface, to open that exact Cuelist's settings. When a tile has a preview image, tap the image itself to inspect a larger modal view. Record plus a pool position creates or updates a Cuelist; Set workflows use an ordinary tile tap as the explicit assignment target. While Set is waiting for that source, every populated Cuelist tile receives the literal Set outline and the complete tile is the target; empty tiles remain ordinary and unoutlined.

The full window can search the 1,000-position pool by number or name using the shared [window search bar](01-desk-interface-and-windows.md#search-bars). Holding a populated tile opens Cuelist configuration: Sequence or Chaser mode, priority, HTP/LTP intensity mode, wrap behavior, restart behavior, timing overrides, Cue renumbering, and Chaser speed, multiplier, and crossfade. **Chaser X-fade** is stored from `0%` to `100%` of the effective step: `0%` snaps, `50%` fades for half the interval, and `100%` fades for the complete interval. Changing the Speed Group BPM or multiplier changes the live fade duration without changing the stored percentage. These settings change the Cuelist itself and must not be confused with Pane Settings.

**Pane configuration:** **Type colors / Individual colors** chooses the configured lime Cuelist treatment or explicit item colors for this pane. Search and Cuelist configuration belong to the full content window. Common size and removal controls also apply.

![Cuelist Pool pane](../../assets/screenshots/panes/cuelist-pool.png)

## Cues - Cuelist

This pane shows the Cues of one Cuelist. By default every row shows these columns, in order: **Preview**, **No.**, **Name**, **Trigger**, **Trigger Time**, **In Delay**, **In Fade**, **Out Delay**, and **Out Fade**. All nine columns remain available without changing the pane settings. Running and next Cues receive status highlighting. Selecting a row changes the current row selection but does not execute it.

See [Cue previews](11-cue-previews.md) for lighting and Pixel thumbnails and their status messages.

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

Other playback windows have their own chapters: [Running](12-running.md), [Timecode](../20-Programmer-and-Cues/18-timecode.md), [Macros](../20-Programmer-and-Cues/19-macros.md), and [Playback surfaces](../20-Programmer-and-Cues/14-playback-surfaces.md).
