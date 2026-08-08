# Media Server legacy inventory

The C++/openFrameworks application is the implementation reference for the Media Server rebuild
where its behavior is intentional. Accidental limitations, duplicated implementations, stale
documentation, and platform-specific omissions are not part of the product contract.

This inventory is the Phase 0 freeze: what exists in the reference application, what is being
transferred, and what is deliberately excluded. It is a snapshot taken for the rebuild, not a
living mirror — a later change to already-migrated reference behavior is a new product
requirement, not something to re-synchronize.

| Field | Value |
| --- | --- |
| Reference commit | `6ddcbd36be2a458712613c27a871ba9607f7eb71` |
| Inventory taken | 2026-08-08 |
| Target baseline | `adbc624d5b90929e564be16e5fa3e49d5540da79` on `main` |

The reference checkout stays runnable as a comparison oracle until cutover, but this repository
never reaches into it. There is no path dependency, npm link, runtime file lookup, submodule, or
build step that reads it, and CI must not assume it exists beside a checkout. Parity tests use
sanitized fixtures copied here instead. `tools/check-architecture.mjs` enforces this.

## Server core

Roughly 14,000 lines of C++ across `apps/server-core/src`.

| Component | Lines | Target | Slice |
| --- | ---: | --- | --- |
| `GDTFTemplates.h` | 3029 | `crates/shared/fixture` + a Media personality adapter | 8 |
| `WebServer.cpp` | 978 | `crates/media` HTTP/API adapter + `apps/media` bootstrap | 6 |
| `MediaResolver.cpp` | 864 | `crates/media` library adapter | 4 |
| `CITPResponder.cpp` | 855 | `crates/media` CITP server adapter | 8 |
| `TextSourceManager.cpp` | 749 | `crates/media` generated-source adapter | 7 |
| `AudioAnalyzer.cpp` | 438 | shared analysis contract + product-owned capture adapter | 7 |
| `MediaIngester.cpp` | 359 | `crates/media` library adapter (import job model) | 4 |
| `DmxMap.cpp` | 359 | `crates/media/domain` canonical personality | 1 |
| `Renderer.cpp`, `ofApp.cpp` | 439 | `crates/media` render adapter + `apps/media` composition root | 2 |
| `SacnReceiver.cpp` | 277 | `crates/media` sACN ingress adapter | 5 |
| `ArtNetReceiver.cpp` | 247 | `crates/media` Art-Net ingress adapter | 5 |
| `GDTFGenerator.cpp` | 194 | folded into the single canonical generator | 8 |
| `StateStore.h`, `Config.h` | 172 | `crates/media/domain` state + `media-application` configuration | 1, 2 |
| `visualizers/` (20 types) | ~2900 | `crates/media` generated-source adapter | 7 |

Two historical fixture-generation paths exist — `DmxMap`'s XML generation and `GDTFGenerator`.
The target has one canonical fixture definition; the duplicate is deleted rather than ported.

## Web UI

17 TypeScript/React sources under `apps/web-ui/src`: `App`, `api`, and the `Dashboard`,
`LayerCard`, `CompactView`, `MediaManager`, `TextSources`, `VisualizerManager`,
`VisualizerConfig`, `AudioSettings`, `Settings`, `Logs`, `Layout`, `StatusBadge` components plus
the `DmxMapPage` route. Feature behavior is preserved in `apps/media/src/`, composed from
`@tosklight/ui` rather than a competing local component library.

## Platform assumptions

| Assumption | Reference behavior | Target treatment |
| --- | --- | --- |
| FFT | `AudioAnalyzer.cpp` uses Apple Accelerate; it is the only platform-conditional source file | Platform-independent analysis with equivalent bands, smoothing, thresholds, timing, and numerical tolerances on all three systems |
| Frame rate | Fixed 60 fps requested at startup | Per-output render clock; `DisplaySynchronized` reads the surface's real capabilities |
| Target monitor | Stored in `media/.info` but never applied at window creation | Completed on macOS, Windows, and Linux |
| Art-Net addressing | Parser handles only the low universe byte | Complete Art-Net addressing, sequence behavior, source arbitration, and timeouts |
| Layer count | Renderer always held eight layers while `fullMode` changed how many DMX updated | Layer count and personality modelled explicitly |

## Licenses

The reference application's dependencies are recorded in its own `LICENSES.md`: openFrameworks
(MIT), cpp-httplib (MIT), Poco via ofxPoco (BSL-1.0), ofxNetwork (MIT), and for the web UI React,
React DOM, React Router DOM, clsx, slate and its packages, socket.io-client (MIT) with
lucide-react (ISC).

None of these are transferred. CMake, openFrameworks, and vendored artifacts are implementation
evidence only; every Rust dependency is selected here through architecture and license review.
Transferred code, tests, fixtures, and assets record their provenance and licensing in the
migration ledger.

## Deliberate exclusions

| Excluded | Reason |
| --- | --- |
| `apps/lighting-console` simulator | Not rebuilt. Only its packet captures, protocol fixtures, and interoperability scenarios become tests here. |
| `apps/artnet-test` | Ad-hoc reference harness; superseded by captured-packet tests. |
| `media/` operator data | Operator library and configuration never enter Git. Only sanitized, deterministic fixtures do. |
| `libs/`, `build/`, `bin/` | Vendored and generated reference build artifacts. |
| `legacy paused` field, layer blackout latch | Settled removals: pause is `playmode = Pause`, and there is no separate latch. |

## Open unknowns

Nothing on this item is undecided: all twenty recorded decision questions are answered and are
requirements. Remaining unknowns are implementation findings, and they are recorded per slice in
[the migration ledger](media-migration-ledger.md) rather than here.
