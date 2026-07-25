> [!CAUTION]
> **NOT YET IMPLEMENTABLE — STOP.** This chunk records server/runtime requirements and unresolved product decisions. Do not implement Dynamic evaluation, schema, persistence, command grammar, or output behavior until this chunk is revised into a decision-complete specification and marked **IMPLEMENTABLE**.

# Runtime and Application Semantics

## Status

**Specification only.** This chunk records the actual Dynamic model, server requirements, application semantics, and output behavior.

## Goal

Define how Dynamics are represented, applied, sampled, stored, migrated, and rendered through the authoritative ToskLight server.

Dynamics should become flexible animated attribute content that can be active in the Programmer, stored into Presets or Cues, assigned later to Playbacks, and managed as reusable show objects.

## Definitions and Attribute Lanes

- A `DynamicDefinition` is a portable, revisioned show object with stable identity.
- A Dynamic may contain one attribute lane or several.
- A combined Intensity-and-Tilt Dynamic is only a container for independent Intensity and Tilt lanes; it must not create a shared multi-attribute path.
- Each attribute is resolved independently through the normal fixture-or-logical-head plus attribute address.
- Lanes may use individual values extracted from Presets as keyframes, bounds, start/end values, or other typed scalar sources.
- Where spatial ordering is required, the Stage or another ordering provider may supply an ordered target projection. The Dynamic runtime must not depend directly on a UI window or renderer.

## Lane Value Modes

Every lane selects one value-generation mode. The mode is per attribute, so lanes in the same Dynamic may use different modes.

### Keyframed Mode

Keyframed mode defines a cyclic sequence of scalar landmarks for one attribute:

- a keyframe source may be a literal value, the matching scalar value extracted from a Preset, or `Current`;
- the first keyframe is also the loop-closing value;
- selecting two sources creates the default three-keyframe cycle `A at 0%`, `B at 50%`, and `A at 100%`;
- inserted keyframes belong only to that lane and have explicit normalized time within the cycle; and
- each adjacent pair has its own interpolation or easing shape.

### Center-and-Size Mode

Center-and-size mode evaluates a cyclic function around a scalar center:

- `center` may be a literal value, the matching scalar value extracted from a Preset, or `Current`;
- `size` defines the scalar excursion or amplitude; and
- `shape` selects the normalized cycle function, initially including sine, cosine, linear rising, linear falling, and pulse-width modulation.

Switching lane mode or shape must use explicit conversion rules. No implementation may silently discard keyframes or shape parameters.

## Current, Composition, and Recursion

`Current` means the fixture's static value for that attribute immediately before the evaluated Dynamic is applied. It is a reference to an upstream value, not an output readback after the same Dynamic has contributed.

The evaluation graph must be acyclic and must define:

- which static winner forms `Current` for a Dynamic instance;
- whether a higher-priority or later LTP Dynamic may modulate the result of an earlier Dynamic;
- whether several Dynamics on one attribute replace, add, multiply, or otherwise compose;
- how LTP timestamps and ownership apply to Dynamic assignments;
- what happens when a source using `Current` wins, releases, or changes priority; and
- how dependency cycles are detected and reported instead of recursively feeding a Dynamic's output back into itself.

## Speed and Synchronization

Each Dynamic has one stable speed source for the lifetime of its runtime instance:

- a fixed duration in seconds per cycle; or
- one Speed Group that supplies shared tempo and transport.

The Dynamic has an overall rational speed multiplier. Each attribute lane may additionally have its own rational multiplier. Linked Speed Groups must share an authoritative transport epoch so all Dynamics using the same or linked groups remain phase-stable.

Start policy remains open, but the implementation must distinguish:

1. start now with a local epoch;
2. join synchronized position now; and
3. start on the next synchronized boundary.

## Phase Distribution

Fixture phase is an offset inside the Dynamic cycle. It is independent of Speed Group transport position, activation quantization, and bar/beat start policy.

Manual entry and `THRU`-style distribution must be supported. Blocks, repeats, and wings are phase-assignment helpers; they do not merge fixtures, change authoritative selected-fixture order, or create multi-attribute paths.

Phase distribution may use:

- authoritative selection order;
- grid linear projection;
- grid radial out; or
- grid radial in.

The grid is used only to order or group fixtures for phase assignment.

## Applying and Storing Dynamics

Programmer should be able to apply a Dynamic to the current ordered selection or a live Group expression.

The implementation must define:

- whether Presets reference Dynamic objects, copy Dynamic content, or store a composed snapshot;
- whether Cues reference Dynamic objects, copy Dynamic content, or store tracked Dynamic values;
- how Preload stages Dynamic assignments alongside static Programmer values and Playback actions;
- how Record, Update, Cue tracking, Cue-only restoration, release, deletion, renumbering, Save As, revisions, and show migration behave;
- how a Dynamic remains active in the Programmer;
- how a Dynamic is stopped or fixed by later static values; and
- how multiple runtime instances share or isolate state.

## Fixed Values and Stopping Dynamics

The current idea is to introduce a fixed value form that can force an ordinary value and stop or pause an animated value on the same attribute. The tentative command-line token is `FAT`, meaning **Fixed At**.

This is an attribute-value concept, not a whole-Dynamic scope:

- a fixed Intensity value affects the matching Intensity address;
- it does not implicitly stop an unrelated Tilt lane, even if Intensity and Tilt originated in one combined Dynamic;
- fixed values participate in normal source priority and ownership; and
- the fixed behavior lasts only while its source remains active.

The runtime policy for freeze, hidden continue, or restart-on-release remains unresolved.

## Server Requirements

The server must own:

- Dynamic definitions and revisions;
- validation and migration;
- target expressions and fixture/head projection;
- runtime instance identity and lifecycle;
- clocks, sampling, Speed Group synchronization, and phase;
- source priority and LTP ownership;
- Programmer, Preset, Cue, Preload, Update, Clear, release, and fixed-value behavior;
- HTTP/WebSocket/OSC/keyboard/hardware command parity where exposed; and
- deterministic snapshots for output, Fixture Sheet, Stage, and feedback.

The frontend may request edits and display previews, but it must not become the evaluator or source of truth for sampling, phase, priority, or fixture projection.

## Acceptance Coverage

1. Dynamic definitions are portable show objects with stable identity and revision handling.
2. Attribute lanes evaluate independently and never form paired multi-attribute path nodes.
3. Keyframed and center-and-size modes have explicit conversion and validation rules.
4. `Current` has an acyclic upstream-value definition.
5. Speed, phase, transport, and start policies are deterministic and testable.
6. Selection-order and grid-projection phase distribution produce documented fixture phases.
7. Programmer, Preset, Cue, Preload, Update, release, save/reload, and migration semantics are specified before runtime implementation.
8. Fixed values or `FAT` behavior is specified before runtime implementation.
9. Server APIs and feedback expose authoritative state consistently across supported surfaces.
10. Tests inspect authoritative Dynamic contributions and resolved output, not only UI state.
