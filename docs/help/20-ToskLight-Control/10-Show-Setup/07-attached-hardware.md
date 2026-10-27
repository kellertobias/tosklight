# Attached hardware controls

The separate **ToskLight Hardware Controls** application is used for the attached hardware-control surface when that artifact is included in the release.

Hardware Controls has an **OSC | Native Hardware | Native Simulator** mode switch in its title bar, and the status
beside it always names the active mode:

- **OSC** sends the on-screen controls to the desk over the documented
  [OSC protocol](../../99-Integration/02-osc.md) and shows whether the desk is connected, its current
  page, and the last control sent.
- **Native Hardware** exercises a device attached to the desk through its native extension. The
  device's controls go straight to the desk, so the on-screen controls are dimmed and only mirror
  feedback. The status shows whether the device is connected, starting, unavailable (no extension
  configured) or in error, with the extension's last error. It reads that health from the desk
  server, so set **Desk HTTP port** in Settings when the server does not use port 5000.
- **Native Simulator** connects the on-screen surface to the loopback relay of the supervised
  simulator extension. Its buttons, faders, and encoders become typed native-extension inputs;
  this mode does not open an OSC subscription. Build and install the simulator extension as
  described in the [native-extension SDK](https://github.com/kellertobias/tosklight/blob/main/docs/engineering/native-extension-sdk.md) before using
  this mode.

Switching mode closes the previous connection before the new one opens, and the choice is kept for
the next launch.

See [Inputs and network control](03-inputs-extensions-and-network.md) for connection status and [Keypad and keyboard reference](../../99-Appendix/00-keypad-and-keyboard.md) for the desk controls.
