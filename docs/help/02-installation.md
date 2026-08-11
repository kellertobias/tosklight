# Installation and First Start

ToskLight ships as a desktop application with its own Light server, and as standalone server archives for browser-connected desks.

## Desktop application

1. Download `tosklight-bundle-macos_arm64.zip` from the matching GitHub release.
2. Expand the ZIP and move **tosklight-desk-macos_arm64.app** to Applications.
3. Open the application. The bundled server uses `127.0.0.1:5000` and stores desk data in the application data location.
4. The testing build is ad-hoc signed for bundle integrity, but is not yet Developer-ID signed or notarized. Verify that the archive came from the expected GitHub release, then Control-click the application in Finder, choose **Open**, and confirm **Open**. If directed to System Settings, use **Privacy & Security > Open Anyway** for ToskLight.
5. Open **Desk Setup > Network & Inputs** and confirm the active server URL.

Do not remove quarantine attributes or re-sign the downloaded application. If macOS reports that
it is damaged instead of offering the approval path above, verify the ZIP against
`report-checksums.txt` and use the newest release; that message indicates a release-integrity bug.
The archive also includes `macos-first-start.txt` beside the applications for offline reference.

The separate **ToskLight Hardware Controls** application is used for the attached hardware-control surface when that artifact is included in the release.

## Standalone server and browser desk

Choose the archive matching macOS Apple Silicon, Windows AMD64, Linux AMD64, or Linux ARM64. Start `light-headless` with a writable data directory, then open the displayed address in a supported browser. Use `--bind 0.0.0.0:5000` only on a trusted lighting network.

When the server is reachable over a LAN, set `LIGHT_DESK_TOKEN` before starting it. Browser and API clients must then send that shared desk token. Users remain passwordless operator identities inside that protected desk boundary.

## First-start checklist

- The status in Desk Setup is connected and an `Operator` user exists.
- A new show can be created and appears in the show library.
- The correct physical screens are assigned.
- OSC, native extensions, Art-Net, and sACN are disabled until intentionally configured.
- The DMX view reports the expected frame rate with no send errors.
- A named revision can be created and loaded as a separate autosaved copy before real programming begins, without rewinding the original show's Latest Autosave.

See [Desk Setup](10-Desk-Setup/index.md) next. Developers building from source should use the repository `./build open` and `./build archive` commands described in the README.
