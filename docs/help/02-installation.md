# Installation and First Start

ToskLight ships as a desktop application with its own Light server, and as standalone server archives for browser-connected desks.

## Desktop application

1. Download the macOS ToskLight application archive from the matching Forgejo release.
2. Expand the ZIP and move **ToskLight.app** to Applications.
3. Open the application. The bundled server uses `127.0.0.1:5000` and stores desk data in the application data location.
4. If macOS blocks an unsigned development build, verify that the archive came from the expected Forgejo release before using the Finder **Open** confirmation.
5. Open **Desk Setup > Network & API** and confirm the active server URL.

The separate **ToskLight Hardware Controls** application is used for the attached hardware-control surface when that artifact is included in the release.

## Standalone server and browser desk

Choose the archive matching macOS universal, Windows AMD64, Linux AMD64, or Linux ARM64. Start `light-server` with a writable data directory, then open the displayed address in a supported browser. Use `--bind 0.0.0.0:5000` only on a trusted lighting network.

When the server is reachable over a LAN, set `LIGHT_DESK_TOKEN` before starting it. Browser and API clients must then send that shared desk token. Users remain passwordless operator identities inside that protected desk boundary.

## First-start checklist

- The status in Desk Setup is connected and an `Operator` user exists.
- A new show can be created and appears in the show library.
- The correct physical screens are assigned.
- OSC, MIDI, RTP-MIDI, Art-Net, and sACN are disabled until intentionally configured.
- The DMX view reports the expected frame rate with no send errors.
- A named revision can be created and loaded as a separate autosaved copy before real programming begins, without rewinding the original show's Latest Autosave.

See [Desk Setup](10-Desk-Setup/index.md) next. Developers building from source should use the repository `./build open` and `./build archive` commands described in the README.
