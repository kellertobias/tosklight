# Test Map

Choose the smallest test that observes the boundary you changed, then widen in proportion to risk.
Passing a private reducer test is not proof of an OSC path, persisted show, rendered frame, visible
gesture, or packaged desktop lifecycle.

The historical pre-migration inventory is [Refactoring test-boundary inventory](refactoring-test-boundaries.md).
This document is the current routing map. For what each command runs — and what
`./test architecture` enforces — see [Build and test commands](build-and-test-commands.md).

## Sources of truth

- `docs/help/` defines operator terminology and behavior.
- `docs/testing/` defines human-readable acceptance scenarios and stable IDs.
- `tests/` implements process-level Playwright acceptance coverage.
- Feature-local Rust and Vitest tests prove pure rules, application services, adapters, stores, and
  presentation details without replacing the public acceptance path.

One test may deliberately cross UI, HTTP, OSC, persistence, and output. Preserve the named surface:
an API helper does not prove a visible software gesture, and a software click does not prove the
exact OSC address and feedback sequence.

## Change-to-test routing

| Change | First evidence | Widen when |
| --- | --- | --- |
| Rust value, parser, reducer, codec, arbitration | Adjacent `#[cfg(test)]` module or the owning crate's `tests/`; `cargo test -p <crate> --no-default-features <filter>` | The value crosses a service, transport, persistence, or output boundary |
| Application command/service/event | Feature tests under `crates/application/src/<feature>/tests*`; `cargo test -p light-application --no-default-features <filter>` | A server adapter, event subscriber, active-show transaction, or automatic source invokes it |
| Portable-show transaction or migration | `crates/show/src/portable/` tests plus compiler/service tests in `crates/application/src/show_compiler/` or the owning capability | Old files, unknown fields, Save As, recovery, runtime replacement, or concurrent revisions are affected |
| Desk schema/session recovery | `crates/show/src/desk/` tests and focused server startup/session tests | Reconnect, restart, user/client deletion, screens, or active-show selection are affected |
| Wire DTO/schema/generation | `cargo test -p light-wire --no-default-features`; generated-artifact test | Any server or frontend mapping changes |
| Server router/auth/adapter | Focused module under `crates/server/src/runtime/tests/`; `cargo test -p light-server --lib --no-default-features <filter>` | The contract needs a real socket, process lifecycle, OSC, or cross-surface proof |
| Frontend decoder/transport | Co-located tests in `apps/control-ui/src/api/`; `(cd apps/control-ui && npm test -- <file>)` | Reconnect, malformed events, authentication, or browser network behavior changes |
| Frontend store/session/hook | Co-located tests under `apps/control-ui/src/features/`; assert revision ordering, wrong-scope rejection, optimistic rollback, subscription lifetime, and gap repair | A production pane consumes the projection or visible latency/rerender behavior changes |
| Component wording, state, or gesture | React Testing Library/Vitest next to the component | Geometry, focus, pointer/touch behavior, software/hardware layout, or end-to-end intent is part of acceptance |
| OSC or attached hardware | Exact OSC Playwright scenario plus `apps/hardware-controls` reducer/surface tests | Server feedback, aliases, press/release ordering, desk sharing, or native UDP lifecycle changes |
| Playback/Programmer/render semantics | Owning domain/application tests and a deterministic bench scenario observing authoritative runtime and output | Current-page/explicit-page, timing, Preload, Highlight, HTP/LTP, Cue tracking, or automatic advance changes |
| Art-Net/sACN/output scheduler | `crates/output/tests/`, engine tests, and focused receiver scenarios under `tests/` | Socket delivery, shutdown, first frame, overload health, or capacity changes |
| Files/MVR/media | Owning unit/integration tests and focused HTTP/UI acceptance scenario | Confinement, archive portability, CITP/socket behavior, long-running feedback, or desktop picker behavior changes |
| Tauri/window/process lifecycle | Browser-adapter unit tests first, then `./test desktop-smoke` | Native launch, child-server ownership, additional screens, shutdown, bundle assets, or stale-bundle behavior changes |
| Manual/help | Markdown review and `./build manual`; screenshot workflow only when intentionally refreshing images | Operator-facing layout or the documented UI changed |
| Dependency/module boundary | `./test architecture` | Always before committing a structural slice |

## Rust test locations

Pure domain rules stay close to the module. Cross-module public boundaries belong in crate-level
integration tests where available:

- `crates/engine/tests/` covers automatic transition publication through the engine boundary;
- `crates/playback/tests/` covers automatic runtime behavior;
- `crates/output/tests/` covers codecs, routes, network behavior, and scheduler characterization;
- `crates/wire/tests/generated_contracts.rs` rejects stale generated files; and
- `crates/server/src/runtime/tests/` is split by router, service adapter, migration, session, OSC,
  output, and operational flow while the server library remains one crate.

