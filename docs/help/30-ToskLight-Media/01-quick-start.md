# Start the Media Server

ToskLight Media Server is a server process with a browser administration interface and separate program-output windows. Starting the server does not automatically put show content on every connected display.

## First packaged launch

Start **ToskLight Media Server** from the installed ToskLight applications. On macOS, the packaged first launch creates its configuration and media-library location under the user's Application Support folder, binds the administration interface for local-network access, and creates one fullscreen Main output on monitor 0. Other packaged platforms use their platform application-data location.

Open the administration address shown by the server. On the server machine the usual address is `http://127.0.0.1:8080`. If another computer is administering it, use the Media Server machine's permitted network address and allow the configured HTTP port through the firewall.

## Development launch

Repository commands are for development, not the normal installed-operator workflow. `npm run open:media` opens the latest existing development build and seeds a development configuration once. `npm run build:media:open` rebuilds before opening it. Those commands use the repository's runtime artifact directory rather than the packaged application-data location.

## Prove the output

1. Open **Outputs** and choose the Main output.
2. Confirm the monitor or off-screen target, fullscreen choice, render resolution, and presentation rate.
3. Enable the output test pattern and look at the physical destination—not only the administration preview.
4. Disable the test pattern, open **Library**, and import or upload one known media file.
5. Wait for conversion to finish, then take control of one layer and select the resulting folder/file slot.
6. Confirm the layer preview, composite preview, and physical output.

Network, monitor, resolution, presentation-rate, sound-device, personality, and DMX-address changes are saved first and become active after a restart. The interface distinguishes saved configuration from the settings the running engine is still using. Playback, layer, master, and takeover changes are live.

If the output is blank, return to the test pattern. A missing physical test pattern points to output/display configuration. A working pattern with missing content points instead to the library job, selected slot, layer state, master state, or DMX ownership.
