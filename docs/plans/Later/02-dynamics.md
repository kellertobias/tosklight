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
- A Dynamic may contain one attribute lane or several. A combined Intensity-and-Tilt Dynamic is only a container for independent Intensity and Tilt lanes; it must not create a shared multi-attribute path. Each attribute is resolved independently through the normal fixture-or-logical-head plus attribute address.
- Lanes may eventually use individual values extracted from Presets as keyframes, bounds, start/end values, or other typed scalar sources and may expose modulation, phase, speed, width, spatial-ordering, and parameter overrides. A combined Preset must be decomposed at the lane boundary; it must not become a paired or multi-attribute path node.
- Where spatial ordering is required, the Stage or another ordering provider may supply an ordered target projection. The Dynamic runtime must not depend directly on a UI window or renderer.

### Patent-avoidance boundary: independent attributes over time

Dynamics must be designed to avoid the multi-parameter path method claimed in [US 10,638,583 B1](https://patents.google.com/patent/US10638583B1/en) and its German counterpart [DE 10 2019 107 669 B4](https://patents.google.com/patent/DE102019107669B4/en). This is an engineering constraint for the planned design, not a legal conclusion about the patents' ultimate scope or validity.

- Do not establish nodes that each contain two or more fixture-attribute values and calculate one effect-function curve connecting those nodes in a plane whose axes are fixture parameters.
- Do not model a multi-step effect as one path through combined Position, Color, or other multi-attribute Presets. In particular, do not fit, store, traverse, or sample a shared Pan-and-Tilt, color-component, or other multi-attribute curve.
- Resolve every Preset reference to the scalar value for one lane before effect evaluation. Each lane then evaluates only its own attribute function, bounds, phase, timing, and steps.
- Combining independently evaluated lane values into the fixture contribution or output frame is required output assembly; it must not introduce a shared curve, paired control point, or multi-attribute path representation.

A lane may support multiple keyframes or steps. Its editor may plot time on the x-axis and that one attribute's scalar value on the y-axis, and the operator may choose the interpolation or easing curve between successive keyframes. Time is the independent variable, not a second fixture parameter. The resulting curve determines only the output value of that one attribute.

When a Dynamic contains several lanes, each lane owns and tracks its own ordered keyframes, interpolation choices, and current segment. Coincident keyframe times may be shown or edited together for operator convenience, but they remain independent scalar keyframes; they must not become one combined node or one curve through multiple attributes. A requested multi-step, multi-attribute effect must therefore be represented as parallel per-attribute step sequences or as successive per-attribute effect segments, even when this requires the runtime and editor to retain separate step state for every participating attribute.

### Attribute-lane value modes

Every lane selects one of two value-generation modes. The mode is per attribute, so lanes in the same Dynamic may use different modes.

#### Keyframed mode

Keyframed mode defines a cyclic sequence of scalar landmarks for one attribute:

- A keyframe source may be a literal value, the matching scalar value extracted from a Preset, or `Current`.
- The first keyframe is also the loop-closing value. The editor may display a terminal keyframe, but it is an alias of the first keyframe rather than an independently editable value.
- Selecting two sources creates the default three-keyframe cycle `A at 0%`, `B at 50%`, and the loop-closing `A at 100%`.
- The operator may insert additional keyframes. Each inserted keyframe belongs only to that attribute lane and has an explicit normalized time within the cycle.
- Each adjacent keyframe pair has its own interpolation or easing shape. The default is a sinusoidal ease. The planned catalog includes at least sine-like, cosine-like, linear rising, linear falling, and other explicitly defined easing shapes.
- A lane editor may display the scalar keyframes with cycle time or normalized phase on the x-axis and the one attribute value on the y-axis. Bezier-style handles or named easing choices may shape a scalar time/value segment, but they must never create a path in a plane whose two axes are fixture attributes.

Keyframes are the extrema or other scalar turning values of the lane. A multi-step Dynamic containing several attributes is not one shared step list: it is a collection of per-attribute keyframe sequences. The runtime must retain the current segment and interpolation state separately for every lane.

#### Center-and-size mode

Center-and-size mode evaluates a cyclic function around a scalar center:

- `center` may be a literal value, the matching scalar value extracted from a Preset, or `Current`;
- `size` defines the lane's scalar excursion or amplitude, with its exact absolute-versus-relative unit policy still to be specified; and
- `shape` selects the normalized cycle function, initially including sine, cosine, linear rising, linear falling, and pulse-width modulation.

All shapes must share one outer lane configuration so the operator can switch shape without rebuilding the lane, but shape-specific parameters remain typed. Pulse-width modulation requires normalized `attack`, `on`, `decay`, and `off` portions. Their total must describe one complete cycle. Before implementation, a deterministic conversion table must define how common values and shape-specific parameters are initialized, preserved, or translated when switching between sine, cosine, ramps, and pulse-width modulation.

The future editor may allow a lane to switch between keyframed and center-and-size mode. The conversion must be explicit and reversible where possible; no implementation may silently discard keyframes or shape parameters.

### `Current`, composition, and recursion

`Current` means the fixture's static value for that attribute immediately before the evaluated Dynamic is applied. It is a reference to an upstream value, not an output readback after the same Dynamic has contributed.

Multiple Dynamics, tracked Cue content, Programmer values, and Playback ownership can make that upstream value ambiguous. The eventual evaluation graph must be acyclic and must define:

- which static winner forms `Current` for a Dynamic instance;
- whether a higher-priority or later LTP Dynamic may modulate the result of an earlier Dynamic;
- whether several Dynamics on one attribute replace, add, multiply, or otherwise compose;
- how LTP timestamps and ownership apply to Dynamic assignments;
- what happens when a source using `Current` wins, releases, or changes priority; and
- how dependency cycles are detected and reported instead of recursively feeding a Dynamic's output back into itself.

LTP-style resolution is the current direction, but it is not yet sufficiently defined to implement. A Dynamic must never derive its own `Current` source from its already-evaluated output.

### Cycle speed and transport synchronization

Each Dynamic has one stable speed source for the lifetime of its runtime instance:

- A fixed duration specifies seconds per complete cycle. The tentative minimum duration is `50 ms`, equivalent to `20 Hz`. At a `40 Hz` output rate this yields only two output samples per cycle, so sampling quality, aliasing, clamping, and operator warnings require explicit acceptance criteria.
- Alternatively, a Dynamic references one Speed Group. A Speed Group supplies a shared tempo and transport, expressed initially as BPM plus a monotonically advancing tick or fractional-beat position.
- The Dynamic has an overall rational speed multiplier. Each attribute lane may additionally have its own rational multiplier. Required choices include multiplication and division by at least `2`, `3`, and `4`, so an effect may run, for example, once per beat, once per three beats, or once per four-beat bar.
- Linked Speed Groups must share an authoritative transport epoch so all Dynamics using the same or linked groups remain phase-stable and synchronous.

Three start policies are required:

1. **Start now, local epoch**: begin at phase zero immediately and remain synchronized to an epoch created at that start. A ten-tick cycle started on tick 1 returns to phase zero on ticks 11, 21, 31, and so on; one started on tick 3 returns on ticks 13, 23, 33, and so on.
2. **Join synchronized position now**: become active immediately at the phase the Dynamic would have had if it had started on the previous qualifying transport boundary.
3. **Start on next synchronized boundary**: wait until the next qualifying beat or bar boundary, then become active at phase zero.

The eventual operator labels for these policies remain open; implementation must not conflate phase alignment with delayed activation.

Bar synchronization belongs to the Speed Group transport. The Speed Group must eventually define meter, at least beats per bar and a stable bar-one origin. A Dynamic then needs a quantization unit, cycle length in beats or bars, and an optional start beat within the bar. This must support starting on a selected beat rather than only on beat one while preserving synchronization across linked Speed Groups.

### Fixture phase distribution

Fixture phase is an offset inside the Dynamic cycle. It is independent of the Speed Group transport position, activation quantization, and bar/beat start policy.

- Every fixture in the ordered target projection receives a phase in degrees or an equivalent normalized fraction.
- Manual entry and THRU-style distribution must be supported. For cyclic endpoint-exclusive spreading, `0 THRU 360` over four fixtures yields `0, 90, 180, 270`; the unassigned interval from the last fixture back to the first equals every other interval.
- A mirrored entry such as `0 THRU 360 THRU 0` must be able to produce `0, 90, 180, 270, 270, 180, 90, 0` over eight fixtures. Exact rounding and behavior for even, odd, and single-fixture selections require acceptance examples.
- Phase editing should be available from the encoder workflow, including tapping an encoder to enter an explicit spread expression.

The helper terminology should avoid the already-established show-object term `Group`:

- **Block size** is the number of adjacent fixtures sharing one phase. With block size 4, fixtures 1-4 form one block and fixtures 5-8 the next. With block size 3, an eight-fixture selection produces blocks 1-3, 4-6, and the shorter final block 7-8.
- **Repeats** is the number of times the same phase pattern is repeated across the ordered selection. With two repeats over eight fixtures, fixtures 1-4 and 5-8 receive equivalent four-fixture patterns.
- **Wings** mirrors the phase order within each repeat so symmetrical selections can run inward or outward. The initial scope is wings off or on; generalized wing counts remain deferred.

Blocks, repeats, and wings are phase-assignment helpers. They do not merge fixtures, change the authoritative selected-fixture order, or create multi-attribute value paths.

### Selection and grid ordering

Phase distribution may use either the authoritative selection order or a Stage/grid projection:

- **Selection order** uses the ordered fixtures exactly as selected or supplied by a live Group expression.
- **Grid linear** projects fixture positions onto a gradient at an operator-selected angle. Fixtures at the same projected position receive the same phase.
- **Grid radial out** distributes from the chosen center toward the perimeter.
- **Grid radial in** reverses the radial distribution.

The grid is used only to order or group fixtures for phase assignment. It must not become the patented adjustment-range plane and must not use two fixture attributes as axes for a value path.

Grid/selection spreading also needs a phase offset and a phase span. The intended `50%` span behavior is that the active spread occupies half of the cycle and the remaining portion holds the last value until wraparound, but the precise distinction between phase span, an active-time window, waveform width, and pulse-width duty cycle is unresolved and must be specified with timelines before implementation.

It may eventually be useful to link pulse width or another shape parameter across lanes or fixtures. Parameter linking, its ownership, and cycle prevention are explicitly deferred product questions.

### Applying and storing Dynamics

- A Dynamic is a preset-like animated-value object, but whether it has its own numbered pool and can be loaded independently from ordinary Presets remains an explicit product decision.
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
14. Per-attribute keyframe and step representation, editing, segment state, and synchronization without introducing paired multi-attribute nodes or a shared path.
15. The exact value and parameter mapping when switching lane mode or waveform, including PWM attack, on, decay, and off timing.
16. The upstream-value, priority, LTP, composition, dependency, and recursion rules for `Current` and multiple Dynamics on one attribute.
17. Fixed-duration limits, sampling behavior, Speed Group transport, linked-group epochs, multipliers, activation policies, meter, bar origin, and beat/bar quantization.
18. Phase-spread expressions and rounding, plus the final names and exact interaction between block size, repeats, wings, offset, and span.
19. Selection-order and grid-projection behavior, including live target changes, ties, missing positions, center selection, linear angles, and radial direction.
20. Whether Dynamics have an independent pool and load workflow, and how Dynamic definitions relate to Presets that contain Dynamic values.
21. Whether shape parameters such as pulse width can be linked, how such links resolve across lanes or fixtures, and how dependency cycles are prevented.

## Gate for future implementation

Do not turn this document into code merely because the architecture can support it. Before implementation, the user must deliberately revise the open sections into a decision-complete specification, add explicit acceptance scenarios, remove the caution at the top, and mark the file **IMPLEMENTABLE**.