Do not assert correctness by acquiring a service's private mutex or registry. Use a command,
immutable query projection, event, wire response, captured protocol packet, or persisted artifact.

Useful focused commands are:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p light-application --no-default-features
cargo test -p light-server --lib --no-default-features runtime::event_transport::tests
cargo test -p light-wire --no-default-features
```

Rust test filters match test paths, not source filenames exactly; use `cargo test -p <crate> --
--list` when discovering the narrow filter.

## Frontend and Hardware Controls tests

Control-UI tests are co-located with API decoders, feature stores/sessions, controllers, models,
components, windows, and desktop adapters. Run from the package directory:

```sh
cd apps/control-ui
npm run typecheck
npm test -- src/features/showObjects/ShowObjectsSession.test.ts
npm test
npm run build
```

Hardware Controls has an independent package and native host. Its reducer, OSC paths, controller
settings, and focused surfaces must be tested separately:

```sh
cd apps/hardware-controls
npm run typecheck
npm test
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
```

Avoid snapshotting one global React context as the default oracle. A narrow feature-store test
should prove that unrelated updates preserve selector identity and do not notify the consumer.

## Playwright bench and intent helpers

`apps/control-ui/e2e/bench/` provides an isolated server, authenticated API driver, real browser,
deterministic clock, Art-Net/sACN receivers, OSC hardware simulator, process restart/fault controls,
and failure artifacts. `pairedScenario` registers independent `@api` and `@ui` cases with one
stable scenario ID and shared final oracle; it does not disguise one surface as the other.

Use the intent helpers in `tests/support/operator/` before adding low-level repetition:

- `doProgrammerStep` presses logical keys through an explicitly named `command-line`, `software`,
  or `osc` surface.
- `executeProgrammerCommand` expresses readable command intent while retaining that surface.
- `withOscProgrammer` owns OSC subscription and cleanup.
- `storeGroup` names pool versus Programmer storage and requires an explicit mode when needed.
- Patch helpers name visible software actions separately from the typed v2 Patch API.

Add a helper when several scenarios repeat infrastructure, not to hide behavior that the scenario
must distinguish. A helper should accept the semantic input and explicit surface, own cleanup, and
leave the assertion at the strongest public boundary. Do not create a universal “do action” helper
that silently picks API instead of software or OSC.

Focused Playwright commands from the repository root are:

```sh
./test e2e tests/04-osc-api-and-cross-surface.spec.ts --grep 'OSC-002'
./test e2e-api tests/01-foundational-dimmers-and-groups.spec.ts --grep 'GROUP-004'
./test e2e-ui tests/01-foundational-dimmers-and-groups.spec.ts --grep 'PROG-001'
./test e2e-supplemental tests/05-virtual-time-persistence-and-recovery.spec.ts
```

Use `cd apps/control-ui && npm run test:e2e -- --list` to confirm test discovery after moving or
splitting a specification.

## Network, desktop, and benchmark evidence

Loopback HTTP/WebSocket, OSC, Art-Net, sACN, CITP, and UDP tests require an environment that permits
local socket binds. A sandbox `EPERM` on `listen` is an environment limitation, not a passing or
failing application assertion; rerun the same command unrestricted and report both facts.

For native behavior:

```sh
./test desktop-smoke
./build open
curl -fsS http://127.0.0.1:5000/api/v1/readiness
```

After `./build open`, inspect `.artifacts/runtime/light-data/light-server.log`. If readiness is
healthy but the UI appears stuck, time `/api/v1/readiness`, `/api/v1/health`, and
`/api/v1/bootstrap` separately and confirm the bundle opened by `build` before changing UI code.

The release-only render-through-encoding benchmark is:

```sh
cargo run --release -p light-server --bin light-benchmark -- \
  --profile all --protocol both --transport encode-only \
  --hardware-label 'describe CPU, memory, OS, and power mode'
```

Use `--transport loopback` for separate UDP-send measurements. Preserve the JSON report: it records
the 32-universe/100 Hz floor, 64-universe/120 Hz target, 4/8-universe low-power goals, distributions,
deadline misses, encoded bytes, contribution coverage, and explicitly unmeasured items. Do not
convert “not measured” CPU, allocation, sound-analysis, or production socket delivery into an
implicit pass.

Frontend performance evidence must cover the full input-to-visible-update path and record request
count, payload bytes, mutation response, event arrival, snapshot repair, store update, and visible
paint. See [Frontend performance baseline](frontend-performance-baseline.md).

## Verification ladders

For a normal typed slice:

```sh
./test architecture
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
./test unit
./test e2e tests/<focused-spec>.spec.ts --grep '<scenario-id>'
```

Before final release handoff, widen to the applicable unrestricted socket tests, all Playwright
surfaces, Hardware Controls tests/build, `./test desktop-smoke`, release benchmarks, manual build,
and `./build open` operator-path verification. Compare failures by stable scenario ID and behavior,
not only by raw failure count.
