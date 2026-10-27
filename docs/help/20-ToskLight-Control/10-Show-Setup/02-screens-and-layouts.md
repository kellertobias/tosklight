# Screens and Desktop layouts

Open **Show > Desk Setup > Screens & playback** to configure the primary desk surface and, in the desktop application, optional operator screens.

## Configure a screen

Name the default screen, then choose **Configure Playbacks**. Each row sets **First Playback Number**, **Playbacks in row**, **Input type**, and **Buttons**. Choose its **Page source**, then **Save**.

A **Fader** row controls levels. A **Buttons** row provides actions only. **Potentiometer or encoder** provides a rotary level control: turn or drag for level, press its centre for the Playback action, or choose **Set value** for numeric entry. These are separate from Programmer encoders.

**Enable software keyboard shortcuts** controls this screen's software shortcuts; attached hardware disables them automatically. Other default-screen changes save immediately. **Undo** in the setup title reverses the latest saved change. Opening settings or changing setup sections does not create an Undo step.

Choose **Known windows** to see every window that has connected. Every window operates the one desk, so the list identifies windows rather than desk configurations to pick between. Connected windows appear first, followed by disconnected ones; each group is ordered by the most recent connection. Every entry shows its stable client identity, connection state, and last-connected time. Older migrated entries show **Last connected unknown** until that window connects again. The window you are looking at is marked **This window**.

A disconnected window can be dropped with **Forget window** and a named confirmation, or every disconnected one at once with **Forget other windows**. This window and any connected window cannot be forgotten. Forgetting removes only the record that the window has connected before. The desk is untouched: its page, playback selection, desk lock, and Update defaults belong to the desk and outlive any single window, as do portable shows, users, optional screens, installation-wide configuration, Playback assignments, and show-owned Solo Regions. A forgotten window that connects again is simply known again, operating the same desk.

Choose **Configure desk lock** in the Screens & playback title bar to open its configuration modal. Set the lock message, unlock control, and optional wallpaper, then choose **Save Lock Configuration** in the modal title bar. The Show menu's **Lock Desk** action applies that saved configuration.

When hardware is connected, **Hardware lighting** provides virtual faders for **LED brightness**, **Gooseneck brightness**, and **Gooseneck color**. Move a fader from 0 to 100%; changes apply automatically while you move it. On Compact Connect, color blends its two lamp channels: 0 is blue, 100 is white, and intermediate values mix them. A brightness of 0 turns that illumination off. The settings are saved with the desk and restored when the hardware reconnects. They apply while ToskLight controls the hardware; another application that takes ownership keeps its own lighting settings.

Continue with [Optional screens](20-optional-screens.md), [Encoder placement](21-encoder-placement.md), or [Task Desktops](22-desktop-layouts.md).
