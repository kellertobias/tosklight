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
| 02 | [Shared frontend components and deterministic screenshots](pending/02-shared-frontend-components-and-storybook-screenshots.md) | Extract production UI into `packages/ui`, validate it in Storybook, then make accepted stories the documentation screenshot source. |
| 03 | [Consistent pool-object colors](pending/03-consistent-pool-object-colors.md) | Implement the shared pool color language. |
| 04 | [Empty-selection object target selection](pending/04-empty-selection-object-target-selection.md) | Make populated Presets and Effect-like objects select their stored targets on the first empty-selection tap. |
| 05 | [Warm frontend state for instant surface switching](pending/05-warm-frontend-state-for-instant-switching.md) | Load active state first, warm the remaining app model in the background, and reconcile it through events. |
| 06 | [Exclusive active-show mutation boundary](pending/06-exclusive-active-show-mutation-boundary.md) | Route every active-show write through one transaction owner. |
| 07 | [Typed commands, events, and Highlight service](pending/07-typed-commands-events-and-highlight-service.md) | Remove the remaining string/JSON compatibility transport and duplicate Highlight orchestration. |
| 08 | [Typed control-surface interaction routing](pending/08-typed-control-surface-interaction-routing.md) | Remove DOM-discovered SET, Store, Update, and keypad routing. |
| 09 | [Patch performance benchmarks](pending/09-patch-performance-benchmarks.md) | Retain server and visible-UI timing evidence for Patch. |
| 10 | [Complete shared control-surface contracts](pending/10-complete-shared-control-surface-contracts.md) | Share stable OSC, layout, Highlight, and playback intent contracts between applications. |
| 11 | [Packaged operator and reference-hardware verification](pending/11-packaged-operator-and-reference-hardware-verification.md) | Finish visual, packaged-app, output, Mac, and low-power evidence. |
| 12 | [Typed active-show object contracts](pending/12-typed-active-show-object-contracts.md) | Remove generic JSON from application-level active-show mutations and events. |
| 13 | [Capability-owned application state](pending/13-capability-owned-application-state.md) | Replace the large raw-lock `AppState` container with capability-owned resources. |
| 14 | [Modular desktop host](pending/14-modular-desktop-host.md) | Leave the Tauri entry point as composition only. |

Product-roadmap work under `docs/plans/Next` and `docs/plans/Later` remains separate unless a queue
file explicitly links it as its behavior contract.
