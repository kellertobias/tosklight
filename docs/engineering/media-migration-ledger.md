# Media Server migration ledger

Authority moves one accepted slice at a time, not in a flag day.

Before a slice starts, the reference application plus the product contract define observed and
desired behavior. The slice gains characterization fixtures, then a Rust/React implementation and
acceptance tests in the Media worktree. After parity and any deliberate differences are reviewed,
the tests and documentation here become authoritative for that slice, and this ledger records
what happened.

There is no bidirectional synchronization. Source and target are separate Git histories, recorded
independently; no commit ever stages files from both.

## Worktree

| Field | Value |
| --- | --- |
| Branch | `worktree-tl-52-media-server` |
| Worktree | `.claude/worktrees/tl-52-media-server` |
| Baseline commit on `main` | `adbc624d5b90929e564be16e5fa3e49d5540da79` |
| Reference commit | `6ddcbd36be2a458712613c27a871ba9607f7eb71` |

## Ledger

Each accepted slice records the reference symbols it replaces, the symbols that replace them, the
fixture evidence, any deliberate difference, the target commit, and who accepted it.

| Slice | Source symbols | Target symbols | Fixture evidence | Deliberate differences | Target commit | Accepted by |
| --- | --- | --- | --- | --- | --- | --- |
| Phase 0 — freeze scope | — | [`docs/engineering/media-legacy-inventory.md`](media-legacy-inventory.md) | — | — | see branch history | pending review |
| Phase 2 — skeleton and seams | `Config.h` | `media-domain`, `media-application`, `media-runtime`, `media-server` | `migration::tests` over the `media/.info` document | Configuration becomes a versioned document with an `outputs` collection; a migrated output keeps `V1Legacy` rather than being silently renumbered to the v2 personality | see branch history | pending review |
| Slice 1 — pure domain | `StateStore.h` (`LayerState`, `MasterState`, `AppState`), `DmxMap.*`, `DmxConstants.h` | `media_domain::{address, color, command, dmx, layer, master, personality, playback, speed, state, tempo}` | `personality::decode::tests` over synthesized 512-slot universes; boundary tests over all 256 values of every enumerated channel | v2 personality: 34-slot layers (was 32), Reverse and Reverse Once added, the whole play-mode channel renumbered into unsynchronized / synchronized / transport blocks, each of the four Once families subdivided into Hold/Black/Transparent, both mask axes 16-bit, speed multiplier and Playback BPM channels added; `paused` and `black` removed; layer count explicit instead of `fullMode`; the reducer is the single writer with typed control-source ownership | see branch history | pending review |
| Slice 2 — renderer and one output | `Renderer.*`, `ofApp.*` render lifecycle | `media_domain::{geometry, clock}`, `media_render::{gpu, compositor, offscreen, texture, OutputRenderer}` | 14 deterministic off-screen reference renders over exact pixels; a real window verified on macOS presenting 696 frames at a measured 60.02 fps against a 60 Hz display | [Separate render adapter](media-renderer-reuse-decision.md), not `viz-render`; per-output render clock replacing the fixed 60 fps request; presentation mode chosen from the surface's real capabilities; the target monitor is actually applied; a failed source draws transparent instead of contributing black; the swapchain takes a non-sRGB format so a window and a reference render agree | see branch history | pending review |
| Slice 3 — video playback | video/image playback logic | `media_domain::{timeline, tempo}` | timeline tests over all twenty modes; tempo resolution and rate tests | [HAP Alpha replaces ProRes and H.264 as the playback format on every platform](media-playback-codec-decision.md); FFmpeg decodes out-of-process at import while this repository owns the HAP encoder | see branch history | pending review |
| Slice 4 — catalog and ingestion | `MediaResolver.*`, `MediaIngester.*` | — | — | — | — | — |
| Slice 5 — Art-Net and sACN | `ArtNetReceiver.*`, `SacnReceiver.*` | — | — | — | — | — |
| Slice 6 — HTTP API and React UI | `WebServer.*`, `apps/web-ui` | — | — | — | — | — |
| Slice 7 — generated sources and compositing | `TextSourceManager.*`, `AudioAnalyzer.*`, `visualizers/` | — | — | — | — | — |
| Slice 8 — CITP server and GDTF | `CITPResponder.*`, `GDTFGenerator.*`, `GDTFTemplates.h` | — | — | — | — | — |
| Phase 4 — integration and cutover | `ofApp` multi-output lifecycle, `WebServer` settings and log routes | `media-runtime` (`presentation`, `off_screen`, `log_buffer`), `media-http` (`routes/{network,text,audio,logs,telemetry}`), `@tosklight/media` settings, text, audio, and log surfaces | Two outputs at different refresh rates in one process; the editors driven against a running server; the legacy text document migrated on a copy of a real installation; the release follow-up decided against real tags | Media packages through its own release follow-up rather than `release.yml` (D19); the configuration document is shared with the outputs so a stored edit is live on the next frame (D20); listen addresses and destinations are separately configurable and a rebind waits for a restart | see branch history | pending review |

## Side-by-side comparison, 2026-08-09

Both servers were run at once on one machine — the reference application from
`build/bin/server-core.app` on its own ports, this one on `6455/5569/4812/8081` — each with its own
copy of the same real library, and each sent the identical Art-Net frame (universe 0, folder `1`,
file `4`, play mode `128`, and one dimmer slot at full).

They disagree, as designed, and the disagreements are worth stating plainly because a migrated
installation meets all of them at once:

| | Reference | This server |
| --- | --- | --- |
| Play mode `128` | `Once` — five bands over the channel | `Reverse Synced` — the v2 three-block layout (D1) |
| Layer pitch | 32 slots | 34 slots, so every channel after the third differs |
| Dimmer at slot 13 | layer 1's dimmer | a different channel of layer 1 |
| CITP port | TCP/UDP **4809** | TCP **4811**, the port `docs/citp-media-servers.md` settles on |
| GDTF attributes | standard names (`Gobo2` for Folder, `Shutter1` for Play mode, `Pan`/`Tilt` for position) | Media's own attributes, so a console cannot apply unrelated semantics (D13) |
| Text addressing | folder `200`, file = slot − 200, so slot `200` was file `0` | file `0` is a blank sentinel; slot `200`'s content is moved and the move is reported |

**The open question this raises.** The Phase 2 migration stores `personalityVersion: v1-legacy` on a
migrated output, recording the layout the show was programmed against — but nothing reads it: the
DMX decoder speaks v2 only. So a migrated installation's existing cues produce different looks, and
until 2026-08-09 they did so in silence. A migrated output now says at startup that the desk must be
repatched from the generated GDTF fixture.

That warning makes the behaviour honest; it does not decide the policy. Cutover needs one of:

1. **Repatch on cutover.** v1 stays read-only metadata, an operator repatches from the generated
   fixture, and the stored version exists only to trigger the warning. Cheapest, and it matches
   "moving to v2 is a deliberate operator action".
2. **Read v1 as v1.** The decoder honours the stored version — 32-slot layers, five play-mode bands
   — so an untouched desk keeps working. A second decode path and a second GDTF fixture, and the
   personality stops being one canonical table.

A console's port also has to be moved from 4809 to 4811, or the port made configurable per output
rather than per process.

## Acceptance rule

Passing the reference application's checks proves only the reference application. Passing focused
Media checks proves only that slice. Accepting a slice also requires the affected full-repository
checks, so a shared-crate change cannot regress the desk.
