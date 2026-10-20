# Pane titles and tile layouts

## PANE-001 — Operator-created pool presentation

Create Group pool, Preset pool, Cuelists, Dynamics, Macro Pool, and Timecode panes through Open Window. Each new pane starts with Automatic squares. Resize it: every tile remains square, and the column count adapts.

Open Settings → Pane Settings. Edit Pane title. The header immediately uses that title. Select Fixed grid, set three Tile rows and four Tile columns, and close Settings. Exactly three rows and four columns fill the visible pool area; rectangular tiles are expected. Later addresses remain reachable by scrolling. Change back to Automatic squares and verify square tiles again.

Save and gracefully restart the same show/server. Custom titles and selected grid dimensions persist. Repeat in a native secondary screen. Screen content, command ownership, fixture selection, and programmer values must not change as a side effect.

Free-region Playback tiles offer the same two modes without rewriting page assignments, changing sparse address order, or dropping Solo outlines. Screen-mirrored Playbacks and explicit fader controls retain their authored layout and explain why the tile layout control is disabled.

Open an older layout without these new fields. Its existing Group fixed-column configuration and authored Playback grid remain unchanged. Legacy/default-title migrations must not overwrite newly authored pane titles, including titles that match old default labels. Invalid optional grid fields must recover without discarding the pane or preventing startup.

Marketing screenshots must use titles and tile layouts that an operator can reproduce with these controls.
