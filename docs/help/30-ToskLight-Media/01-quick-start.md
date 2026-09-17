# Start ToskLight Pixel

ToskLight Pixel is a media-server process with a browser administration interface and separate program-output windows. Starting Pixel does not automatically put show content on every connected display.

## First packaged launch

Start **ToskLight Pixel** from the installed ToskLight applications. An unconfigured first launch creates one enabled Main output in a window on monitor 0, where the standby picture tells you which address to open in a browser. You can later make it fullscreen or choose **Off-screen (no window)** under **Settings > Outputs**. On macOS, the packaged first launch also creates its configuration and media-library location under the user's Application Support folder and binds the administration interface for local-network access. Other packaged platforms use their platform application-data location.

Open **Settings > Libraries** and use **Show folder on Media Server** to reveal the single portable data folder on the server computer. On macOS, the Pixel menu-bar item also offers **open Folder in Finder**. On macOS and Windows, the desktop status item offers **Convert multiple files**. Copy that whole folder—not only its `Media` subfolder—to carry `media-server.json`, uploaded media, thumbnails, and generated-source configuration to another computer. Stop Pixel before replacing the folder on the destination computer.

To keep the configuration and media somewhere else, such as a show drive, choose **Change folder…** beside **Show folder on Media Server**. The picker lists the folders on the Media Server computer, marks the ones that already hold a Pixel configuration, and **Use this folder** makes the open folder both the configuration folder and the media-library root. When the folder already contains `media-server.json`, Pixel loads that configuration and its library; otherwise it writes a copy of the current settings there and uses the folder itself as the empty library. Media is not copied. Pixel then restarts and the page reloads when it is back. A folder that does not exist, cannot be read or written, or holds a configuration Pixel cannot use is refused with the reason, and the current folder stays in use. The choice is remembered in `media-server-location.json` beside the original configuration; if the chosen folder is missing at a later start, Pixel starts from the original folder. Choosing the original folder again returns to it.

Open the administration address shown by the server. On the server machine the usual address is `http://127.0.0.1:8080`. If another computer is administering it, use the Media Server machine's permitted network address and allow the configured HTTP port through the firewall.

In a short browser window, the Pixel dock on the left scrolls as one piece, so **Settings**, **Take over playback**, and the connection state stay reachable, and the dock keeps the open destination in view. Each Settings tab scrolls on its own below the window title with the mouse wheel, a trackpad, or touch. Click or tab into the tab's content, then use the arrow keys, **Page Up**/**Page Down**, or **Home**/**End**. Choosing another Settings tab opens it at its top.

## Development launch

Repository commands are for development, not the normal installed-operator workflow. `npm run open:media` opens the latest existing development build and seeds a development configuration once. `npm run build:media:open` rebuilds before opening it. Those commands use the repository's runtime artifact directory rather than the packaged application-data location.

## Prove the output

1. Open **Outputs** and choose the Main output.
2. Confirm the monitor or off-screen target, fullscreen choice, render resolution, and presentation rate.
3. Enable the output test pattern and look at the physical destination—not only the administration preview.
4. Disable the test pattern, open **Library**, and import or upload one known media file.
5. Wait for conversion to finish, then take control of one layer and select the resulting folder/file slot.
6. Confirm the layer preview, composite preview, and physical output.

> [!danger] Missing graphic
> Add an annotated Pixel administration overview showing Outputs, Library, layer preview, composite preview, physical output, and test-pattern controls.

Library-directory, network, monitor, resolution, presentation-rate, sound-device, personality, and DMX-address changes are saved first and become active after a restart. The interface distinguishes saved configuration from the settings the running engine is still using. Playback, layer, master, and takeover changes are live.

If the output is blank, return to the test pattern. A missing physical test pattern points to output/display configuration. A working pattern with missing content points instead to the library job, selected slot, layer state, master state, or DMX ownership.
