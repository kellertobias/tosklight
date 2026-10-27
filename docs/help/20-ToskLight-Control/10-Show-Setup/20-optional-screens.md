# Optional operator screens

The Tauri desktop application can add optional screens. Choose **Add Screen** in the Screens &
playback title bar; it sits in its own button group, apart from **Undo** and the configuration
actions. The browser version shows no **Add Screen** action. Each new screen appears as a row with
its name and the **Copy browser link**, **Configure screen**, **Open Screen**, and **Remove Screen**
buttons.

**Configure screen** opens one modal with the **Layout**, **Settings**, **Placement**, and
**Playbacks** tabs in its title bar. Each tab is a single form without extra frames or headings, and
every change saves immediately. On **Layout**, **Content** and the chosen pane or Desktop span the
form; **Dock** and **Playbacks** sit side by side above **Command line** and **Programming**, and
**Page controls** follows below. In a narrow modal the four switches stack in one column and the
form scrolls. **Playbacks** edits the screen's playback rows in place, without a second dialog:
choose **Add Row** in the modal title bar, drag a row by its handle to reorder it, or remove it with
its delete button. A row layout that would run past Playback position 1000 stays on screen with a
**Not saved yet** note until you correct it; switching tabs keeps what you typed. If the desk
rejects a change, the modal says **Could not save this screen** with the reason.

Each optional screen can use its normal
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
chosen widget keeps its configured **Pane width (%)** (a share of the window width, so it holds its
proportion on every display) over the full height on that side, and the
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
controls the content inside that window. Its **Playbacks** tab provides the same row controls as
the default screen's **Configure Playbacks** dialog and also selects its page mode. Choose **Own page** for independent page selection, **Pinned page** for one fixed page, or **Follow surface** to track another named surface. Follow chains cannot loop. The same show-owned assignments appear on every surface displaying their address.
With the Dock shown, **Exit fullscreen** is at its top. With the Dock hidden, it is in the upper-right corner.

Browser-only operation displays the default-screen controls but cannot create or claim support for
native optional-screen windows.

Pressing **X** in an optional screen window closes that window and marks the screen closed, so it
stays closed until you open it again from **Setup → Screens** or with its **Open Screen** action.
The rest of the desk keeps running. Pressing **X** on the main window quits ToskLight.

An optional screen window is part of the same desk as the main window. It uses the main window's
server and operator session, so it shows the same command line, selection and live state; it never
starts a desk of its own and has no server setting. If a screen cannot join (for example because
the server is not reachable or the main window is still connecting) the screen says why and
retries on its own. **Retry now** tries again at once; pressing it repeatedly never opens another
desk. Keep the main window open and connected while screens are in use.

See [Encoder placement](21-encoder-placement.md) when another screen should carry the encoders.
