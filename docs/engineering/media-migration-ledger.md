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
| Phase 2 — skeleton and seams | `Config.h` | `media-domain`, `media-application`, `media-runtime`, `media-server` | `migration::tests` over the `media/.info` document | Configuration becomes a versioned document with an `outputs` collection. A migrated output originally kept a `V1Legacy` marker; document version 2 drops it, because this product was never published and no desk was ever patched against the C++ application's layout | see branch history | pending review |
| Slice 1 — pure domain | `StateStore.h` (`LayerState`, `MasterState`, `AppState`), `DmxMap.*`, `DmxConstants.h` | `media_domain::{address, color, command, dmx, layer, master, personality, playback, speed, state, tempo}` | `personality::decode::tests` over synthesized 512-slot universes; boundary tests over all 256 values of every enumerated channel | v2 personality: 34-slot layers (was 32), Reverse and Reverse Once added, the whole play-mode channel renumbered into unsynchronized / synchronized / transport blocks, each of the four Once families subdivided into Hold/Black/Transparent, both mask axes 16-bit, speed multiplier and Playback BPM channels added; `paused` and `black` removed; layer count explicit instead of `fullMode`; the reducer is the single writer with typed control-source ownership | see branch history | pending review |
| Slice 2 — renderer and one output | `Renderer.*`, `ofApp.*` render lifecycle | `media_domain::{geometry, clock}`, `media_render::{gpu, compositor, offscreen, texture, OutputRenderer}` | 14 deterministic off-screen reference renders over exact pixels; a real window verified on macOS presenting 696 frames at a measured 60.02 fps against a 60 Hz display | [Separate render adapter](media-renderer-reuse-decision.md), not `viz-render`; per-output render clock replacing the fixed 60 fps request; presentation mode chosen from the surface's real capabilities; the target monitor is actually applied; a failed source draws transparent instead of contributing black; the swapchain takes a non-sRGB format so a window and a reference render agree | see branch history | pending review |
| Slice 3 — video playback | video/image playback logic | `media_domain::{timeline, tempo}` | timeline tests over all twenty modes; tempo resolution and rate tests | [HAP Alpha replaces ProRes and H.264 as the playback format on every platform](media-playback-codec-decision.md); FFmpeg decodes out-of-process at import while this repository owns the HAP encoder | see branch history | pending review |
| Slice 4 — catalog and ingestion | `MediaResolver::{scanMedia,getMediaListJSON,getThumbnailPath}`, `MediaIngester::{queueTranscode,checkAndConvertAll,workerLoop,performTranscode,generateThumbnail}` | `media_library::{discover,pending_imports,LibraryStorage,Importer,Upload,thumbnails}`, `media_codec::{import,format}`, `media_domain::catalog` | Catalog discovery and address-collision tests; bounded import-queue progress/cancel/failure tests; storage rename/move/swap/thumbnail/source-alignment tests; upload streaming and cleanup tests; import measured on eleven real 1080p clips; browser import on a copy of the legacy library; running-server upload, catalog publication, move, playback, and thumbnail proof | Stable asset IDs are distinct from mutable addresses; HAP Alpha is the only playback format (D2); imports preserve their source (D22), publish visible jobs, and are bounded; file `0`/`255` stay blank; collisions refuse or explicitly swap instead of overwriting | `b311b68e`, `1905cfe1`, `42d91a14`, `d1be3387`, `066f6485`, `2a4d2876`, `5dd5907e` | pending review |
| Slice 5 — Art-Net and sACN | `ArtNetReceiver::{threadedFunction,processPacket,applyDmxToState,UniverseInfo}`, `SacnReceiver::{threadedFunction,processPacket,applyDmxToState,joinUniverse,UniverseInfo}` | `media_net::{artnet,sacn,arbitration,ingress}`, `media_runtime::dmx`, `media_http::{diagnostics::DmxTelemetry,wire::DmxIngressView}` | Byte fixtures for both protocols; source-priority, sequence, timeout, termination, and per-universe arbitration tests; real disposable UDP listener tests; identical Art-Net/sACN frame-to-state tests; known Art-Net frame observed through running-server telemetry with exact configured-footprint bytes, sender, rate, age, and active state | One host listener per protocol; full Art-Net port address and sACN CID/name/priority are honoured; winning sources expire instead of remaining active forever; raw values and receive diagnostics are protocol-aware pushed telemetry rather than Art-Net-only polling | `0db63917`, `26a41733`, `20843dc4`, `7f642e50`, `066f6485` | pending review |
| Slice 6 — HTTP API and React UI | `WebServer::{setupRoutes,broadcastLoop}`, `apps/web-ui/src/{api.ts,components,pages}` | `media_http::{routes,wire,replay,tolerant,assets,diagnostics}`, `media_runtime::{serve_with,log_buffer,logging}`, `apps/media/src/{app,features,shared}` | Typed route suites (118 tests at parity closure), checked-in Rust-to-TypeScript contract drift test, 74 React tests through the real client/resource cache, embedded-SPA fallback/404 tests, running-server HTTP/WebSocket checks for output settings, library work, DMX diagnostics, logs and restart behavior, and a live browser check of the separate Server/Show log controls | `/api/v2` is output-scoped and typed; configuration edits carry request IDs and persist before publication; volatile state is pushed; this is a trusted-LAN service; log reads use cursors, Server log level is runtime-only, and the independent Show selector is only a client filter | `e0779cb4`, `acd12c1f`, `06be906d`, `24ba54c2`, `06206223`, `066f6485`, `b9e5f78d`, `5dd5907e` | pending review |
| Slice 7 — generated sources and compositing | `TextSourceManager::{update,renderSource,renderSimpleText,renderRichText}`, `AudioAnalyzer::{audioIn,performFFT,detectBeats,calculateBPM}`, `visualizers/*.cpp` | `media_domain::{text,visualizer,audio}`, `media_text::{fonts,raster}`, `media_audio::{service,snapshot}`, `media_render::{compositor,source,shaders/visualizers}`, `media_runtime::{layer_sources,text_sources}` | All twenty shader modules compile, draw, and react in real GPU tests; deterministic compositor and text render fixtures; audio worker tests for silence, bands, beat, BPM, phase, and retuning; real input at 16 kHz moved Equalizer Bars; a clock changed between captures and a countdown started on visibility; legacy text migration exercised on a copy of the real installation | One typed parameter block advertises only controls each visualizer reads (D8), and one broken shader cannot take down the other nineteen (D9); audio callback only feeds a bounded lock-free queue (D14); machine fonts are used with reported substitution (D15); text occupies folders 200–219 and the lossy legacy migration—including flattening rich spans to one styled line—names every simplification (D21) | `6f982719`, `9cb21ae5`, `4dd0ebe8`, `5d9ca47a`, `1398d904`, `881fdf9`, `dae7e0a2` | pending review |
| Slice 8 — CITP server and GDTF | `CITPResponder::{udpLoop,tcpLoop,processCITPPacket,handleMSEX}`, `DmxMap::{generateGdtf,getChannelMap}`, `WebServer` routes `/api/gdtf` and `/api/dmx-map` | `media_citp::{packet,message,server}`, `media_runtime::{citp,preview}`, `media_application::gdtf`, `light_fixture::gdtf`, `media_http::{routes::fixtures,wire::DmxMapView}` | Packet framing and malformed-length fixtures; negotiated MSEX library/thumbnail/status/stream tests; exact byte offsets consumed by Light's client; live `SInf`, `LSta`, `ELIn`, `VSrc`, subscription, and eight valid 320×180 JPEG frames; generated importable GDTF archives whose layer has 27 channels across 34 slots; HTTP fixture downloads and canonical DMX map tests | Media and Light keep independent codecs until their byte contracts prove shared types (D11); preview capture costs nothing until subscribed (D16); GDTF declares Media-owned attributes rather than borrowing standard semantics (D13); GDTF and UI consume the canonical personality table for defaults and value sets; the default port difference is resolved separately below | `72efa537`, `64990198`, `601a3ac5`, `40916eae`, `16469927`, `1ca55181`, `bf58d873` | pending review |
| Phase 4 — integration and cutover | `ofApp` multi-output lifecycle, `WebServer` settings and log routes | `media-runtime` (`presentation`, `off_screen`, `log_buffer`), `media-http` (`routes/{network,text,audio,logs,telemetry}`), `@tosklight/media` settings, text, audio, and log surfaces | Two outputs at different refresh rates in one process; the editors driven against a running server; the legacy text document migrated on a copy of a real installation; the release follow-up decided against real tags | Media packages through its own release follow-up rather than `release.yml` (D19); live text, visualizer, and applicable audio edits reach the shared configuration on the next frame (D20), while output identity and bound listeners truthfully require restart; listen addresses and destinations are separately configurable | see branch history | pending review |

