# Repository-wide Dead-code Removal

## Queue position and status

**Pending and ordered after the built-in Stage visualizer and Dedicated Virtual
Playbacks.**

Claim this plan only after
[`14-efficient-built-in-stage-visualizer.md`](../doing/14-efficient-built-in-stage-visualizer.md)
and
[`15-virtual-playbacks-and-exclusion-zones.md`](15-virtual-playbacks-and-exclusion-zones.md)
are finished. Those feature migrations deliberately retain transitional paths while
their callers and compatibility boundaries are changing; auditing first would create
false positives or force the same code to be inspected twice.

Move this file to `doing/` before changing implementation code, query Tosken Raider as
required by the queue README, and follow the normal verification, Result, finished-plan,
and semantic-commit workflow.

## Goal

Inspect the complete repository for dead, unreachable, superseded, or unowned code and
remove it safely. Cover the Rust backend first and most deeply, then the desktop UI,
shared UI packages, tests and fixtures, tooling, scripts, generated-contract workflows,
and build or CI configuration.

The outcome is a smaller production and test surface without changing operator
behavior, persisted-show compatibility, supported APIs, OSC paths, desktop lifecycle,
output timing, or control-surface parity.

## Proof standard

Do not delete code merely because its name is old, it has no obvious direct caller, or
an automated detector reports it. Before removal, classify each candidate using at
least the applicable evidence:

- repository references, public re-exports, generated consumers, feature flags, Cargo
  targets, build scripts, dynamic dispatch, serde names, route registration, Tauri
  commands/events, OSC mappings, reflection-like registries, and fixture/package
  discovery;
- runtime entry points, startup/shutdown ownership, persistence migration and recovery
  paths, portable-show codecs, unknown-field preservation, and compatibility adapters;
- test-only and benchmark-only consumers, Storybook stories, screenshot/manual
  generation, release packaging, CI, developer commands, and platform-specific code;
- `git log` or the completed refactoring plans when a compatibility or migration seam
  has intentionally delayed removal; and
- focused characterization or contract coverage when removing a path whose behavior
  is not already protected.

Record retained false positives and the reason they are live. Prefer deleting an
entire proven-obsolete path over leaving forwarding shells, stale flags, unused DTO
fields, commented-out implementations, or tests that only exercise deleted behavior.

## Backend-first audit

Audit every Rust workspace member and executable, with special attention to
`crates/light/adapters/headless`, application services, wire contracts, show
persistence, Programmer, Playback, Dynamics, Stage visualization, control, output,
and desktop host commands.

At minimum:

1. Find unused modules, types, traits, methods, functions, fields, enum variants,
   feature flags, dependencies, dev-dependencies, routes, commands, events, metrics,
   migrations, codecs, and compatibility facades.
2. Trace raw shared-state and lock-bearing access, duplicated active-show mutation,
   string-plus-JSON commands, broad facade notifications, legacy visualization
   polling, old Virtual Playback aliases, and removed Phaser/Dynamics compatibility
   paths. Delete only the portions whose callers have genuinely migrated.
3. Remove obsolete transport DTOs and generated contract members together with every
   Rust and TypeScript consumer; regenerate checked-in schemas and clients through the
   repository workflow.
4. Preserve required legacy show decoding, unknown-object retention, startup recovery,
   OSC compatibility, platform-gated implementations, and intentionally supported v1
   adapters unless their removal is explicitly proven and accepted.
5. Check Cargo manifests for dependencies and features that became unused after code
   deletion. Do not use destructive automated dependency fixes.
6. Verify that removal does not add work, allocation, locking, serialization, or
   awaited operations to the engine and output critical paths.

## UI and desktop audit

Audit `apps/light-desktop`, `apps/light-hardware-controls`, shared UI packages, and the
Tauri hosts for:

- unused exports, components, hooks, stores, reducers, providers, adapters, DTO
  translations, CSS selectors, assets, routes, stories, feature flags, event handlers,
  bridge commands, and window lifecycle paths;
- superseded broad `useServer()` or compatibility access after all real callers use
  capability-owned state;
- duplicate production and Storybook implementations, retaining the production
  component and representative story rather than a story-only lookalike;
- effects, subscriptions, timers, observers, sockets, caches, and disposal paths that
  exist solely for removed consumers; and
- package dependencies, build aliases, and generated imports left unused by the
  refactor.

Do not remove accessibility, hardware-connected, software-only, touch, keyboard,
additional-screen, offline, loading, error, reconnect, or recovery paths just because
they are difficult to reach in a single development session.

## Tests, fixtures, tooling, and documentation audit

- Remove tests, fixtures, fakes, helpers, semantic bindings, screenshots, and stories
  that exclusively protect deleted behavior. Retain regression coverage for the
  supported replacement behavior.
- Detect tests that pass without executing their intended production path, stale skips,
  orphaned snapshots, unused fixture data, and duplicate helpers. Resolve them
  deliberately rather than maximizing deletion count.
- Audit root scripts, `tools/`, Cargo and npm commands, Playwright configuration,
  Storybook/manual/screenshot workflows, CI/release jobs, artifact helpers, and
  architecture checks for unreferenced or superseded code.
- Preserve developer and release entry points documented in
  `docs/engineering/build-and-test-commands.md`, platform packaging, generated files,
  and repository-owned artifact policies.
- Update architecture, code-tour, test-map, build-command, and help documentation when
  a removed path was still described as supported.

## Tooling and retained evidence

Use several complementary signals rather than treating one scanner as authoritative:

- Rust compiler warnings and workspace checks across all targets and applicable
  features;
- Cargo dependency metadata plus a reviewed unused-dependency report when a compatible
  tool is available;
- TypeScript type-checking, lint/build output, import/export and package-dependency
  analysis configured for this monorepo's aliases, generated files, stories, tests,
  and platform entry points;
- repository searches for routes, serialized names, OSC paths, Tauri commands, scripts,
  CI commands, and documentation consumers; and
- focused runtime checks for dynamically registered or platform-owned paths.

Store reports under the canonical `.artifacts/` paths. Add a maintained check only when
its false-positive model is understood and it can run deterministically in the normal
repository workflow. Do not add a noisy CI gate merely to claim tool coverage.

## Verification and completion

Work in coherent, reviewable removal slices, starting with the backend. After each
slice, run the smallest affected checks and inspect the diff for accidental API,
schema, migration, or operator-behavior changes.

Completion requires:

1. Rust formatting, workspace checks and tests across all targets and applicable
   feature/platform configurations available on the current host.
2. Generated wire-contract verification and focused persistence, startup/recovery,
   API, WebSocket, OSC, Programmer, Playback, Dynamics, Stage, control, and output
   tests for every affected backend boundary.
3. Frontend lint/type-check, unit tests, production builds, Storybook checks, and
   focused software/hardware UI acceptance for affected surfaces.
4. Tooling, architecture, source-size, artifact-path, semantic-catalog, and CI-script
   checks affected by the removal.
5. The proportionate root API, UI, supplemental E2E, desktop smoke, and `npm run open`
   gates, including readiness and runtime-log inspection.
6. A final repository-wide reference and dependency scan with every retained candidate
   classified, plus `git diff --check` and a clean worktree.
7. Documentation of removed paths, retained false positives, compatibility decisions,
   exact verification results, and any external platform evidence that remains
   unavailable.
8. A `## Result`, move to `finished/`, and a semantic-release commit.

Do not claim that all dead code is removed solely from compiler success or a scanner
report. Completion means every repository area was inventoried, backend candidates
were traced deeply, removals were verified through their real boundaries, and retained
candidates have an explicit reason.
