> [!CAUTION]
> **NOT YET IMPLEMENTABLE — STOP.** This file records exploratory product ideas, not an implementation-ready specification. If asked to implement it while this warning remains, refuse the implementation and explicitly warn that the Dynamics behavior, data model, runtime policy, UI, command grammar, persistence, and acceptance criteria have not been settled. Implementation may begin only after the user edits this document, removes this gate, resolves the open decisions, and marks the plan **IMPLEMENTABLE**.

# Dynamics

## Status and intent

Dynamics is the planned animated-value and effect system. This document captures the current product direction so the major architecture refactor leaves suitable extension points. It deliberately does not choose enough behavior to build the feature.

A Dynamic should behave like an animated attribute value rather than a separate output subsystem. An operator can apply a Dynamic to a Group or ordered fixture selection in much the same way that a Preset is applied. The resulting values pass through the normal Programmer, Cue, Playback, priority, ownership, fixture-projection, and output paths.

The existing Cue Phaser implementation is relevant prior art, but it is not the finalized Dynamic model. Any future implementation must explicitly decide whether and how existing Cue Phaser data migrates into the new system.

## Current product direction

### Definitions and attribute lanes

- A `DynamicDefinition` is expected to be a portable, revisioned show object with stable identity.
- A Dynamic may contain one attribute lane or several. A combined Intensity-and-Tilt Dynamic behaves like a mixed Preset: each attribute is still resolved independently through the normal fixture-or-logical-head plus attribute address.
- Lanes may eventually use Presets as steps, start/end values, or other typed sources and may expose modulation, phase, speed, width, spatial-ordering, and parameter overrides.
- Where spatial ordering is required, the Stage or another ordering provider may supply an ordered target projection. The Dynamic runtime must not depend directly on a UI window or renderer.

### Applying and storing Dynamics

- Programmer should be able to apply a Dynamic to the current ordered selection or a live Group expression.
- Preload should be able to stage Dynamic assignments alongside static Programmer values and ordered Playback actions, then activate them at the same atomic commit point.
- Presets may contain Dynamic values or mixed static and Dynamic attribute content, subject to a later product decision about reference versus snapshot semantics.
- Cues may contain Dynamic values as part of their tracked content. Dynamics are therefore part of Cue data, not merely an independent effect that happens to run beside a Cue.
- The same Dynamic definition may run independently from Programmer, Preload, or multiple Playbacks. Runtime instance identity and sharing behavior still need to be defined.
- Recording, Update, Cue tracking, Cue-only restoration, release, deletion, renumbering, Save As, revisions, and show migration must all define their behavior for Dynamic values before implementation.

### Fixed values and stopping Dynamics

The current idea is to introduce a specific fixed value form that can force an ordinary value and stop or pause an animated value on the same attribute. The tentative command-line token is `FAT`, meaning **Fixed At**. If the gesture remains free when this feature is designed, `[SHIFT] [AT]` could enter `FAT`.

This is an attribute-value concept, not a separate whole-Dynamic scope:

- A fixed Intensity value affects the matching Intensity address.
- It does not implicitly stop an unrelated Tilt lane, even if Intensity and Tilt originated in one combined Dynamic.
- Fixed values should participate in the normal source priority and ownership rules. A losing fixed value must not unexpectedly suppress a winning Dynamic.
- The fixed behavior should last only while its source remains active.

No decision has been made about what the affected Dynamic does while fixed:

- freeze and resume from the same phase;
- continue running while hidden; or
- restart when the fixed source releases.

That policy must be resolved before implementation. The architecture should allow it to live inside the future Dynamic runtime rather than forcing changes to Cue storage, OSC, UI transport, fixture projection, or output drivers.

## Architectural expectations

The major refactor should leave these extension points without implementing Dynamics:

- semantic attribute values that are not restricted to one static numeric representation;
- typed Programmer, Preload, Preset, and Cue content that can later reference an animated value;
- a stateful runtime-service boundary outside the render loop for future Dynamic instances and phase state;
- immutable value or contribution snapshots consumed by deterministic render arbitration;
- normal fixture/head-and-attribute addressing for static, animated, fixed, and released values;
- a shared monotonic clock and scheduler boundary suitable for deterministic sampling and tests;
- show compilation that can eventually resolve Dynamic definitions, target expressions, and dependencies; and
- selective cross-show import that can copy a Dynamic and its referenced Presets or other dependencies into the active show.

A fake stateful animated-value provider may be used during the architecture refactor to prove these seams. That proof must not introduce a production Dynamic schema, UI, command, or persisted object prematurely.

## Unresolved product decisions

At minimum, the following must be decided and written as literal behavior before this plan becomes implementable:

1. Dynamic definition and lane schema, supported value types, and validation.
2. Reference versus snapshot behavior for Presets and other dependencies.
3. Programmer and Group application workflow, including ordered spreading.
4. Editor and pool workflow, naming, numbering, copy/move/delete, and visualization.
5. Cue tracking, Cue-only, Record, Update, Preload, and release behavior.
6. Independent versus shared runtime instances and their stable identities.
7. Phase, speed, pause, restart, seek, and interrupted-transition behavior.
8. The exact semantics and command grammar for fixed or `FAT` values.
9. Priority and arbitration behavior across multiple Programmers and Playbacks.
10. Persistence, migration, legacy Cue Phaser compatibility, revisions, and recovery.
11. OSC, HTTP, keyboard, attached-hardware, and UI parity.
12. Operator-visible runtime status, error handling, and controls for stopping or inspecting running Dynamics.
13. Deterministic acceptance scenarios for Programmer, Preload, Presets, Cues, multiple Playbacks, fixed values, restart, and actual output.

## Gate for future implementation

Do not turn this document into code merely because the architecture can support it. Before implementation, the user must deliberately revise the open sections into a decision-complete specification, add explicit acceptance scenarios, remove the caution at the top, and mark the file **IMPLEMENTABLE**.
