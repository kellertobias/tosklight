# OSC, MIDI, and Network Control

Preload capture configuration lives in **Desk Setup > Programmer**. MIDI, OSC, RTP-MIDI, Matter, and remote-server status live together in **Desk Setup > Network & Inputs**.

![Desk input status and Preload capture settings](../assets/screenshots/workflows/desk-setup-inputs.png)

## OSC

Network & Inputs reports the active OSC bind address; it does not edit that binding. Configure the server-side OSC bind through the installation configuration, then return here to verify what the running desk loaded. Bind only to the trusted lighting-network interface. One ToskLight application and the OSC hardware subscribed to its alias form one desk: a physical button continues that desk's visible command and behaves like the corresponding UI button. A different desk alias retains its own command line, page, and button state. Programmer values are owned by the logged-in user instead, so a value that has been confirmed into that user's programmer is visible from the same user's sessions on every desk without copying the originating desk's unfinished interaction state.

After binding, test a harmless selection and confirm the command text and result in the application. Avoid exposing OSC to untrusted networks; OSC itself does not provide the desk-token boundary used by REST and WebSocket clients.

## MIDI and RTP-MIDI

Network & Inputs reports selected native MIDI inputs and the active RTP-MIDI bind; those values are not editable from this screen. Configure them in the installation/server configuration and return here to verify the running state. Timecode source priority and fallback are reported separately under **Timecode**.

## Software keypad

On **Screens & playback**, the default-screen card can enable or disable all software keyboard shortcuts. Software shortcuts are also disabled automatically while hardware controls are connected so one physical action is not processed twice. The complete key map is in [Command Line Reference](../30-Programmer/01-command-line.md).

## Matter playback bridge

Open **Show > Enter Setup > Network & Inputs**, then use the **Matter server disabled** toggle. When enabled, its label changes to **Matter server enabled**. This is physical desk installation data, not Desktop layout or show data: changing a Desktop, changing or loading a show, or switching the current playback page does not change the setting, pairing identity, or commissioned fabrics. Disabling the setting stops Matter networking and advertising without deleting that persisted identity.

When the status says **Ready to commission**, enter the displayed **Manual pairing code** in the Matter controller. **QR payload** exposes the standard `MT:` payload for controller or integration tooling. A basic commissioning window is time-limited by Matter; if the desk has not yet been commissioned and the window expires, disable and re-enable the bridge to open a new window. **Starting Matter networking…** means the UDP and mDNS sockets are not ready yet. A displayed error, such as a port conflict or missing suitable network interface, means the desk is not advertising and is not commissionable.

Every assigned page/playback control becomes one dimmable Matter light, including one-button and faderless Virtual Playbacks. Its endpoint is derived from the explicit global page and playback address, so changing the page visible on an operator screen never retargets a Matter light. Empty page/playback slots and playback-pool entries that are not assigned to a page remain unexposed. Adding, removing, or renaming an assigned playback briefly restarts only Matter networking so controllers can rediscover the changed endpoint list.

Matter On/Off and Level Control writes use the same authoritative playback dispatcher as desk controls. A faderless assignment gains a Matter-only virtual master without adding a fader to its desk layout: a non-zero Level activates it at that master, Off or Level 0 reports off, and a later On restores the current non-zero level or starts at full. Where the assignment retains Temp or manual XFade behavior, the virtual master uses that same authoritative runtime position. Desk-side button actions, fader movement, tracking, automatic release, and other playback changes are read from the same authoritative runtime and mirrored back to subscribed Matter controllers. The standard Matter UDP port is exclusive; only one Matter service can own it on a host.

Current builds use the official `rs-matter` development vendor, product, and attestation credentials because ToskLight does not yet ship CSA-issued production credentials. Controllers that accept development devices can commission the bridge; a controller that requires certified production attestation may warn or reject it.

## REST, WebSocket, and remote servers

The desktop app normally connects to `http://127.0.0.1:5000`. Change **Light server URL** to operate a remote server, then press **Connect to server**. REST provides snapshots and coarse operations; WebSocket carries live events and typed controls. A LAN server should use `LIGHT_DESK_TOKEN`.

For address structure, authentication, subscriptions, current-page versus explicit-page playback addressing, and the main REST resource families, continue to [OSC, REST, and WebSocket Protocols](../50-Protocols/01-osc-rest-and-websocket.md).

![Remote server, REST, and WebSocket configuration](../assets/screenshots/workflows/desk-setup-network-api.png)
