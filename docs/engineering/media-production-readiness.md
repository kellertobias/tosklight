# Media server production readiness

This record describes the September 2026 hardening pass for ToskLight Pixel. The operator
contracts remain in `docs/help/30-ToskLight-Media/`; the media server remains one standalone Rust
process with an embedded browser interface and separately rendered outputs.

## Ownership and control flow

- `apps/media/src/main.rs` enters `media-runtime::run`. Runtime owns configuration, ingress,
  the shared catalog, import workers, HTTP services, audio capture and output teardown.
- `apps/media/src/shared/api/` owns browser request identity and edit queues. Live control uses
  the existing socket path; persistent edits retain a request identity and send typed intent.
- `crates/media/adapters/http/src/routes/edit.rs` holds an edit lease from replay lookup through
  response publication. Configuration reads, persistence and publication share one transaction
  lock. Multipart transfers hold their request lease without blocking configuration editing.
- `crates/media/adapters/library/` owns numeric address reservations, import jobs, staging and
  filesystem edits. A job retains its reservation through catalog publication. A replacement
  preserves the old playable file until conversion succeeds.
- `crates/media/adapters/runtime/src/catalog_publication.rs` serializes import rescans with
  catalog edits. Unchanged assets keep their identity; replaced assets receive a fresh identity
  so playback reloads them. Revisions advance from the previous live snapshot.
- `crates/media/adapters/playback/` owns clip loading and per-layer sessions. Runtime uses bounded
  disk workers; cold loads and streamed frame reads do not run on the presentation thread.
- DMX, HTTP live-control and renderer reports update the shared MediaState with compare-and-swap
  retries. Reducers may retry; filesystem writes and external effects never occur in those retries.

## Correctness fixes

The pass addressed LAN HTTP request-ID failures, duplicate pixel-map IDs, debounced edits lost
on navigation, concurrent edit/replay races, lost DMX/control state, import name publication,
source/replacement collisions, cancellation reported as success, premature address release,
catalog identity/revision resets, stale selected sources after import/replacement, unreleased
clip pins, inaccessible oversized-clip streaming, and render-thread disk I/O.

Malformed clip indexes are checked against the real file length before allocation, and payload
ranges use checked arithmetic. Snappy's advertised decompression size is checked against the
frame geometry before allocation. The XML parser dependency was updated to quick-xml 0.41,
addressing [attribute parsing CPU exhaustion](https://github.com/tafia/quick-xml/issues/969) and
[namespace allocation exhaustion](https://github.com/tafia/quick-xml/issues/970).

The HTTP service's failure now stops the native output loop and propagates its error. The actual
HTTP lifecycle listens for SIGTERM and Ctrl-C and broadcasts shutdown; an unused signal helper
is insufficient. Startup errors distinguish an occupied HTTP port from permission/address errors.

## Verification commands

Initialize canonical artifact paths before direct Rust checks:

```sh
source tools/artifact-paths.sh
light_init_artifact_paths "$PWD"
```

Use a Bash shell for that initialization. It sets Cargo, temporary-file and runtime artifact
locations and preserves explicit caller overrides.

```sh
npm run test --workspace @tosklight/media -- --maxWorkers=2
npm run typecheck --workspace @tosklight/media
cargo test -p media-domain -p media-application -p media-http -p media-library \
  -p media-codec -p media-net -p media-citp -p media-pixel -p media-text \
  -p media-audio -p media-playback -p media-runtime
cargo test -p media-render
cargo test -p light-fixture -p light-mvr --lib
npm run build:media:open
```

The media-specific build/open command is the workflow documented in `apps/media/README.md`.
An isolated rehearsal should use a copy of the configuration and library under `LIGHT_TMP_DIR`,
unique loopback ports, and `MEDIA_CONFIG` pointing to that copy. Check the instance identity in
`/api/v2/health`, output source state and advancing preview sequences. HTTP availability alone
is not evidence of a rendered picture. Never stop another application merely because it owns
the configured port.

## Recorded results — 5 September 2026

- **981 media Rust tests passed:** 630 domain/application/HTTP/network/audio/text/pixel/CITP
  checks, 288 codec/import/playback/runtime checks, and 63 GPU/render checks.
- **208 browser-interface cases passed** across the complete run and focused retry. One existing
  LibraryPage test exceeded its local five-second timeout during concurrent native builds; its
  23-test file passed with the existing CI-equivalent twenty-second allowance. TypeScript passed.
- **180 shared fixture/MVR tests passed** after the XML parser dependency update.
- `cargo clippy -p 'media-*' --all-targets -- -D warnings`, media Cargo formatting and
  `git diff --check` passed. The media release executable and macOS application bundle built.
- Before committing, `cargo check --locked --offline -p media-server` also passed against an
  isolated export of the staged source, excluding the pre-existing audio/timecode/fixture
  changes. The broader test and runtime evidence above used the working checkout.
- The release executable was exercised against an isolated copy of the CITP fixture library,
  with a one-byte cache budget to force streaming. It rendered the fixture, imported a PNG into
  a selected missing slot, recovered automatically, and replaced the playing slot with a
  visibly different image without reselection. Unrelated catalog IDs remained stable.
- Concurrent audio/time configuration edits survived restart. SIGTERM exited successfully and
  stopped the output. Occupied HTTP-port checks failed promptly with an actionable message in
  both headless and native-desktop modes.

The local evidence is under `.artifacts/tmp/media-production-verification/`: `verification.json`,
`runtime.log`, `desktop-failure.log`, `streamed-preview.png`, `replacement-preview.png`, the
isolated verification scripts, and package test/build logs. These are generated artifacts.

Initial socket/GPU test failures were sandbox permission failures and passed after access was
granted. `node tools/check-architecture.mjs` still reports an already-stale test-bench migration
inventory and existing control-label warnings; the unrelated inventory was not regenerated.
The regular development configuration's port 8080 was already occupied by another Node process,
so the isolated runtime checks used port 18080 without changing the operator's configuration.

The updated local RustSec scan reports no quick-xml vulnerabilities. Its remaining vulnerability
is h2 0.4.15 in the wider workspace, absent from the tested macOS media-server dependency tree.
Linux GTK dependency maintenance/unsoundness advisories remain a platform-level review item;
this macOS pass does not certify Linux or Windows deployment.

## Operational boundary

This is the existing trusted-lighting-LAN service: it has no authentication of its own. Keep its
HTTP, Art-Net, sACN and CITP interfaces on the intended network. This pass does not introduce an
Internet-facing deployment or change persisted show/configuration schemas.

The shipped baseline remains one Main output. Actual display routing, projector/capture devices,
audio output, sustained show duration and multi-output GPU/storage capacity must be rehearsed on
the production machine. Kernel-blocked filesystem I/O cannot be portably interrupted by a Rust
worker; use reliable local storage for show-critical media.

The npm advisory query requires approval because npm receives private workspace dependency
names. Automatic approval review blocked that query in this pass. RustSec comparison runs locally
against the downloaded public advisory database; workspace-wide findings must be checked against
the media-server dependency tree instead of being attributed to Pixel automatically.
