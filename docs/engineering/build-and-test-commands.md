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
npm run dev                        # Light headless + Tauri app with UI hot reload
npm run storybook                  # shared operator components, no Light server
npm run storybook:build            # deterministic static Storybook artifact
npm run screenshots:marketing      # regenerate the marketing screenshot gallery
npm run open                       # incremental main-desk build, then open the app
npm run manual                     # PDF and HTML manuals from docs/help
npm run bundle [install]           # release artifacts for macOS, Windows, Linux
npm run clean                      # remove reproducible artifacts
npm run clean:cargo-incremental    # discard only Cargo's debug incremental objects
npm run artifact-path -- NAME            # resolve an artifact path

npm run test:architecture          # dependency direction + source size
npm run test:ui-package            # shared UI typecheck + component tests
npm run test:storybook             # static Storybook + serial Chrome gate
npm run test:unit                  # fast TS/Vitest + application Rust library tests
npm run test:verify                # architecture + frontend builds + full Rust workspace
npm run test:e2e-api               # Playwright @api, no browser
npm run test:e2e-ui                # Playwright @ui, real Chrome
npm run test:e2e -- [spec]            # everything, or one focused spec
npm run screenshots:help          # regenerate the whole help gallery (both captures)
npm run test:help-screenshots      # the Storybook half of the help gallery
npm run test:help-screenshots-live # the live-desk half of the help gallery
npm run test:all                   # unit then e2e

cargo run -p light-wire --example generate-contracts   # regenerate wire TS + schemas
cargo fmt                                              # never standalone rustfmt
```

## `npm run dev`

Hot-reload development loop. Starts `cargo run -p light-headless` in the foreground against the
artifact data directory and `assets/fixture-library`, waits for readiness, then runs the control-UI
Tauri dev server.

UI and Tauri changes hot-reload. **Rust changes require restarting `npm run dev`.** It traps EXIT/INT/TERM
so the headless process is killed with it.

Open `http://127.0.0.1:5000`. A new desk contains one enabled `Operator` user.

## `npm run` build scripts

| Command | What it does |
| --- | --- |
| `npm run open` | The authoritative desktop path. Checks runtime migration, stops running instances, reuses `node_modules` when its lockfile signature is current, reuses the control UI bundle when its source-content signature is current, builds `light-headless` and the main Tauri debug bundle, copies the headless binary into the app, submits it as launchd job `de.tokenet.tosklight.dev-server`, verifies readiness belongs to that PID, and opens the app. It no longer builds icon contact sheets or the separate Hardware Controls app. Set `LIGHT_FORCE_NPM_CI=1` or `LIGHT_FORCE_FRONTEND_BUILD=1` to force the corresponding clean step. |
| `npm run storybook` | Serves the tracked `@tosklight/ui` package and its deterministic mock stories at `http://127.0.0.1:6006`, without a Light server or mutable show. |
| `npm run storybook:build` | Builds the static review artifact under `.artifacts/build/storybook/ui`. |
| `npm run screenshots:marketing` | Builds static Storybook and recreates every manifest-owned marketing PNG under `docs/marketing/assets/screenshots`; CI publishes this directory as the `marketing-screenshots` artifact consumed by the Pages build. |
| `npm run manual` | Auto-provisions a pinned Python venv at `.artifacts/cache/manual-venv`, then builds and verifies the PDF and the HTML manual. See the [manual authoring guide](../help/99-Development/04-manual-and-help-screenshots.md). |
| `npm run bundle` | Cross-platform release. macOS universal binary via `lipo`, plus Windows `x86_64-pc-windows-gnu` and Linux `x86_64`/`aarch64-unknown-linux-musl` via `cargo zigbuild`. Release Tauri bundles for both apps; each server zipped with `assets/fixture-library`. Requires `cargo, npm, ditto, zip, lipo, rustup, cargo-zigbuild, zig`. |
| `npm run bundle:install` | The above, then install into `~/Applications` and open. |
| `npm run migrate-artifacts` | Explicitly moves legacy `./light-data` into `.artifacts/runtime/light-data`. Never implicit; stops without merging if both exist. |
| `npm run clean` | Removes reproducible artifacts, preserving the active development runtime. |
| `npm run clean:cargo-incremental` | Removes only `.artifacts/build/cargo/debug/incremental`. Use this when old feature/build variants have made filesystem scans slow; ordinary compiled dependencies remain cached. |
| `npm run clean -- runtime PATH` | Removes runtime data. Deliberately separate and requires the exact absolute path, because it includes local shows and desk state. |
| `npm run artifact-path -- NAME` | Prints a resolved path: `root, cargo, manual-pdf, manual-html, release, runtime, test-results, playwright-report, visual-inspection`. |