The Slice 8 source row deliberately names `DmxMap`: `GDTFGenerator.*` and `GDTFTemplates.h` exist
only as dirty/untracked work in the old checkout, not in the frozen reference commit.

## Operator-facing parity closure

The route and interface audit found nine capabilities an operator could reach in the frozen
reference application. Each is now either reachable here or recorded as an intentional product
difference. Reference symbols below mean commit `6ddcbd36`; current uncommitted files in the old
checkout are not evidence.

| Gap | Reference surface | Replacement and evidence | Status / deliberate difference |
| --- | --- | --- | --- |
| Full output settings | `WebServer::setupRoutes` GET/POST `/api/settings`; `apps/web-ui/src/components/Settings.tsx` | `routes::outputs::{output_configuration,update_output_configuration}`, `OutputConfigurationView`, and `OutputSettings.tsx`; full-document validation/replay tests and a running-server stored-universe edit followed by restart | Built for target, monitor selector, fullscreen, resolution, presentation/FPS, 2/8 layers, protocol, universe, and start address; every field truthfully says restart. The inert migrated `targetCodec` field remains readable but its obsolete selector is superseded by D2/D22; the stored `status_overlay` field remains but its operator switch/render behavior is dropped because no renderer consumes it. The old handler also failed to apply some fields its UI sent; that bug is not copied. |
| Clip rename and renumber | POST `/api/media/file/update`; `MediaManager.tsx` rename and within-folder drag | `routes::library::update_item`, `LibraryStorage::{rename_item,move_item,swap_items}`, stable `CatalogItemView.id`, and always-visible Library actions; storage/route/UI tests plus running-server disk/catalog proof | Built. Moving refuses an occupied address unless the operator explicitly chooses safe swap; the reference could overwrite. Preserved import sources and thumbnails move with the playable asset. |
| Folder rename | POST `/api/media/folder/update`; `MediaManager.tsx` | `routes::library::update_folder`, `LibraryStorage::rename_folder`, and the Library folder-label editor; storage/route/UI tests and running-server `.info` proof | Built for the operator-visible rename/clear behavior. The reference had an unexposed route-only folder-index swap; it is deliberately not promoted into the product. |
| Upload | POST `/api/upload`; `MediaIngester::queueTranscode`; `MediaManager.tsx` | `routes::library::upload`, `media_library::{Upload,Importer}`, and Library upload controls; bounded multipart, job, cleanup, UI, and running-server PNG-to-playable proof | Built as an explicit destination/address operation with an 8 GiB bound, safe names, occupied/sentinel refusal, visible progress/cancel/failure, source preservation, and HAP Alpha import. The reference trusted filenames, buffered without a bound, and silently allocated/overwrote; those behaviors are rejected. |
| Thumbnails | GET `/api/media/thumbnail`; `MediaResolver::getThumbnailPath`; `MediaManager.tsx` | address-owned `routes::library::thumbnail`, `LibraryStorage::thumbnail_path`, importer generation, and Library image/missing state; JPEG/404 route tests, image/missing-state UI tests, and a running-server JPEG response | Built. Missing images return a typed 404/missing state; arbitrary filesystem paths are never accepted. |
| DMX channel map and GDTF downloads | GET `/api/dmx-map` and `/api/gdtf`; `DmxMap::{getChannelMap,generateGdtf}`; `DmxMapPage.tsx` | `DmxMapView::of`, `personality::channels`, `routes::outputs::dmx_map`, `routes::fixtures`, and `DmxPage.tsx`; canonical metadata/GDTF/route/UI tests | Built per output from one canonical table, including defaults, resolutions, value sets, absolute addresses, and explicit unimplemented effect slots. Generated GDTFs are directly downloadable. |
| Raw DMX values | GET `/api/dmx-values`; `DmxMapPage.tsx` 100 ms polling | `media_runtime::dmx` retained footprint plus `DmxIngressView` on `/api/v2/telemetry`; `DmxPage.tsx`; exact-byte runtime/wire/UI tests and known-frame running proof | Built as protocol/output-aware pushed telemetry. The reference returned only Art-Net bytes even when sACN was selected; polling that bug is not parity. |
| Art-Net/sACN receive diagnostics | GET `/api/artnet` and `/api/sacn`; receiver `UniverseInfo` | `media_net::ingress` source identity/arbitration plus expiring `DmxTelemetry`; telemetry socket and `DmxPage.tsx`; parser/socket/rate/expiry/UI tests and running source/rate/age proof | Built. Active expires, the winning sender is named, and rate/age/coverage are reported. The reference's active flag never expired and did not arbitrate sources correctly. |
| Server log level and re-encode | POST `/api/settings/log-level` calling `ofSetLogLevel`; POST `/api/settings/reencode` calling `MediaIngester::checkAndConvertAll`; `Logs.tsx` / `Settings.tsx` | `routes::logs::{server_level,update_server_level}`, runtime tracing reload handle, separate `LogsPage` controls, and D22 `ImportPanel`; route/replay/UI tests plus running debug-record and restart-reset proof | Runtime log level is built and intentionally resets from `MEDIA_LOG` on restart, as selected by the maintainer. Re-encode and codec choice are superseded: D2 has one HAP Alpha playback format, and D22 Import all is the visible rebuild job. No obsolete H.264/ProRes counterpart action is added. |

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
| CITP port | TCP/UDP **4809** | TCP/UDP **4809** by default; a custom TCP port is advertised exactly |
| GDTF attributes | standard names (`Gobo2` for Folder, `Shutter1` for Play mode, `Pan`/`Tilt` for position) | Media's own attributes, so a console cannot apply unrelated semantics (D13) |
| Text addressing | folder `200`, file = slot − 200, so slot `200` was file `0` | file `0` is a blank sentinel; slot `200`'s content is moved and the move is reported |

**Resolved, 2026-08-09 (maintainer).** The comparison first read as an open question: the Phase 2
migration stored `personalityVersion: v1-legacy` on a migrated output, recording the layout the show
had been programmed against, and nothing read it — the DMX decoder speaks v2 only — so a migrated
installation's cues would have produced different looks, silently.

The question dissolved rather than being answered: **this product was never published**, so there is
no installation whose desk is patched against the C++ application's 32-slot layer, and nothing to
preserve. `PersonalityVersion` and the startup warning that briefly went with it are gone, and
document version 2 drops the stored field. There is one personality, and it is the one this build
speaks. A version 1 document still loads: the migration removes the field rather than refusing the
document, so a development installation is not stranded on something it wrote itself.

The maintainer resolved that split: discovery and the default advertised TCP endpoint both use the
industry-standard port **4809**. A configured custom TCP port is preserved and advertised exactly.

## Acceptance rule

Passing the reference application's checks proves only the reference application. Passing focused
Media checks proves only that slice. Accepting a slice also requires the affected full-repository
checks, so a shared-crate change cannot regress the desk.
