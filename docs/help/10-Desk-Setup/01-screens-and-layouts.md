# Screens and Desktop Layouts

Open **Show > Desk Setup > Screens & playback** to configure the primary desk surface and, in the desktop application, optional operator screens.

## Configure a screen

For the default screen, set its operator-facing name and OSC alias. Choose **Configure Playbacks** to set the playbacks per row and add playback rows. Each row has its own first playback number, fader availability, and number of buttons. The default screen owns the Main Page. **Enable software keyboard shortcuts** controls the complete software shortcut layer for this screen; attached hardware disables that layer automatically. These changes take effect and save immediately. Choose **Undo** in the Screens & playback title bar to reverse the most recent actual default-screen change. Opening a modal or moving between setup sections does not create an undo step.

Choose **Desk Lock** in the Screens & playback title bar to open its configuration modal. Set the lock message, unlock control, and optional wallpaper, then choose **Save Lock Configuration** in the modal title bar. The Show menu's **Lock Desk** action applies that saved configuration.

The Tauri desktop application can add optional screens. Each optional screen can show or hide the Dock, Playbacks, and Page Controls; select a physical display; and enter fullscreen. Its **Configure Playbacks** dialog provides the same row controls as the default screen and also selects its page mode. Choose **Follow Main** when its page tracks the primary page. Choose **Dedicated Page** for an independent operator surface. Browser-only operation displays the default-screen controls but cannot create native desktop windows.

Playback rows share all available playback height according to their controls. With attached playback hardware, a row without faders uses one height unit and a row with faders uses two. On a touch surface, a one-button row uses one unit, a multi-button row uses two, and a fader row uses four. The unit size adapts so the configured rows fill the playback area.

![Default screen and playback configuration](../assets/screenshots/workflows/desk-setup-screens.png)

## Build task Desktops

Create separate Desktops for common jobs such as Programming, Playback, Patch, and diagnostics. Add only the panes needed for that job. Use full built-ins for temporary work that should not change a Desktop. Layout changes are autosaved to desk data.

For pane geometry and available windows, see [Application Layout and Window Manager](../01-application-layout.md).
