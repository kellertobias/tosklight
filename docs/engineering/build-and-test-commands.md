# Build and Test Commands

Every supported workflow runs through the root `package.json` scripts (`npm run …`), which are
backed by `tools/dev.sh`, `tools/build.sh`, and `tools/test.sh`. Prefer them over calling `cargo`,
`npm`, or `playwright` directly — they resolve artifact paths, generate Tauri configs, and compose
the steps in the right order. Run `npm run` to list every script.

- [Quick reference](#quick-reference)
- [`npm run dev`](#dev)
- [`npm run` build scripts](#build)
- [`npm run test` scripts](#test)
- [What `npm run test:architecture` actually checks](#what-test-architecture-actually-checks)
- [Verification ladder](#verification-ladder)
- [Other tools](#other-tools)
- [Artifact paths](#artifact-paths)
- [CI](#ci)

## Quick reference

```sh
npm run dev                        # server + Tauri app with UI hot reload
npm run open                 # debug builds, stop old instances, open the app
npm run manual               # PDF and HTML manuals from docs/help
npm run bundle [install]    # release artifacts for macOS, Windows, Linux
npm run clean                # remove reproducible artifacts
npm run artifact-path -- NAME            # resolve an artifact path

npm run test:architecture          # dependency direction + source size
npm run test:unit                  # architecture + tsc/vite + cargo + vitest
npm run test:e2e-api               # Playwright @api, no browser
npm run test:e2e-ui                # Playwright @ui, real Chrome
npm run test:e2e -- [spec]            # everything, or one focused spec
npm run test:desktop-smoke         # packaged .app process integration (macOS)
npm run test:help-screenshots      # regenerate help images — only when intentional
npm run test:all                   # unit then e2e

cargo run -p light-wire --example generate-contracts   # regenerate wire TS + schemas
cargo fmt                                              # never standalone rustfmt
```

## `npm run dev`

Hot-reload development loop. Starts `cargo run -p light-server` in the foreground against the
artifact data directory and `assets/fixture-library`, waits for readiness, then runs the control-UI
Tauri dev server.

UI and Tauri changes hot-reload. **Rust changes require restarting `npm run dev`.** It traps EXIT/INT/TERM
so the server is killed with it.

Open `http://127.0.0.1:5000`. A new desk contains one enabled `Operator` user.

## `npm run` build scripts

| Command | What it does |
| --- | --- |
| `npm run open` | The authoritative desktop path. Checks runtime migration, stops running instances (launchd + `light-server`/`ToskLight`/vite), writes Tauri configs, `npm ci` in both apps, builds the control UI, builds `light-server`, builds both Tauri debug bundles, copies the server binary into `ToskLight.app/Contents/MacOS/light-server`, submits the server as launchd job `de.tokenet.tosklight.dev-server`, waits for readiness, **verifies the launchd PID owns that readiness**, and opens the app. |
| `npm run manual` | Auto-provisions a pinned Python venv at `.artifacts/cache/manual-venv`, then builds and verifies the PDF and the HTML manual. See the [manual authoring guide](../help/99-Development/04-manual-and-help-screenshots.md). |
| `npm run bundle` | Cross-platform release. macOS universal binary via `lipo`, plus Windows `x86_64-pc-windows-gnu` and Linux `x86_64`/`aarch64-unknown-linux-musl` via `cargo zigbuild`. Release Tauri bundles for both apps; each server zipped with `assets/fixture-library`. Requires `cargo, npm, ditto, zip, lipo, rustup, cargo-zigbuild, zig`. |
| `npm run bundle:install` | The above, then install into `~/Applications` and open. |
| `npm run migrate-artifacts` | Explicitly moves legacy `./light-data` into `.artifacts/runtime/light-data`. Never implicit; stops without merging if both exist. |
| `npm run clean` | Removes reproducible artifacts, preserving the active development runtime. |
| `npm run clean -- runtime PATH` | Removes runtime data. Deliberately separate and requires the exact absolute path, because it includes local shows and desk state. |
| `npm run artifact-path -- NAME` | Prints a resolved path: `root, cargo, manual-pdf, manual-html, release, runtime, test-results, playwright-report, visual-inspection`. |

After `npm run open`:

```sh
curl -fsS http://127.0.0.1:5000/api/v1/readiness
```

Check `.artifacts/runtime/light-data/light-server.log` first for app-owned startup problems. If
readiness is healthy but the app looks stuck, time `/api/v1/readiness`, `/api/v1/health`, and
`/api/v1/bootstrap` separately.

**If the app looks stale, verify which bundle the build script actually opened before reworking UI
code.**

## `npm run test` scripts

| Command | What it runs |
| --- | --- |
| `npm run test:architecture` | `tools/check-architecture.mjs`, the source-size unit tests, and `tools/check-source-size.mjs`. See [below](#what-test-architecture-actually-checks). |
| `npm run test:unit` | `architecture` → control-UI `npm run build` (`tsc --noEmit && vite build`) → `cargo test --workspace --exclude light-control-ui --exclude light-hardware-controls --no-default-features` → `npm test` (vitest). |
| `npm run test:e2e -- [args]` | Builds the UI and server, then Playwright with the root config. |
| `npm run test:e2e-api` | Playwright `--grep '@api'`. Process-level, no browser. |
| `npm run test:e2e-ui` | Playwright `--grep '@ui'`. Real Chrome. |
| `npm run test:e2e-supplemental` | `--grep-invert '@api\|@ui'` — the `@osc`, `@wire`, `@restart`, `@desktop`, `@bench` tags. |
| `npm run test:desktop-smoke` | macOS only. Builds the Tauri debug bundle, copies the server binary in, runs `tests/05-desktop-process-integration.spec.ts` with `LIGHT_DESKTOP_SMOKE=1`. |
| `npm run test:help-screenshots` | **Wipes and regenerates** `docs/help/assets/screenshots/`. Only run when intentionally refreshing images, and review the diffs visually. |
| `npm run test:record` | Serial narrated video of the whole catalog, assembled with ffmpeg into `.artifacts/test/visual-inspection/`. |
| `npm run test:demo` | The product walkthrough; refreshes `assets/demo.show`. |
| `npm run test:app-icons` | Asserts the required Tauri icon set for both apps. |
| `npm run test:artifact-paths` | Self-test of the artifact path bindings across bash, Node, and Python. |
| `npm run test:all` | `unit` then `e2e`. |

Test layering:

| Layer | Where | Runner |
| --- | --- | --- |
| Rust unit/integration | each crate's `tests/` or feature-local modules | cargo |
| TS unit/component | `apps/control-ui/src/**/*.test.ts(x)` (jsdom + Testing Library) | vitest |
| Type/build gate | `tsc --noEmit && vite build` | tsc/vite |
| Acceptance | root `tests/`, using the bench in `apps/control-ui/e2e/bench/` | Playwright |

Acceptance tests act through the same public surfaces an operator uses — visible UI, exact OSC, the
command-line HTTP API, or explicit deterministic bench controls. `pairedScenario(...)` registers an
`@api` and a `@ui` test with the same arrangement and the same assert oracle, which is how surface
parity is proven rather than assumed. See [test map](test-map.md) and `docs/testing/README.md`.

## What `npm run test:architecture` actually checks

It is the machine-enforced half of this repository's architecture rules. Convention is not relied
on: if a boundary matters, it is checked here.

### `tools/check-architecture.mjs`

Five checks. Every failure is collected and reported as `architecture error: …` with exit code 1.

1. **Rust dependency directions** — parsed from `cargo metadata`.
   - `light-wire` may depend on **no** workspace crate.
   - `light-application` must not depend on `light-wire`, `light-server`, or either UI crate.
   - Any other `crates/*` except `light-server` must not depend on `light-application`,
     `light-wire`, or `light-server`.
   - `light-server` **must** depend on both `light-application` and `light-wire` — it is the
     composition root.
2. **Thin server entry point** — `crates/server/src/main.rs` is at most 10 non-empty lines, must
   contain `light_server::run().await`, and must not mention `Router`, `AppState`, `TcpListener`, or
   `tokio::spawn`.
3. **Active-show mutation direction** — `crates/server/src/runtime/update_plans.rs` must not contain
   `.put_object(`, `refresh_command_show`, or `load_engine_snapshot`. Writes route through
   `ActiveShowService`; a router never writes SQLite.
4. **Closed Playback ownership** — no `pub fn playback(` in `crates/engine/src`, no
   `pub fn operation_lock(` in `crates/application/src/playback`, and no `engine.playback()` or
   `playback_action_lock` anywhere in `crates/application/src` or `crates/server/src` (tests
   excluded). Callers use typed commands and immutable projections.
5. **TypeScript dependency directions** —
   - `apps/control-ui/src/api/generated/light-wire.ts` must exist, must start with the generated
     header, and must contain no local imports.
   - Only files under `apps/control-ui/src/api/` may import it. A component importing wire DTOs
     fails with *"imports wire DTOs directly; map them at the API boundary"*.
   - At least one consumer must exist.
   - Nothing under `src/api/` may import from `src/components/` or `src/windows/`.

### `tools/check-source-size.mjs`

Limits from `tools/source-size/config.mjs`:

| | Hard limit | Design goal |
| --- | --- | --- |
| File | 1200 lines | 400 lines |
| Function | 150 lines | 20 lines |

Files are enumerated with `git ls-files --cached --others --exclude-standard`. Per-language function
scanners exist for Rust, JavaScript/TypeScript, Python, and shell.

**Exemptions are deliberately narrow** — machine-managed lockfiles, Tauri `gen/schemas/*.json`,
generated wire schemas, and standalone prototypes under `experiments/`. Test sources are exempt from
the hard limits but still reported against the goals.

The **ratchet** (`tools/source-size/baseline.json`) is currently empty: zero legacy violations
remain, so any new oversized file or function fails immediately. After genuinely reducing a
violation, tighten the baseline with:

```sh
node tools/check-source-size.mjs --ratchet
node tools/check-source-size.mjs --print-baseline
```

Split by responsibility, abstraction level, ownership, and test boundary — not to satisfy a number.

### Related contract check

Not part of `npm run test:architecture`, but the same family: `crates/wire/tests/generated_contracts.rs`
re-renders every generated artifact in memory and asserts byte equality with the checked-in files,
so a stale `light-wire.ts` fails `cargo test` (and therefore `npm run test:unit`). Regenerate with:

```sh
cargo run -p light-wire --example generate-contracts
```

## Verification ladder

Start with the smallest relevant check, then widen by risk.

| You changed | Run |
| --- | --- |
| Module boundaries, crate deps, file sizes | `npm run test:architecture` |
| Rust domain or application logic | `cargo test -p <crate>`, then `npm run test:unit` |
| Wire DTOs | regenerate contracts, then `npm run test:unit` |
| Frontend logic | `npm test` in `apps/control-ui`, then `npm run test:unit` |
| Operator-visible behaviour | `npm run test:e2e-api` and `npm run test:e2e-ui`, or `npm run test:e2e -- tests/<spec>.spec.ts` |
| OSC, restart, or wire behaviour | `npm run test:e2e-supplemental` |
| Desktop lifecycle, native windows, server supervision | `npm run test:desktop-smoke` |
| `docs/help/` content | `npm run dev` to check live help, then `npm run manual` |
| Panes, or anything the help images show | `npm run test:help-screenshots`, then review diffs visually |
| Real operator behaviour, before handoff | `npm run open` |

Use `cargo fmt` for Rust formatting. Do not run standalone `rustfmt` against workspace files.

## Other tools

| Command | Purpose |
| --- | --- |
| `npx @tobisk/codesafari dev` | Serve the onboarding tour in `.tour/` at `http://localhost:4317` |
| `npx @tobisk/codesafari validate` | Check tour content for bad frontmatter, dangling links, unresolved steps |
| `node tools/check-source-size.mjs --ratchet` | Tighten the size baseline after reductions |
| `cargo run -p light-wire --example generate-contracts` | Regenerate wire TypeScript and JSON schemas |

## Artifact paths

Nothing hardcodes `target/` or `light-data/`. `tools/artifact-layout.conf` is the single source of
truth, with bindings for bash (`tools/artifact-paths.sh`), Node (`tools/artifact-paths.cjs`/`.mjs`),
and Python (`tools/artifact_paths.py`).

Everything reproducible lives under ignored `.artifacts/`:

```
.artifacts/build/cargo            CARGO_TARGET_DIR
.artifacts/cache/manual-venv      manual generator venv
.artifacts/generated/manual/      PDF, HTML site, deployable ZIP
.artifacts/release/               release binaries and app zips
.artifacts/runtime/light-data/    local desk data + light-server.log
.artifacts/test/results/          Playwright output
.artifacts/test/playwright-report/
.artifacts/test/visual-inspection/
```

Override the root with `LIGHT_ARTIFACTS_DIR`, or the data directory with `LIGHT_DATA_DIR`. Resolve
any path for a script with `npm run artifact-path -- NAME`.

## CI

`.github/workflows/test.yml`:

| Job | Runner | Runs |
| --- | --- | --- |
| `unit` | ubuntu | `npm run test:unit` |
| `e2e` | ubuntu, sharded matrix over `api`/`ui`/`supplemental` | `npm run test:e2e-*`, uploading `.artifacts/test/results` on failure |
| `desktop-smoke` | macos-14 | `npm run test:desktop-smoke` |

Manual and release CI runs on Forgejo (`.forgejo/workflows/manual.yml`): the manual builds on PR and
main, and on `v*` tags the PDF and HTML archive are attached to the release. PR builds never receive
credentials.
