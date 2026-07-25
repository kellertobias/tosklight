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
| 00 | [Programmer relative encoders and fade-time scope](pending/00-programmer-relative-encoders-and-fade-time-scope.md) | Implement the existing cross-surface encoder behavior contract. |
| 01 | [Unpatched fixtures from Add Fixture](pending/01-unpatched-fixtures-on-add-fixture.md) | Implement the existing **Empty** address workflow. |
| 02 | [Shared frontend components and deterministic screenshots](pending/storybook%20changes/02-shared-frontend-components-and-storybook-screenshots.WORKING.md) | Concurrent Storybook lane; move the UI library to `apps/ui-library`, validate production components in Storybook, then make accepted stories the documentation screenshot source. |
| 03 | [Consistent pool-object colors](pending/03-consistent-pool-object-colors.md) | Waiting for accepted shared pool primitives from plan 02. |
| 04 | [Empty-selection object target selection](pending/04-empty-selection-object-target-selection.md) | Waiting for stable application pool adapters from plan 02. |
| 05 | [Warm frontend state for instant surface switching](pending/05-warm-frontend-state-for-instant-switching.md) | Waiting for stable frontend provider and Storybook-harness boundaries from plan 02. |
| 06 | [Exclusive active-show mutation boundary](pending/06-exclusive-active-show-mutation-boundary.md) | Route every active-show write through one transaction owner. |
| 07 | [Typed commands, events, and Highlight service](pending/07-typed-commands-events-and-highlight-service.md) | Waiting for stable desktop providers, generated-client consumers, and frontend verification after plan 02. |
| 08 | [Typed control-surface interaction routing](pending/08-typed-control-surface-interaction-routing.md) | Waiting for plan 02's component callback and modal/interaction boundaries. |
| 09 | [Patch performance benchmarks](pending/09-patch-performance-benchmarks.md) | Waiting for plan 02 and plan 05's stable client-store and visible-paint path. |
| 10 | [Complete shared control-surface contracts](pending/10-complete-shared-control-surface-contracts.md) | Waiting for plans 07–08 to stabilize typed command and interaction names. |
| 11 | [Packaged operator and reference-hardware verification](pending/11-packaged-operator-and-reference-hardware-verification.md) | Waiting for the accepted frontend and access to the required reference hardware. |
| 12 | [Typed active-show object contracts](pending/12-typed-active-show-object-contracts.md) | Waiting for plan 07's final semantic event and generated-client boundaries. |
| 13 | [Capability-owned application state](pending/13-capability-owned-application-state.md) | Waiting for plan 07's mutation/event owners, as required by the plan. |
| 14 | [Modular desktop host](pending/14-modular-desktop-host.md) | Next independent plan; leave the Tauri entry point as composition only. |

Product-roadmap work under `docs/plans/Next` and `docs/plans/Later` remains separate unless a queue
file explicitly links it as its behavior contract.

## Concurrent Storybook lane

Plans under `pending/storybook changes/` form one frontend-owned lane. A `.WORKING.md` suffix marks
its active ownership. That lane may run concurrently with the lowest-numbered unblocked plan only
when its files, contracts, runtime state, and verification do not overlap.

Plan 02 is currently active in that lane. Plans 03–05 and 07–13 record explicit dependencies on
its shared components, application adapters, providers, generated consumers, or downstream typed
contracts. Plan 06 completed independently; plan 14 is the next independent numbered plan.
