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
4. Run every verification gate recorded in the plan.
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
| 17 | [Efficient built-in Stage visualizer](doing/14-efficient-built-in-stage-visualizer.md) | Currently owned; add isolated visualization telemetry, retained rendering, and the four Stage render qualities without slowing engine or DMX output. |
| 18 | [Dedicated Virtual Playbacks and exclusion zones](pending/15-virtual-playbacks-and-exclusion-zones.md) | Replace page-slot aliases with Virtual Playbacks 1001–9998 and rebuild exclusion zones on the new identity. |
| 19 | [Repository-wide dead-code removal](pending/19-repository-wide-dead-code-removal.md) | After feature migration stabilizes, audit and safely remove dead backend, UI, test, tooling, dependency, and compatibility code, prioritizing the Rust backend. |

Product-roadmap work under `docs/plans/Next` and `docs/plans/Later` remains separate unless a queue
file explicitly links it as its behavior contract.

The filenames of the Stage and Virtual Playback plans retain their originally authored
numbers. The table's execution order is authoritative: finish Dynamics first, then
the built-in Stage visualizer, Dedicated Virtual Playbacks, and the repository-wide
dead-code removal.

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

The current execution order is the built-in Stage visualizer, Dedicated Virtual Playbacks,
then the repository-wide dead-code removal.
Before claiming each new plan, query the large Codex usage window with the Tosken Raider MCP
`get_remaining_usage` tool. Do not start another plan when the remaining large-window allowance is
below 30%; finish and commit an already claimed coherent plan before stopping.

An empty pending queue is not by itself completion. After the numbered queue is empty, audit the
current repository against every applicable requirement and acceptance item in
`../major-refactoring.md`, including its known-incomplete list and retained operational evidence.
Create new numbered pending plans for any genuine implementation or verification gaps, then
continue them under this workflow while the large-window allowance remains at least 30%.

Only when that final audit finds the major refactor genuinely complete should the agent run the
proportionate final release gates, update the consolidated execution record, commit the coherent
completion state, and push `main`. The authorization to push applies to that completed
refactoring handoff only; do not push a partial queue.
