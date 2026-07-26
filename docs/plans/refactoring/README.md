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
| 03 | [Consistent pool-object colors](pending/03-consistent-pool-object-colors.md) | Next unblocked plan; consume the accepted shared pool primitives from plan 02. |
| 04 | [Empty-selection object target selection](pending/04-empty-selection-object-target-selection.md) | Queued after plan 03; stable application pool adapters are now available. |
| 05 | [Warm frontend state for instant surface switching](pending/05-warm-frontend-state-for-instant-switching.md) | Queued after plan 04; the frontend provider and Storybook-harness prerequisite is complete. |
| 06 | [Exclusive active-show mutation boundary](finished/06-exclusive-active-show-mutation-boundary.md) | Completed single transaction owner for active-show writes. |
| 07a | [Backend Highlight application service](finished/07a-highlight-application-service.md) | Completed independent backend prerequisite; existing HTTP, WebSocket, OSC, persistence, output, event, and feedback behavior now shares one application service. |
| 07b | [Typed commands and capability events](pending/07b-typed-commands-and-capability-events.md) | Queued after plans 03–05; its plan 02 frontend prerequisite and Highlight-service prerequisite are complete. |
| 08 | [Typed control-surface interaction routing](pending/08-typed-control-surface-interaction-routing.md) | Queued after plan 07b; plan 02's component callback and modal/interaction boundaries are stable. |
| 09 | [Patch performance benchmarks](pending/09-patch-performance-benchmarks.md) | Waiting for plan 02 and plan 05's stable client-store and visible-paint path. |
| 10 | [Complete shared control-surface contracts](pending/10-complete-shared-control-surface-contracts.md) | Waiting for plans 07–08 to stabilize typed command and interaction names. |
| 11 | [Packaged operator and reference-hardware verification](pending/11-packaged-operator-and-reference-hardware-verification.md) | Waiting for the accepted frontend and access to the required reference hardware. |
| 12 | [Typed active-show object contracts](pending/12-typed-active-show-object-contracts.md) | Waiting for plan 07's final semantic event and generated-client boundaries. |
| 13 | [Capability-owned application state](pending/13-capability-owned-application-state.md) | Waiting for plan 07's mutation/event owners, as required by the plan. |
| 14 | [Modular desktop host](finished/14-modular-desktop-host.md) | Completed thin Tauri composition root and cohesive host modules. |

Product-roadmap work under `docs/plans/Next` and `docs/plans/Later` remains separate unless a queue
file explicitly links it as its behavior contract.

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

After that signal, continue plans 03, 04, 05, 07b, 08, 09, 10, 11, 12, and 13 in numeric dependency
order. Before claiming each new plan, query the large Codex usage window with the Tosken Raider MCP
`get_remaining_usage` tool. Do not start another plan when the remaining large-window allowance is
below 50%; finish and commit an already claimed coherent plan before stopping.

Plan 11 has an external evidence dependency in addition to its queue dependency: the named
reference Mac, attached controls, and Pi-class host must be available. Mocked or development-only
evidence cannot substitute for those surfaces. If they are unavailable when plan 11 is reached,
record the exact missing access and stop at that non-deferrable blocker.

An empty pending queue is not by itself completion. After the numbered queue is empty, audit the
current repository against every applicable requirement and acceptance item in
`../major-refactoring.md`, including its known-incomplete list and retained operational evidence.
Create new numbered pending plans for any genuine implementation or verification gaps, then
continue them under this workflow while the large-window allowance remains at least 50%.

Only when that final audit finds the major refactor genuinely complete should the agent run the
proportionate final release gates, update the consolidated execution record, commit the coherent
completion state, and push `main`. The authorization to push applies to that completed
refactoring handoff only; do not push a partial queue.
