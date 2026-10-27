# Screens and Desktop layouts

Open **Show > Desk Setup > Screens & playback** to configure the primary desk surface and, in the desktop application, optional operator screens.

## Configure a screen

For the default screen, set its operator-facing name. Choose **Configure Playbacks** to set the playbacks per row and add playback rows. Each row has its own first playback number, fader availability, and number of buttons. The default screen owns the Main Page. **Enable software keyboard shortcuts** controls the complete software shortcut layer for this screen; attached hardware disables that layer automatically. These changes take effect and save immediately. Choose **Undo** in the Screens & playback title bar to reverse the most recent actual default-screen change. Opening a modal or moving between setup sections does not create an undo step.

Choose **Known windows** to see every window that has connected. Every window operates the one desk, so the list identifies windows rather than desk configurations to pick between. Connected windows appear first, followed by disconnected ones; each group is ordered by the most recent connection. Every entry shows its stable client identity, connection state, and last-connected time. Older migrated entries show **Last connected unknown** until that window connects again. The window you are looking at is marked **This window**.

A disconnected window can be dropped with **Forget window** and a named confirmation, or every disconnected one at once with **Forget other windows**. This window and any connected window cannot be forgotten. Forgetting removes only the record that the window has connected before. The desk is untouched: its page, playback selection, desk lock, and Update defaults belong to the desk and outlive any single window, as do portable shows, users, optional screens, installation-wide configuration, Virtual Playback assignments, and show-owned Solo Regions. A forgotten window that connects again is simply known again, operating the same desk.

Choose **Configure desk lock** in the Screens & playback title bar to open its configuration modal. Set the lock message, unlock control, and optional wallpaper, then choose **Save Lock Configuration** in the modal title bar. The Show menu's **Lock Desk** action applies that saved configuration.

When hardware is connected, **Hardware lighting** provides virtual faders for **LED brightness**, **Gooseneck brightness**, and **Gooseneck color**. Move a fader from 0 to 100%; changes apply automatically while you move it. On Compact Connect, color blends its two lamp channels: 0 is blue, 100 is white, and intermediate values mix them. A brightness of 0 turns that illumination off. The settings are saved with the desk and restored when the hardware reconnects. They apply while ToskLight controls the hardware; another application that takes ownership keeps its own lighting settings.


Continue with [Optional screens](20-optional-screens.md), [Encoder placement](21-encoder-placement.md), or [Task Desktops](22-desktop-layouts.md).
