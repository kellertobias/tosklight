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
| Slice 1 — pure domain | `StateStore.h`, `DmxMap.*`, `DmxConstants.h` | — | — | — | — | — |
| Slice 2 — renderer and one output | `Renderer.*`, `ofApp.*` | — | — | — | — | — |
| Slice 3 — video playback | video/image playback logic | — | — | — | — | — |
| Slice 4 — catalog and ingestion | `MediaResolver.*`, `MediaIngester.*` | — | — | — | — | — |
| Slice 5 — Art-Net and sACN | `ArtNetReceiver.*`, `SacnReceiver.*` | — | — | — | — | — |
| Slice 6 — HTTP API and React UI | `WebServer.*`, `apps/web-ui` | — | — | — | — | — |
| Slice 7 — generated sources and compositing | `TextSourceManager.*`, `AudioAnalyzer.*`, `visualizers/` | — | — | — | — | — |
| Slice 8 — CITP server and GDTF | `CITPResponder.*`, `GDTFGenerator.*`, `GDTFTemplates.h` | — | — | — | — | — |
| Phase 4 — integration and cutover | — | — | — | — | — | — |

## Acceptance rule

Passing the reference application's checks proves only the reference application. Passing focused
Media checks proves only that slice. Accepting a slice also requires the affected full-repository
checks, so a shared-crate change cannot regress the desk.
