# Screens and Desktop Layouts

Open **Show > Desk Setup > Screens & playback** to configure the primary desk surface and, in the desktop application, optional operator screens.

## Configure a screen

For the default screen, set its operator-facing name and OSC alias. Choose **Configure Playbacks** to set the playbacks per row and add playback rows. Each row has its own first playback number, fader availability, and number of buttons. The default screen owns the Main Page. **Enable software keyboard shortcuts** controls the complete software shortcut layer for this screen; attached hardware disables that layer automatically. These changes take effect and save immediately. Choose **Undo** in the Screens & playback title bar to reverse the most recent actual default-screen change. Opening a modal or moving between setup sections does not create an undo step.

Choose **Choose default screen** to see every known client. Connected clients appear first, followed by disconnected clients; each group is ordered by the most recent connection. Every entry shows its stable client identity, connection state, last-connected time, and associated screen configuration. Older migrated entries show **Last connected unknown** until that client connects again. The current client and the screen this app currently uses are identified separately.

A disconnected historical client can be removed with **Remove client** and a named confirmation. The current client and any client or screen configuration with an active session cannot be removed. Removal clears that client's registration, default-screen configuration, per-show page and playback selection, desk lock, and Update defaults. It does not change portable shows, users, optional screens, other clients, installation-wide configuration, Virtual Playback assignments, or show-owned exclusion zones. If the same removed client identity reconnects later, ToskLight registers it with a new default screen configuration instead of restoring the deleted settings.

Choose **Desk Lock** in the Screens & playback title bar to open its configuration modal. Set the lock message, unlock control, and optional wallpaper, then choose **Save Lock Configuration** in the modal title bar. The Show menu's **Lock Desk** action applies that saved configuration.

The Tauri desktop application can add optional screens. Each optional screen can use its normal
configurable **Desktop** or a **Fixed full-screen pane**. A fixed pane fills the screen's pane
workspace without pane headers, resizing, settings, selection, or editing controls. Configure it
from the controlling screen; the external display remains view-only.

The fixed-pane choices are **Fixture Sheet**, **Stage - 2D**, **Stage - 3D**,
**Cues - Cuelist**, and **Text**. Their display settings and any fixed Cuelist or text file are
stored with that optional screen. If configured content is missing, the screen shows an unavailable
state and does not substitute another Cuelist or file.

**Show Dock** is incompatible with **Fixed full-screen pane**. Selecting the mode turns the Dock
off and keeps its control disabled. Returning to **Desktop** makes the Dock control available
again, but does not silently turn it back on. **Playbacks** and **Page Controls** remain independent
and reserve their normal screen space in either content mode.

Each optional screen can also select a physical display and enter native fullscreen. Native
fullscreen controls the application window on the physical display; **Fixed full-screen pane**
controls the content inside that window. Its **Configure Playbacks** dialog provides the same row
controls as the default screen and also selects its page mode. Choose **Follow Main** when its page
tracks the primary page. Choose **Dedicated Page** for an independent operator surface.
Browser-only operation displays the default-screen controls but cannot create or claim support for
native optional-screen windows.

Playback rows share all available playback height according to their controls. With attached playback hardware, a row without faders uses one height unit and a row with faders uses two. On a touch surface, a one-button row uses one unit and makes the whole playback section its button, with the function label at bottom-right. A two- or three-button faderless row uses two units and places its buttons side by side. A fader row uses four units. The unit size adapts so the configured rows fill the playback area.

![Default screen and playback configuration](../assets/screenshots/workflows/desk-setup-screens.png)

## Build task Desktops

Create separate Desktops for common jobs such as Programming, Playback, Patch, and diagnostics. Add only the panes needed for that job. Use full built-ins for temporary work that should not change a Desktop. Layout changes are autosaved to desk data.

For pane geometry and available windows, see [Application Layout and Window Manager](../01-application-layout.md).
