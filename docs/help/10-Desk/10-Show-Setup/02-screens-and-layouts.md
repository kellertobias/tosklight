# Screens and Desktop Layouts

Open **Show > Desk Setup > Screens & playback** to configure the primary desk surface and, in the desktop application, optional operator screens.

## Configure a screen

For the default screen, set its operator-facing name. Choose **Configure Playbacks** to set the playbacks per row and add playback rows. Each row has its own first playback number, fader availability, and number of buttons. The default screen owns the Main Page. **Enable software keyboard shortcuts** controls the complete software shortcut layer for this screen; attached hardware disables that layer automatically. These changes take effect and save immediately. Choose **Undo** in the Screens & playback title bar to reverse the most recent actual default-screen change. Opening a modal or moving between setup sections does not create an undo step.

Choose **Known windows** to see every window that has connected. Every window operates the one desk, so the list identifies windows rather than desk configurations to pick between. Connected windows appear first, followed by disconnected ones; each group is ordered by the most recent connection. Every entry shows its stable client identity, connection state, and last-connected time. Older migrated entries show **Last connected unknown** until that window connects again. The window you are looking at is marked **This window**.

A disconnected window can be dropped with **Forget window** and a named confirmation, or every disconnected one at once with **Forget other windows**. This window and any connected window cannot be forgotten. Forgetting removes only the record that the window has connected before. The desk is untouched: its page, playback selection, desk lock, and Update defaults belong to the desk and outlive any single window, as do portable shows, users, optional screens, installation-wide configuration, Virtual Playback assignments, and show-owned exclusion zones. A forgotten window that connects again is simply known again, operating the same desk.

Choose **Configure desk lock** in the Screens & playback title bar to open its configuration modal. Set the lock message, unlock control, and optional wallpaper, then choose **Save Lock Configuration** in the modal title bar. The Show menu's **Lock Desk** action applies that saved configuration.

The Tauri desktop application can add optional screens. Each optional screen can use its normal
configurable **Desktop** or a **Fixed full-screen pane**. A fixed pane fills the screen's pane
workspace without pane headers, resizing, settings, selection, or editing controls. Configure it
from the controlling screen; the external display remains view-only.

The fixed-pane choices are **Fixture Sheet**, **Stage - 2D**, **Stage - 3D**,
**Cues - Cuelist**, and **Text**. Their display settings and any fixed Cuelist or text file are
stored with that optional screen. If configured content is missing, the screen shows an unavailable
state and does not substitute another Cuelist or file.

A screen's **Content** is **Desktop**, **Controls only**, **Fixed full-screen pane**, **Fixed left
pane**, or **Fixed right pane**. Only **Desktop** and **Fixed full-screen pane** put pane content
above the control region.

**Controls only** gives the whole screen height to the control region: the encoders when this screen
carries them, the Playback section otherwise. Nothing is reserved above it.

**Fixed left pane** and **Fixed right pane** divide the entire screen, not just its upper part. The
chosen widget keeps its configured **Pane width (%)** — a share of the window width, so it holds its
proportion on every display — over the full height on that side, and the
control region takes every remaining pixel on the other side. There is no empty region above either
one. When the screen carries neither the encoders nor Playbacks, the remaining side stays empty.

**Show Dock** needs **Desktop** content. Every other content mode turns the Dock off and keeps its
control disabled. Returning to **Desktop** makes the Dock control available again, but does not
silently turn it back on. **Playbacks** and **Page Controls** remain independent and reserve their
normal screen space wherever a control region exists.

The encoder section of an optional screen never carries the keypad, the programmer fader or the
Delete and Move tools; those stay on the main screen. It keeps the encoder group tabs with their
Align, Special Dialog and Dynamics controls and the encoders below them. **Command line** adds the
programmer command line above them on that screen.

While an optional screen carries both the encoders and Playbacks, one button toggles between the two
sections. It sits at the top right of the command line where that screen shows it, and directly right
of **Dynamics** on the encoder tab row where it does not. A screen without the encoders has nothing
to toggle and shows no such button.

On a **Desktop** screen the control region is the bottom band across the whole window width, as on
the main screen: the Dock reaches down to it and no further.

Each optional screen can also select a physical display and enter native fullscreen. Native
fullscreen controls the application window on the physical display; **Fixed full-screen pane**
controls the content inside that window. Its **Configure Playbacks** dialog provides the same row
controls as the default screen and also selects its page mode. Choose **Follow Main** when its page
tracks the primary page. Choose **Dedicated Page** for an independent operator surface.
Browser-only operation displays the default-screen controls but cannot create or claim support for
native optional-screen windows.

> [!danger] Missing graphic
> Add a screen-ownership diagram showing the default screen, additional screens, connected clients, Desktop or fixed-pane content, encoder placement, playback placement, and Follow Main versus Dedicated Page.

Pressing **X** in an optional screen window closes that window and marks the screen closed, so it
stays closed until you open it again from **Setup → Screens** or with its **Open Screen** action.
The rest of the desk keeps running. Pressing **X** on the main window quits ToskLight.

## Encoder placement

Choose **Configure encoder placement** in the Screens & playback title bar. Its **Encoder placement** modal decides which screen carries the encoder section, independently of the
Playback controls. Choose **Encoders on** to select the main screen or any optional screen, and
**Visible encoders** to show four or six software encoders; attached hardware always keeps its six.

The main screen keeps its whole programmer whatever the placement is. While the encoders live on
another screen, the main screen still carries its command line, its keypad and its Delete and Move
tools, and its Programmer/Playback toggle stays available; only the encoder pane itself moves, and
it names the screen the encoders went to. It does not announce the placement anywhere else; the
setting you made stays out of the main screen list. If the chosen screen is closed, the same modal shows
the placement warning and offers **Use encoders on this screen** to take the encoders back in one
explicit action.

An optional screen that carries the encoders and also shows **Playbacks** or **Page Controls**
offers a **Playback**/**Encoders** switch and starts on **Encoders**. The switch has no row of its
own: on the encoder view it sits at the end of the encoder group tabs, directly right of
**Dynamics**; on the Playback view it sits in the page controls, directly above **PAGE UP**. Neither
section loses height to it. It is a single button carrying both labels, the current section in blue
and the other in white; pressing it changes section. The button keeps the same size and place in
either view, so the row around it never moves. A screen that carries only one of the two shows that section without a
switch.

Playback rows share all available playback height according to their controls. With attached playback hardware, a row without faders uses one height unit and a row with faders uses two. On a touch surface, a one-button row uses one unit and makes the whole playback section its button, with the function label at bottom-right. A two- or three-button faderless row uses two units and places its buttons side by side. A fader row uses four units. The unit size adapts so the configured rows fill the playback area.

![Default screen and playback configuration](../../assets/screenshots/workflows/desk-setup-screens.png)

## Build task Desktops

Create separate Desktops for common jobs such as Programming, Playback, Patch, and diagnostics. Add only the panes needed for that job. Use full built-ins for temporary work that should not change a Desktop. Layout changes are autosaved to desk data.

Desktop layout controls arrange operator panes; they do not define fixture selection geometry.
There is no built-in Layout pane or window. Use Stage to author fixture X/Y/Z positions, Group
settings to define shared Projection and Phase ranking, and a Dynamic's Projection tab for a local
override. Older Desktops that contained the retired Layout pane open without it and keep their
remaining panes.

For pane geometry and available windows, see [Application Layout and Window Manager](../30-Windows/01-desk-interface-and-windows.md).
