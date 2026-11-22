# Installation and First Start

ToskLight ships as a desktop application with its own Light server, and as standalone server archives for browser-connected desks.

## Desktop application

1. Download `tosklight-bundle-macos_arm64.zip` from the matching GitHub release.
2. Expand the ZIP and move the application you want to use to Applications.
3. Open the application. A release built with Apple distribution credentials is Developer ID signed and notarized. Testing builds use ad-hoc signatures instead. If macOS blocks a testing build, verify its download source, then open **System Settings > Privacy & Security**, choose **Open Anyway**, and confirm **Open**. On macOS Sequoia and later, Control-click does not bypass this approval.
4. For optional local signing of a testing build, open Terminal in the expanded bundle and run `bash sign-macos-apps-locally.sh`. For an individually downloaded application, pass its path, for example `bash sign-macos-apps-locally.sh "/Applications/ToskLight.app"`. The script verifies existing Developer ID signatures and keeps them; other recognized ToskLight apps receive fresh ad-hoc signatures. Local signing does not replace macOS approval.
5. Control's bundled server uses `127.0.0.1:5000`; open **Desk Setup > Network & Inputs** and confirm the active server URL.

The signing script does not remove quarantine or disable Gatekeeper. If it fails, verify the ZIP
against `report-checksums.txt` and use the newest release instead of overriding macOS security
checks. The archive also includes `macos-first-start.txt` beside the applications for offline
reference.

For a Homebrew installation of all three applications, follow the
[Homebrew instructions on the downloads page](https://tosklight.de/downloads/#homebrew).
The `tosklight` cask installs Control, Architect with its visualizer, and Pixel from a prebuilt
GitHub Release on Apple Silicon Macs. Homebrew also installs FFmpeg and ffprobe for Pixel imports.
Before installation, Homebrew signs testing builds locally and verifies all three apps and their
helpers. Valid Developer ID signatures are preserved. Rerun the Homebrew installation script to
refresh signatures on an existing installation, even if its version is unchanged.
The same macOS approval steps apply to testing builds installed through Homebrew.

## macOS launcher and updates

Keep **ToskLight Launcher** beside the product applications in Applications and pin only the
launcher in the Dock. Choose **ToskLight Control**, **ToskLight Architect**, or **ToskLight Pixel**
to open that application. Each running product has its own Dock icon. Click the launcher icon
again to return to the chooser.

The launcher displays **Installed** and **Latest** versions for each product and checks GitHub
Releases when opened. Use **Check for updates** to check again. When versions differ, click the
product's **Update** link or **Update all ToskLight applications**. Either action installs the
complete matching suite, including the launcher. **Change log** opens that GitHub release's
notes in your default browser.

Save your work and quit the product applications before updating. The launcher shows download
and verification progress; **Cancel** leaves installed applications unchanged. During installation,
the launcher closes and an update window shows progress. The launcher then reopens with the result.
Shows, libraries, and desk settings stay in their existing data folders. An installation failure
restores the previous applications or reports any backup paths that require attention.

If the Applications folder is not writable, use Homebrew to update, or move the complete suite
to your own Applications folder. Local signature verification and macOS first-open approval remain
separate checks.

## Media import and hardware controls

Pixel media import requires separately installed, working `ffmpeg` and `ffprobe` executables. These
dependencies are not included in the application downloads. Existing playback and application
startup checks do not prove that importing new media is available on a machine without them.

Double-click **tosklight-media-macos_arm64.app** to start ToskLight Pixel on the first display. Before the
first valid DMX frame it shows the browser address and projector-alignment marks; its persistent
configuration and library live under Application Support.

Use the separate **ToskLight Hardware Controls** application for an attached control surface when
it is included in the release. Choose **OSC** for an on-screen surface over the network, or
**Native Hardware** for a configured attached device. Native mode mirrors device feedback and
dims its on-screen controls. Set **Desk HTTP port** in Settings if Control uses a port other than
5000. The selected mode is remembered.

## Standalone server and browser desk

Choose the archive matching macOS Apple Silicon, Windows AMD64, Linux AMD64, or Linux ARM64. Start `light-headless` with a writable data directory, then open the displayed address in a supported browser. Use `--bind 0.0.0.0:5000` only on a trusted lighting network.

When the server is reachable over a LAN, set `LIGHT_DESK_TOKEN` before starting it. Browser and API clients must then send that shared desk token. Every connected surface operates the same desk and Programmer.

## First-start checklist

- Desk Setup reports a connected server and the expected active show.
- A new show can be created and appears in the show library.
- The correct physical screens are assigned.
- OSC, native extensions, Art-Net, and sACN are disabled until intentionally configured.
- The DMX view reports the expected frame rate with no send errors.
- A named revision can be created and loaded as a separate autosaved copy before real programming begins, without rewinding the original show's Latest Autosave.

Continue with [Architect](../10-ToskLight-Architect/index.md) to plan the rig, or [Control setup](../20-ToskLight-Control/10-Show-Setup/index.md) to work directly at the desk.

The source and distribution terms are in the [ToskLight repository](https://github.com/kellertobias/tosklight), the [Community License](https://github.com/kellertobias/tosklight/blob/main/LICENSE), and the [third-party notices](https://kellertobias.github.io/tosklight/third-party-licenses.html).
