# Output and Help Windows

## DMX output

The DMX output pane is a live monitor and diagnostic override surface. Values view displays up to 512 slots per shown universe. Selecting a slot reveals its decimal and hexadecimal address, DIP-switch representation, patched fixture, fixture-channel position, attribute, current raw value, and a 0-255 override control. **Release override** returns that address to normal engine output.

With no slot selected, the information area summarizes output health over the last 60 seconds: the minimum, average, and maximum measured frame rate; a histogram counting the frames delivered in each rate band; and the send errors in that window alongside the total since the current show was opened. The bands are disjoint, so every frame is counted exactly once: a frame at 25 Hz appears in the 20–30 Hz band and nowhere else. The maximum reading is capped at the 60 Hz the desk is asked for, and anything faster reads as “> 60 Hz”. A compact pane stays in Values view, limits the universe list to the first two universes, and uses the global DMX dot-size preference.

The full DMX built-in adds **Sources**, which lists and releases active raw overrides; **Nodes**, which lists every Art-Net and sACN endpoint the desk sends to or hears from; and **DMX Settings**, which changes Small/Large dot size.

**Nodes** shows one row per endpoint with its protocol (Art-Net or sACN, plus Broadcast, Multicast, or Unicast delivery for a route), its direction (**Send** or **Receive**, marked **Configured** or **Heard** on the network), the address and port, the universe (a route shows `logical → wire` universe; a heard source shows its universes as ranges such as `9–11, 20`), and a status with what to do about it:

* **Active** — data is flowing now. **Listening** — the desk is ready to hear this endpoint.
* **Idle** — configured, but nothing was sent or heard recently. **Disabled** — the route is switched off in **Desk Setup > Outputs > Routes**.
* **Conflict** — another source on the network sends a universe this desk also sends. The row names that source; move one of them to another universe.
* **Error** — sending fails or the route is invalid; the row gives the reason. **Unavailable** — the transport behind the endpoint could not start, for example because the output bind address has no broadcast network or does not allow multicast.

Send rows are the show's network routes plus the desk's own announcements (ArtPollReply answers and sACN universe discovery). Receive rows are the ArtPoll and sACN-discovery listeners, the **ArtTimeCode UDP bind** when one is configured, and whatever the desk heard: controllers polling it, other devices broadcasting Art-Net, and other sACN sources announcing their universes. A heard endpoint disappears 25 seconds after it goes quiet. A row that is one of ToskLight's own applications rather than third-party hardware is marked **ToskLight Media Server**, **ToskLight Visualizer** or **ToskLight Desk** beside its role, so an own endpoint is not mistaken for another console on the network; the Media Server and the Visualizer announce those names themselves, and an sACN source keeps the operator's own output label after the name. Select a row to see all of its details in the information area; with nothing selected, the area counts endpoints per protocol and direction and lists those that need attention. The tab reads the desk's network state once a second while it is open. When the desk cannot be read, an error banner stays above the last known list until the next successful read. Output-engine fields and editable logical-universe routes live under **Desk Setup > Outputs**, not in the DMX pane or Pane Settings.

**Pane configuration:** only common size and removal controls.

![DMX output pane](../../assets/screenshots/panes/dmx.png)

## Help

The Help pane renders the same numbered Markdown catalog used to build this manual. Folder navigation selects a topic, safe relative images are loaded from Help assets, and desk buttons, keyboard keys, tables, and links receive their documentation styling. When live Help is enabled, the catalog refreshes automatically.

The catalog remains in a left column and the selected topic remains in a right column, including when Help is embedded as a pane. External links are restricted to safe HTTPS targets and local images cannot traverse outside Help assets.

**Pane configuration:** only common size and removal controls. The selected topic is navigation state rather than a persistent pane-setting field.

![Help pane](../../assets/screenshots/panes/help.png)
