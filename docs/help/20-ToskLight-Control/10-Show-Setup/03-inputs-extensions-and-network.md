# Inputs and network control

Preload capture configuration lives in **Desk Setup > Preferences > Others**. **Desk Setup > Network & Inputs** selects **Control & server**, **Sound**, or **Bridges** from the window title. Control & server contains remote-server, native-extension, and OSC status. Sound contains Sound-to-Light input, and Bridges contains Matter. Switching tabs keeps unfinished fields mounted and does not change which service owns them.

![Desk input status and Preload capture settings](../../assets/screenshots/workflows/desk-setup-inputs.png)

## OSC

**Network & Inputs > Control & server** reports the active OSC bind address; it does not edit that binding. Configure the server-side OSC bind through the installation configuration, then return here to verify what the running desk loaded. Bind only to the trusted lighting-network interface. One ToskLight application and its attached OSC hardware form one desk: a physical button continues the desk's visible command and behaves like the corresponding UI button. There is no desk alias to configure. What a surface may do is decided by the path it connects on: `/light/desk/...` is a desk button and takes the full command set, `/light/remote/...` is a remote-control guest and operates playback only. See the [OSC Protocol](../../99-Integration/02-osc.md) reference for both paths.

After binding, test a harmless selection and confirm the command text and result in the application. Avoid exposing OSC to untrusted networks; OSC itself does not provide the desk-token boundary used by remote application connections.

## Sound-to-Light audio input

Under **Network & Inputs > Sound > Sound input**, request microphone access and choose the audio input used by Sound to Light. This is one browser-local selection for the current desk and is shared by Speed Groups A–E. It is not portable show data and is not sent to the server as a device identifier. Use **Refresh inputs** after connecting or renaming an audio interface.

Speed Group source, frequency region, gain, confidence, smoothing, tempo range, hold, and ratio remain in each Speed Group's settings. Open those settings with Shift-tap/Shift-click or a hold on the Speed Group control.

## Software keypad

On **Screens & playback**, the default-screen card can enable or disable all software keyboard shortcuts. Software shortcuts are also disabled automatically while hardware controls are connected so one physical action is not processed twice. The complete key map is in [Command Line Reference](../20-Programmer-and-Cues/01-command-line.md).

## Remote servers

The desktop app normally connects to `http://127.0.0.1:5000`. Change **Light server URL** to operate a remote server, then press **Connect to server**. For a LAN server, ask the installation administrator for the protected connection details.

See [REST API](../../99-Integration/01-rest-api.md) for HTTP session authentication. [OSC](../../99-Integration/02-osc.md) explains surface subscriptions and current-page versus explicit-page playback addresses.

![Remote server configuration](../../assets/screenshots/workflows/desk-setup-network-api.png)

See [Highlight](06-highlight-look.md), [Native extensions](08-native-extensions.md), and [Matter](09-matter-bridge.md) for their setup and operating behavior.
