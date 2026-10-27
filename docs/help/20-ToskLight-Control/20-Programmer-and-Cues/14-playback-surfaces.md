# Playback surfaces

Every screen, pane, and attached control operates the same show-owned Playback assignments. Pages **1-99** each have positions **1-1000**. Use `page.position` to address one explicitly; an undotted number uses the visibly selected surface's effective page.

## Display tiles or controls

Add a **Playbacks** pane. A Playback is a single large action tile by default, showing its title, number, image, action, and current Cue. The Cue sits in a dark insert at the bottom; shallow rows put Cue and action on one line.

Enable **Show playback controls** in pane Settings to show the configured buttons, faders, or push encoders instead. This changes only that pane's presentation, including in Screen and Free region modes.

## Choose the displayed arrangement

Choose **Screen** to display a selected screen's configured rows. Choose **Free region** to keep independent rows and positions. Set each free row's first position, count, and presentation. A four-column, two-row arrangement starting at 1 covers positions 1-8 on its page; gaps between rows can remain.

Configured rows and columns fit inside the pane without scrolling. Resizing changes cell proportions, while the row and column counts stay fixed. Pane controls use the same Playback actions, fader modes, icons, and images as screen controls.

**Take from Screen** copies a saved screen arrangement into a draft. Review it, then **Apply**. It works with a closed screen or disconnected hardware. The copy includes layout, keeps Playback assignments unchanged, and does not follow later screen edits.

## Choose the page

**Own** lets this surface change pages independently. **Pinned** holds one page. **Follow** tracks another named surface; following chains cannot loop. In pane Settings, a page can follow the displayed screen, follow another screen, or use a **Custom page**. Use the page number and caret in the pane title to select a custom page.

Different panes can operate different pages at once. Playback mode names the selected surface and effective page in the command field, even when its screen is closed. Programmer mode shows **Programmer is assigned to** the other screen when the encoders are placed there.

Page changes never operate Playbacks. A held Flash, Swap, or other control keeps its original address and source through a page change. Release still reaches that original Playback when its cell is no longer visible. Simultaneous holds remain independent.

## Configure and operate a Playback

Right-click a cell, or press **SET** then its control, to open [Playback configuration](25-playback-configuration.md). An actual runtime failure gives the Playback a red outline. Ordinary command mistakes use a brief command-line pulse and toast, retaining the entered text in grey.

A Macro assignment uses **GO** to run once. A Timecode assignment uses **GO** to start or resume, **Pause** to pause, and **Off** to stop. These assignments have no fader value.

A one-Cue Cuelist uses its Cue preview when no icon or image is configured. Media-only Cues use Pixel's Program or layer image, with transparent layers over a checkerboard. Loading, missing, and offline states are named. Chosen artwork takes precedence; removing it restores the automatic preview. Editing the Cue refreshes the picture; a list without exactly one Cue returns to a plain label.

Use [Solo Regions](34-solo-regions.md) for exclusive permanent playback choices. [Preload](12-preload.md) captures eligible actions by position range, independent of the surface. Flash and fader movement remain live.

![Playback pane](../../assets/screenshots/panes/virtual-playbacks.png)

![Playback pane settings](../../assets/screenshots/panes/virtual-playbacks-settings.png)
