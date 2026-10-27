# Start Pixel and check the output

Start the installed **ToskLight Pixel** application. Pixel opens its program output separately from its browser interface. An unconfigured installation creates one enabled **Main** output on the first monitor.

![Pixel administration interface](../assets/screenshots/workflows/pixel-administration.png)

## Open the interface

Read the administration address from the standby picture, the desktop status item, or **Settings > Network & DMX**. Pixel does not open a browser automatically. On the server computer the usual address is `http://127.0.0.1:8080`.

From another computer, open Pixel's reachable network address and allow its configured HTTP port through the firewall. Packaged installations use the platform's application-data location. On macOS, the first launch also enables administration over the local network.

If another application occupies the default port 8080, Pixel chooses a free port and reports it in Network settings. It tries 8080 again on the next start. A custom port is never substituted: free that port or change the setting if Pixel cannot start. If Pixel is already running, a second launch identifies the existing server and stops.

The left dock scrolls in a short browser window, keeping **Settings**, **Take over playback** and connection status reachable. Settings tabs scroll independently with touch, the mouse, or keyboard navigation. Each tab opens at its top.

## Check a physical display

1. Open **Settings > Picture** and choose the Main output's monitor or **Off-screen (no window)** target.
2. Set fullscreen, render resolution and presentation rate for that destination.
3. Restart Pixel if the section shows **Applies on restart**.
4. Enable the output test pattern and check the physical display.
5. Disable the test pattern.
6. [Upload a known image or video](02-library-and-uploads.md#upload-a-slot) and wait for conversion to finish.
7. [Take over playback](04-playback-and-layers.md#take-control), select the file on a layer and raise its Dimmer.
8. Check the layer preview, composite preview and physical output.

The standby picture clears when valid DMX arrives or browser takeover begins, even with no media selected or dimmers at zero. It remains visible while the library is empty. An input configuration problem directs you to the browser settings. Valid DMX with empty playback produces black output.

If the physical test pattern is missing, check the display configuration and cabling. If the pattern works but content is missing, check conversion, the selected slot, layer and Master dimmers, and playback ownership.

![Check the display, prepare media, and then play it](../assets/diagrams/pixel-workflow.svg)

## Back up or move the server data

Open **Settings > Libraries > Show folder on Media Server** to reveal Pixel's portable data folder on the server computer. The macOS menu-bar item also has **open Folder in Finder**.

Copy the whole folder to carry `media-server.json`, media, thumbnails and generated-source settings to another computer. Stop Pixel before replacing that folder on the destination.

To use a show drive or another location, choose **Change folder…** beside **Show folder on Media Server**. Browse the server computer's folders and choose **Use this folder**. Folders containing a Pixel configuration are marked.

An existing `media-server.json` loads that folder's settings and library. Otherwise Pixel copies its current settings there and uses the folder as an empty library; it does not copy media. Pixel restarts automatically and reloads the page. An unreadable, unwritable or invalid folder is refused with a reason, leaving the current folder in use.

Pixel remembers the choice beside its original configuration. If that location is missing on a later start, it uses the original folder. Choose the original folder again to return to it.

To change only the media directory, follow [Prepare the media library](02-library-and-uploads.md). The settings that need a restart are listed under [Outputs and displays](03-outputs-and-displays.md#apply-settings).
