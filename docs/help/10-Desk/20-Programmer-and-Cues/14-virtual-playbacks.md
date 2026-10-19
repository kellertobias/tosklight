# Playbacks

A Playbacks pane is a configurable control surface for the same show-owned Playbacks used by screens and attached hardware. Each page has positions 1–1000, on pages 1–99. Address a Playback as `page.position`; there is no separate virtual numbering bank. Two surfaces displaying the same address operate the same assignment and authoritative runtime.

By default, each Playback is a square tile that acts as one large button. Its title is at the top left and its Playback number at the top right, with its background image and current Cue visible inside the tile. In pane Settings, enable **Show playback controls** to display the configured buttons, faders or push encoders instead. This setting applies to both **Screen** and **Free region**, is saved for this pane, and does not change Playback assignments or another surface's controls.

In pane Settings, choose **Screen** to display a selected screen’s live configured rows, or **Free region** to keep independent rows. Screen mode fits every configured row and column without scrolling. Its page may follow the displayed screen, follow another screen, or use a **Custom page**. Select a custom page through the page number and caret in the pane title. Free region retains touch-sized controls and can scroll when its independent grid is larger than the pane.

Configure each free-region row's first position, number of positions and presentation: buttons, faders or push encoders. Preserve gaps between rows when useful. For example, a 20-column, 14-row pane starting at 401 covers positions 401–680 on its displayed page. Pane controls use the same configured Playback actions, fader modes, icons and images as screen controls.

Choose **Pinned** for a fixed page, **Own** for this surface's page selection, or **Follow** another named surface. Following chains cannot contain cycles. Different panes may operate different pages concurrently. The shared command line uses the visibly selected surface's effective page for an undotted Playback number; use an explicit `page.position` to address another page. In Playback mode, the command field names the selected surface and its effective page, including when that Screen is closed. In Programmer mode, when the encoders are on another screen, it shows **Programmer is assigned to** that screen.

Pressing a held control captures its original Playback address and control source. Changing page while Flash, Swap or another held control is pressed keeps that contribution until release. Release addresses the original Playback even after its cell is no longer displayed. Simultaneous holds remain independent. Page changes themselves never operate Playbacks.

**Take from Screen** copies a selected screen's saved position arrangement into a draft. Review the draft before Apply. The screen may be closed or its hardware disconnected. The saved copy does not silently follow later screen edits and does not copy Playback assignments.

Right-click a cell, or press **SET** and then its control, to open Playback Configuration. A Playback with an actual runtime failure has a red outline. Ordinary command mistakes use the brief red command-line pulse and a short toast above the bar; entered text remains grey after the pulse.

Macro and Timecode assignments use button actions. Macro **GO** runs the assigned Macro once. Timecode **GO** starts or resumes transport, **Pause** pauses it, and **Off** stops it. These assignments have no fader value.

When a Playback runs a Cuelist containing exactly one Cue and no configured icon or image, it shows that Cue's preview. A media-only Cue uses the Media Server's Program or layer picture described in [Cues - Cuelist](../30-Windows/03-cues-and-playbacks.md#cues---cuelist). Layer images retain transparency over a checkerboard. Loading, missing and offline media states are named explicitly. A configured icon or image takes priority; removing it restores the automatic preview. Editing the Cue updates the image, while adding or removing Cues returns the cell to a plain label when it no longer has exactly one Cue.

## Solo Regions

A Solo Region is a show-owned named set of explicit Playback addresses. Permanent activation of a member releases permanent peers using Solo handover fades. Regions may span pages and overlap; shared targets and overlapping peers are deduplicated. Turning the winner Off starts no replacement. Membership belongs to the exact Playback address you start. A Timecode starts Cues directly; those Cue starts do not trigger Solo. When a Timecode is assigned to a Solo member Playback, starting another member permanently stops that Timecode.

**Flash bypasses Solo.** Flashing a member preserves permanent peers. Starting a permanent peer preserves existing Flash holds. Releasing Flash removes its temporary contribution without restarting a previously released permanent Playback or stopping the current winner.

Choose **Create Solo Region** in the pane title, select grid cells, enter a name, then **Save** or **Cancel**. In pane Settings, **Manage Solo Regions** lists saved regions: select one to rename it, **Delete**, or **Edit**. Edit closes Settings and returns to the grid to add or remove members before Save or Cancel. Long-press the lower section’s **PROG/PLAYBK** button, or an optional screen’s **Playback/Encoders** switch, for its page-source settings and **Manage Solo Regions** tab. Choose **Create New Solo Region** there; the command line becomes the draft’s name, Save and Cancel controls. An optional screen shows that editing line only while its Solo draft is open. Selection and region editing are configuration gestures and do not press or operate Playbacks. Adjacent selected members share an amber outer fence; disconnected members form separate islands. Change pages during a draft to include addresses across pages. Membership belongs to the show and survives page changes, layout changes, closing or removing a pane, and hardware disconnection. Only explicitly changing or deleting a region changes its membership.

## Preload

Preload eligibility is configured by Playback position ranges, independent of the surface used. For example, `1 THRU 15 + 101 THRU 115` captures eligible permanent actions in those positions. Range union applies to every page by default, with optional page restrictions. Overlaps do not duplicate an action and unmatched positions remain Live.

The existing permanent-action set and Temp toggle behavior remain unchanged. Flash and fader movement stay Live. Programmer capture is a separate option. Captured actions retain their exact address, order and source through page changes and are committed together by Preload GO. See [Preload](12-preload.md) for inspection, removal, timing and release behavior.