After `npm run open`:

```sh
curl -fsS http://127.0.0.1:5000/api/v2/readiness
```

Check `.artifacts/runtime/light-data/light-headless.log` first for app-owned startup problems. If
readiness is healthy but the app looks stuck, time `/api/v2/readiness` and `/api/v2/bootstrap`
separately.

**If the app looks stale, verify which bundle the build script actually opened before reworking UI
code.**

## `npm run test` scripts

| Command | What it runs |
| --- | --- |
| `npm run test:architecture` | `tools/check-architecture.mjs`, the source-size unit tests, and `tools/check-source-size.mjs`. See [below](#what-test-architecture-actually-checks). |
| `npm run test:ui-package` | Typechecks `@tosklight/ui` and runs its focused Vitest component suite. |
| `npm run test:storybook` | Builds static Storybook, serves it locally, and enumerates every story in serial real Chrome. It rejects blank or unstable stories, browser errors, REST/WebSocket dependencies, invalid modal stacking, and invalid 24×18 desktop geometry. |
| `npm run test:unit` | Fast local loop: root bench type/unit tests, shared UI and patch-package tests, Viz editor tests, both frontend Vitest suites, and the broad `light-application` Rust library test binary. It skips architecture scans, frontend production builds, the many per-crate/integration test processes, and the Viz Rust workspace. |
| `npm run test:verify` | Comprehensive pre-merge gate: architecture and artifact checks, all fast TypeScript tests, both frontend production builds, and the full non-Tauri Rust workspace test suite. Release CI runs this command. |
| `npm run test:bench-unit` | Root Vitest coverage for the reusable helpers under `tests/bench`. |
| `npm run test:e2e -- [args]` | Builds the UI and server, then Playwright with the root config. |
| `npm run test:e2e-api` | Playwright `--grep '@api'`. API-only contracts and constructed failure, persistence, concurrency, and wire conditions that cannot be driven truthfully through UI. |
| `npm run test:e2e-ui` | Playwright `--grep '@ui' --grep-invert '@(demo\|docs)\b'`. Real Chrome operator workflows, including OSC and attached-hardware surfaces. Generated visual documentation runs separately. |
| `npm run test:help-screenshots` | Builds static Storybook and serially checks every entry in `docs/help/screenshot-manifest.json`. Story-owned captures require stable IDs/dimensions, no blank output, no browser errors or live REST/WebSocket requests, and no unreviewed pixel diff. It does not launch Light. |
| `npm run screenshots:help` | Regenerates the whole help gallery: the Storybook capture and the live-desk capture. A fresh clone has no help images until this runs. |
| `npm run test:help-screenshots-live` | Smaller, separately named production browser/server path for manifest entries still marked `live-app`; Storybook-owned captures cannot be overwritten by this command. |
| `npm run test:record` | Serial narrated video of the whole catalog, assembled with ffmpeg into `.artifacts/test/visual-inspection/`. |
| `npm run test:demo-show` | Fast API-only generation and validation of `assets/demo.show`; release CI uses this path. |
| `npm run test:demo` | Offline product walkthrough and video capture on the operator computer; also refreshes `assets/demo.show`. |
| `npm run test:app-icons` | Asserts the required Tauri icon set for both apps. |
| `npm run test:artifact-paths` | Self-test of the artifact path bindings across bash, Node, and Python. |
| `npm run test:all` | `verify` then `e2e`. |

Repository build and test scripts serialize Cargo access through a shared lock under
`.artifacts/tmp`. If another command owns Cargo, the waiting command prints the owner PID, label,
and start time instead of appearing frozen behind Cargo's internal lock. Rust dev- and test-profile
incremental objects are disabled because the repository's many feature combinations can grow a
filesystem-heavy cache that makes macOS compiler processes sleep for minutes. Cargo's normal
whole-crate fingerprints and dependency outputs remain cached, so unchanged crates remain instant.

Test layering:

| Layer | Where | Runner |
| --- | --- | --- |
| Rust unit/integration | each crate's `tests/` or feature-local modules | cargo |
| TS unit/component | `apps/ui-library/src/**/*.test.ts(x)`, `apps/light-desktop/src/**/*.test.ts(x)`, and `tests/bench/**/*.test.ts` | vitest |
| Type/build gate | `tsc --noEmit && vite build` | tsc/vite |
| Deterministic component browser | `apps/ui-library/src/**/*.stories.tsx` and `apps/light-desktop/src/**/*.stories.tsx`, asserted by `apps/ui-library/storybook/tests/` | Storybook + Playwright |
| Acceptance | root `tests/`, using the bench in `tests/bench/` | Playwright |

`apps/ui-library` is the authoritative source for the `@tosklight/ui` presentation primitives. Compatibility
modules at the former Control UI paths may re-export package components or retain thin
application-owned adapters while migration is in progress. The supported package contract is:

```sh
npm run test:ui-package  # component types and behavior
npm run test:storybook   # deterministic real-browser presentation and interaction
npm run test:unit        # fast package/application library loop
npm run test:verify      # comprehensive package/application verification
npm run test:e2e-ui      # real application composition and operator workflows
```

The static Storybook output is reproducible and ignored; tracked package source, stories, and tests
remain the source of truth. During refactoring plan 02, Storybook is a review surface first. It does
not become the documentation-screenshot source until the review gate is accepted and the tracked
screenshot manifest is implemented.

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
   - `light-application` must not depend on wire, headless runtime, runnable apps, or UI crates.
   - Crates under `crates/light/domain` and `crates/shared` must not depend outward on application,
     wire, headless runtime, or runnable app crates.
   - `light-headless-runtime` composes `light-application` and `light-wire`.
   - `light-headless` depends on the runtime adapter and remains the runnable composition root.
2. **Thin headless entry point** — `apps/light-headless/src/main.rs` is at most 10 non-empty lines, must
   contain `light_headless_runtime::run().await`, and must not mention `Router`, `AppState`, `TcpListener`, or
   `tokio::spawn`.
3. **Active-show mutation direction** — `crates/light/adapters/headless/src/runtime/update_plans.rs` must not contain
   `.put_object(`, `refresh_command_show`, or `load_engine_snapshot`. Writes route through
   `ActiveShowService`; a router never writes SQLite.
4. **Closed Playback ownership** — no `pub fn playback(` in `crates/light/domain/engine/src`, no
   `pub fn operation_lock(` in `crates/light/src/playback`, and no `engine.playback()` or
   `playback_action_lock` anywhere in `crates/light/src` or `crates/light/adapters/headless/src` (tests
   excluded). Callers use typed commands and immutable projections.
5. **TypeScript dependency directions** —
   - `apps/light-desktop/src/api/generated/light-wire.ts` must exist, must start with the generated
     header, and must contain no local imports.
   - Only files under `apps/light-desktop/src/api/` may import it. A component importing wire DTOs
     fails with *"imports wire DTOs directly; map them at the API boundary"*.
   - At least one consumer must exist.
   - Nothing under `src/api/` may import from `src/components/` or `src/windows/`.

### `tools/check-source-size.mjs`

Limits from `tools/source-size/config.mjs`:

| | Hard limit | Design goal |
| --- | --- | --- |
| File | 1200 lines | 400 lines |
| Function | 150 lines | 20 lines |

Files are enumerated with `git ls-files --cached --others --exclude-standard`, then restricted to
`.ts`, `.tsx`, `.js`, `.rs`, and `.py` files under `apps/`, `crates/`, and `packages/`. Any nested
`assets/`, `artifacts/`, `docs/`, or `experiments/` directory is excluded. JSON, Markdown, shell,
repository-level tests, and all other paths and extensions are outside this check. Test sources
within the included roots are exempt from the hard limits but still reported against the goals.

The **ratchet** (`tools/source-size/baseline.json`) is currently empty: zero legacy violations
remain, so any new oversized file or function fails immediately. After genuinely reducing a
violation, tighten the baseline with:

```sh
node tools/check-source-size.mjs --ratchet
node tools/check-source-size.mjs --print-baseline
```

Split by responsibility, abstraction level, ownership, and test boundary — not to satisfy a number.

### Related contract check

Not part of `npm run test:architecture`, but the same family: `crates/light/contracts/wire/tests/generated_contracts.rs`
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
| Rust domain or application logic | `cargo test -p <crate>`, then `npm run test:unit`; use `npm run test:verify` before merging |
| Wire DTOs | regenerate contracts, then `npm run test:verify` |
| Frontend logic | `npm test` in `apps/light-desktop`, then `npm run test:unit`; use `npm run test:verify` before merging |
| Operator-visible behaviour, including OSC and attached hardware | `npm run test:e2e-ui`, or `npm run test:e2e -- tests/<spec>.spec.ts` |
| API-only failure construction, restart, migration, or wire behaviour | `npm run test:e2e-api` |
| Desktop lifecycle, native windows, server supervision | Focused Rust/Tauri tests locally; the GitHub Actions release build probes the newly built desktop process on macOS, Linux, and Windows |
| `docs/help/` content | `npm run dev` to check live help, then `npm run manual` |
| Storybook-owned panes or help images | `npm run screenshots:help` (CI regenerates them on every run regardless) |
| Manifest entries still marked live-app | `npm run test:help-screenshots-live`, then review only those image diffs |
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
.artifacts/build/storybook/ui/    static shared-UI review artifact
.artifacts/cache/manual-venv      manual generator venv
.artifacts/generated/manual/      PDF, HTML site, deployable ZIP
.artifacts/release/               release binaries and app zips
.artifacts/runtime/light-data/    local desk data + light-headless.log
.artifacts/test/results/          Playwright output
.artifacts/test/playwright-report/
.artifacts/test/visual-inspection/
```

Override the root with `LIGHT_ARTIFACTS_DIR`, or the data directory with `LIGHT_DATA_DIR`. Resolve
any path for a script with `npm run artifact-path -- NAME` (`storybook` resolves the static UI
artifact).

## CI

`.github/workflows/release.yml`:

| Job | Runner | Runs |
| --- | --- | --- |
| `unit` | Ubuntu | `npm run test:unit` |
| `e2e` | Ubuntu, sharded over API exceptions and UI/OSC coverage | `npm run test:e2e-*`, uploading `.artifacts/test/results` |
| `build` | macOS, Linux, Windows | Builds release artifacts, then launches each newly built desktop application for five seconds and fails if it exits early |

The launch probe is intentionally CI-only. Local `npm run open` and
`npm run bundle` behavior is unchanged and does not automatically launch an
application merely to validate a build.

`.github/workflows/media.yml` is the Media Server's quality gate: `cargo fmt --check`,
`cargo clippy -- -D warnings`, the tests, and a packaged `--check-configuration` smoke test, over
every Media package on macOS, Windows, and Linux x86_64 and aarch64.

`.github/workflows/media-release.yml` packages it. Media does **not** ship inside `release.yml`:
it remains a separate product and the workflow runs as a follow-up (`workflow_run` on the release
pipeline, `completed`, and only when that run concluded successfully on `main`). Every actual
ToskLight release builds all four Media targets and attaches them to that version, so the static
Pages download matrix never points at a current-version Media asset that was omitted merely because
Media itself had not changed. A successful pipeline run that published no release still does
nothing.

The four archives are named `tosklight-media-<version>-<slug>.zip` and are attached, with their own
`MEDIA-SHA256SUMS`, to the **same** GitHub release the pipeline just published — never a second
release. `workflow_dispatch` builds the archives without publishing them, for checking that
packaging still works.
