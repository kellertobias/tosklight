# OSC Protocol

Use OSC to connect a hardware surface or a playback remote over a trusted lighting network.
Set its listen binding in the server installation configuration, then check the displayed address in
**Desk Setup > Network & Inputs > Control & server**.

## Choose the control role

| Role | Use it for | Allowed controls |
| --- | --- | --- |
| Desk buttons (`/light/desk/...`) | The operator’s attached surface | Keypad, programming, encoders and playbacks; shares Control’s command line |
| Remote (`/light/remote/...`) | A guest operating the show | Playbacks only; cannot Record, Update, Assign or change Programmer values |

A remote button operates its playback even when Record is armed on the desk. Fader pickup still
applies. Choose the role in the surface’s OSC configuration; it must subscribe using the same role.

## Connect and check

1. Set the desk’s OSC listen address and port. Match them in the surface’s destination settings.
2. Give the surface a unique client ID and a feedback port. Allow both directions through the firewall.
3. Subscribe the surface, then check that its page and playback feedback follow Control.
4. Test a playback press and release. For a programming surface, also test the shared command line.

A current-page playback address follows page changes. An explicit page-and-position address keeps
controlling that exact playback. Flash and Swap must send both press and release; changing pages
while holding one does not move the held action to another playback.

The **ToskLight Hardware Controls** application’s **OSC** mode provides a software surface for this
connection. Its status shows the connection, page and last control sent.

For custom controllers, use the complete [OSC integration reference](https://github.com/kellertobias/tosklight/blob/main/docs/engineering/protocols/01-osc.md).
