# Refactoring Queue

This directory is the execution queue for the remaining ToskLight refactoring and closely related
operator-behavior work. Lower numbered pending files are implemented first unless a blocking
dependency is recorded in the file.

The completed major-refactoring history and the audit that produced the remaining architecture
items are consolidated in [`../major-refactoring.md`](../major-refactoring.md).

## State workflow

1. Read this README and the complete pending plan.
2. Move the selected file from `pending/` to `doing/` before changing implementation code.
3. Keep the plan and its linked source specifications current while working.
4. Run the focused checks and required full end-to-end coverage recorded in the plan. Do not run
   the complete three-show benchmark sweep until the final renderer/benchmark phase.
5. Add a `## Result` section containing the implementation summary, tests, limitations, and commit.
6. Move the file from `doing/` to `finished/` and commit the move with the implementation.

Only one agent owns a plan in `doing/`. Independent plans may run concurrently only when their
files, contracts, runtime state, and verification surfaces do not overlap.

## Ordered pending work

| Order | Plan | Purpose |
|---:|---|---|
| 00 | [Programmer relative encoders and fade-time scope](finished/00-programmer-relative-encoders-and-fade-time-scope.md) | Completed cross-surface encoder behavior contract. |
| 01 | [Unpatched fixtures from Add Fixture](finished/01-unpatched-fixtures-on-add-fixture.md) | Completed **Empty** address workflow. |
| 02 | [Shared frontend components and deterministic screenshots](finished/storybook%20changes/02-shared-frontend-components-and-storybook-screenshots.DONE.md) | Completed production UI-library adoption, Storybook validation, and deterministic/live documentation screenshot ownership. |
| 03 | [Consistent pool-object colors](finished/03-consistent-pool-object-colors.md) | Completed shared pool-color semantics and adoption. |
| 04 | [Empty-selection object target selection](finished/04-empty-selection-object-target-selection.md) | Completed authoritative empty-selection target workflow. |
| 05 | [Warm frontend state for instant surface switching](finished/05-warm-frontend-state-for-instant-switching.md) | Completed authoritative frontend capability warmup. |
| 06 | [Exclusive active-show mutation boundary](finished/06-exclusive-active-show-mutation-boundary.md) | Completed single transaction owner for active-show writes. |
| 07a | [Backend Highlight application service](finished/07a-highlight-application-service.md) | Completed independent backend prerequisite; existing HTTP, WebSocket, OSC, persistence, output, event, and feedback behavior now shares one application service. |
| 07b | [Typed commands and capability events](finished/07b-typed-commands-and-capability-events.md) | Completed typed application commands and semantic capability events. |
| 08 | [Typed control-surface interaction routing](finished/08-typed-control-surface-interaction-routing.md) | Completed typed software and hardware interaction routing. |
| 09 | [Patch performance benchmarks](finished/09-patch-performance-benchmarks.md) | Completed retained Patch performance evidence. |
| 10 | [Complete shared control-surface contracts](finished/10-complete-shared-control-surface-contracts.md) | Completed shared control-surface contracts. |
| 11 | [Sustained complex-show output benchmark](finished/11-sustained-complex-show-output-benchmark.md) | Completed retained complex-show output benchmark evidence. |
| 12 | [Typed active-show object contracts](finished/12-typed-active-show-object-contracts.md) | Completed typed active-show object contracts. |
| 13 | [Capability-owned application state](finished/13-capability-owned-application-state.md) | Completed capability-owned application state. |
| 14 | [Modular desktop host](finished/14-modular-desktop-host.md) | Completed thin Tauri composition root and cohesive host modules. |
| 16 | [Dynamics](finished/16-dynamics/README.md) | Completed scalar Dynamics runtime, persistence, Programmer/Cue/Preload, Playback, transport, reviewed UI, compatibility removal, and acceptance evidence. |
| 17 | [Dynamics lane layout and interaction regression](finished/14a-dynamics-lane-layout-and-interaction-regression.md) | Completed full-width lane geometry, valid sibling interactions, and production Storybook regression coverage. |
| 18 | [Dedicated Virtual Playbacks and exclusion zones](finished/18-virtual-playbacks-and-exclusion-zones.md) | Completed: stable 300-control Virtual Playback banks and portable show-wide number-based exclusion zones. |
| 19 | [Repository-wide dead-code removal](finished/19-repository-wide-dead-code-removal.md) | Completed repository-wide audit, safe removal, retained-path classification, and source-size hard-limit cleanup. |
| 20 | [Three-tier demo and benchmark shows](finished/20-three-tier-demo-and-benchmark-shows.md) | Completed realistic demo, 1,000-fixture interactive benchmark, and 2,000–4,000-fixture headless stress workloads for the final Stage optimization phase. |
| 21 | [Efficient built-in Stage visualizer](finished/21-efficient-built-in-stage-visualizer.md) | Completed bounded latest-value telemetry, adaptive Stage rendering, Fixture Sheet virtualization, and the benchmark/end-to-end performance gate. |
| 22 | [Per-fixture master participation and Pan/Tilt inversion](finished/22-per-fixture-master-participation-and-pan-tilt-inversion.md) | Completed per-fixture Group Master and Grand Master participation plus Pan/Tilt inversion in Show Patch. |
| 23 | [Schedules](finished/23-schedules.md) | Completed authoritative scheduling, persistence, validation, APIs, events, and the Scheduler operator surface. |
| 25 | [External screen fixed full-screen pane](finished/25-external-screen-fixed-pane.md) | Completed desk-configured, view-only fixed panes for optional external screens. |
| 26 | [Selection grids, Layout pane, and Auto 2D Grid](finished/26-auto-2d-grid-from-3d-positions.md) | Finished: deterministic auto-2D generation, persisted selection grids, shared Shift actions, and the Layout pane are complete. |
| 27 | [Partial Show Load](finished/27-partial-show-load.md) | Finished: one selective-import workflow now provides grouped sections and patch layers, positional replacement, additive allocation and rewrites, scoped Stage merging, File Manager parity, and one operator Undo step. |
| 28 | [Desk-wide Highlight look](finished/28-desk-wide-highlight-look.md) | Finished: one desk-owned semantic Highlight Look now resolves authored fixture capabilities while preserving explicit legacy compatibility. |
| 29 | [Attribute registry, activation groups, and Indexed Presets](finished/29-attribute-registry-and-activation-groups.md) | Completed the canonical registry, coherent programmer activation, Indexed Presets, persistence, fixture-import mapping, and legacy compatibility contracts. |
| 30 | [Green GitHub CI and visible benchmark results](finished/30-green-github-ci-and-visible-benchmark-results.md) | Completed exact-commit green CI, release publication, visible measured benchmark evidence and GitHub Pages deployment. |
| 31 | [Supported scale, output isolation, and warm operator UI](doing/31-supported-scale-output-isolation-and-warm-operator-ui.md) | In progress: prove the packaged 1,000-fixture Stage, Fixture Sheet, output-isolation, and action-latency contract. |
| 32 | [Macros](pending/32-macros-and-scheduled-macros.md) | Implement the settled TypeScript Macro package, runtime, permission, interaction, and persistence contract after supported-scale/output-isolation work. This is the final queued refactoring to start. |

Product-roadmap work under `docs/plans/Next` and `docs/plans/Later` remains separate unless a queue
file explicitly links it as its behavior contract.

The queue's filenames and table use the same execution order. Plans through green GitHub CI and
visible benchmark evidence are complete. Supported-scale/output-isolation work is active; Macros
remains next. The benchmark's measured result remains visibly degraded under Plan 30's temporary
informational policy. Timecode remains outside this queue.

## Completed Storybook lane

The frontend-owned Storybook lane completed on 2026-07-26 and is archived under
`finished/storybook changes/`. Every lane plan has a `.DONE.md` suffix and a Result section; no
active `.WORKING.md` plan remains. Plans 03–05, 07b, and 08–13 may now consume its stable shared
components, application adapters, providers, generated consumers, and downstream typed contracts.

## Completed Storybook handoff

The frontend lane was handed back after its owner:

1. completed every plan in the Storybook lane, including focused and proportionate
   broader verification;
2. added the required Result and commit information to the lane plans;
3. moved the complete `pending/storybook changes/` directory to
   `finished/storybook changes/`; and
4. prepared the coherent lane without leaving an active `.WORKING.md` plan behind.

The directory move is the machine-readable signal that plan 02 and the frontend contracts it owns
are stable. That signal is now present.

## Usage-gated continuation

Before claiming a plan and at meaningful checkpoints, query the large Codex usage window with the
Tosken Raider MCP tool **`mcp__tosken_raider.get_remaining_usage`**. Use its large-window
remaining percentage for these completion gates:

| Required completed phase | Minimum remaining usage |
|---|---:|
| Dynamics, including the lane-layout and interaction regression | Above 80% |
| Dedicated Virtual Playbacks and exclusion zones | Above 75% |
| Repository-wide dead-code removal | Above 65% |
| Renderer optimization and the separate demo/benchmark shows | Above 50% |

These are completion thresholds, not merely permission-to-start thresholds. If the active phase
cannot be completed while still above its threshold, stop expanding scope, wrap the current
coherent work, keep the queue state truthful, and report exactly what is done and what remains.
Do not claim or begin the next phase. If renderer optimization and the new shows are not complete
above 50%, stop after the same wrap-up and handoff.

Every plan still receives its focused checks and required full end-to-end coverage. The complete
three-show performance sweep is intentionally deferred to the final renderer/benchmark phase;
running that long sweep after every preceding refactor is neither required nor useful.

An empty pending queue is not by itself completion. After the numbered queue is empty, audit the
current repository against every applicable requirement and acceptance item in
`../major-refactoring.md`, including its known-incomplete list and retained operational evidence.
Create new numbered pending plans for any genuine implementation or verification gaps, then
continue them under this workflow only while the applicable usage threshold above can still be
met.

Only when that final audit finds the major refactor genuinely complete should the agent run the
proportionate final release gates, update the consolidated execution record, commit the coherent
completion state, and push `main`. The authorization to push applies to that completed
refactoring handoff only; do not push a partial queue.
