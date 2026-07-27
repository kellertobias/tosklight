# Dynamics

This is chunk 16 of the overall refactoring effort.


## Status

**DOING.** The product, data, runtime, persistence, control-surface, playback, migration, and acceptance decisions for the first complete Dynamics release are settled in this folder.

The implementation checkpoint is now committed. The plan remains in `doing/` until the required
user visual review, post-acceptance UI automation/screenshots, remaining acceptance gates, and
`## Result` records are complete.

## Committed implementation checkpoint

- `4b5c2f88` — canonical scalar attribute mappings and fixture-schema prerequisite.
- `b3c59791` — authoritative Dynamics runtime, persistence, Programmer/Cue/Preload, playback,
  HTTP/WebSocket/OSC, benchmark, and legacy Phaser removal.
- `160c4ee0` — production Dynamics pool/editor, live controls, encoders, and Storybook review
  surface.

## Product model

Dynamics is ToskLight's animated-value and effect system. The operator-facing family is **Dynamics** and one reusable pool object is a **Dynamic**. A running use is a **Dynamic instance**; do not call it a Dynamic application.

A Dynamic is first-class animated attribute content. It passes through the normal Programmer, Cue, Preload, Playback, priority, ownership, fixture projection, Stage, Fixture Sheet, and output paths. It is not a sidecar effect runner and never writes DMX directly.

Each Dynamic contains one or more independently represented scalar attribute lanes. One instance coordinates their clock, phase distribution, start policy, and Random groups so Intensity, Position, Color components, and other lanes remain synchronized. No lane representation, editor gesture, Preset reference, runtime sample, or output assembly may create a shared multi-attribute path.

The complete first release includes:

- numbered portable Dynamic objects and their pool/editor;
- Programmer use, immediate object editing, Dynamic Off, and `FixAT`;
- Cue tracking, Cue-only, Update, Preload, save/reload, and selective import;
- authoritative server evaluation, spatial phase projection, Speed Groups, Random pulses, pause, transitions, and output;
- Dynamic Playback assignment, faders, buttons, feedback, and auto-off;
- Fixture Sheet and Stage visibility;
- command-line, HTTP/WebSocket, OSC, software, and attached-hardware parity where a surface exposes the operation; and
- deterministic backend, runtime, persistence, API, output, and later UI acceptance coverage.

Ordinary Presets remain static-only in this release. Dynamic lanes may use live scalar values from ordinary Presets, but a Preset cannot contain a Dynamic instance.

## Plans in this folder

- [Open Questions](questions.md) is the single register for unresolved decisions. It currently contains no blocking questions.
- [Dynamics Window](01-dynamics-window.md) defines the pool, production editor, immediate edit workflow, lane controls, target binding, and accepted UI-review boundary.
- [Programmer Dynamics Modal and Encoder](02-programmer-dynamics-modal-and-encoder.md) defines pool use, the Dynamics encoder, Dynamic Off, `FixAT`, command grammar, Fixture Sheet feedback, and Programmer behavior.
- [Runtime and Instance Semantics](03-runtime-and-instance-semantics.md) defines the portable schema, references, instances, evaluation, phase, Random, transitions, tracking, Preload, persistence, API/event boundary, removal of legacy Cue phasers, and backend tests.
- [Playback Assignment](04-playback-assignment.md) defines singleton and independent instances on Playbacks, fader/button choices, speed, pause, auto-off, addressing, and feedback.

## Implementation order

The documents are grouped by operator subsystem; implementation follows this dependency order:

1. Finish the active shared-frontend/Storybook refactor lane before changing its pool, window, modal, encoder, and playback primitives.
2. Complete the canonical attribute-registry portion of `docs/plans/Next/71-attribute-registry-and-activation-groups.md`. Dynamics consumes stable continuous-scalar identity, units, bounds, and fixture mapping; it must not create a private attribute catalog. Activation-group product behavior is not part of Dynamics.
3. Remove the accidental legacy Cue Phaser model, evaluator, writer route, UI helpers, and Phaser-specific tests. Re-save the three canonical SQLite shows after the schema change; none contains a non-empty Phaser.
4. Implement the portable Dynamic model, object intents, compiler projection, runtime service, contribution-batch integration, deterministic clocking, Stage/Fixture Sheet projections, Programmer values, Cue/Preload behavior, persistence, and backend tests.
5. Implement the Dynamics pool and editor against the real server capabilities. The production editor has no embedded fixture preview or browser evaluator.
6. Implement Dynamic Playback assignment and all supported software, virtual, OSC, HTTP/WebSocket, and attached-hardware paths.
7. Build and manually review the complete real UI with the user. Do not add new Dynamics pool/editor UI automation before this explicit visual-acceptance checkpoint.
8. Incorporate accepted UI changes, then add focused UI interaction tests, human-readable scenarios, help/manual updates, screenshots, and the full cross-surface acceptance run.

Backend/domain/persistence/API/output tests are written with the backend implementation. Only Dynamics pool/editor UI automation is deferred until UI acceptance.

## Required architecture

- `DynamicDefinition` is a portable, revisioned show object with a stable UUID and a unique integer pool slot from 1 through 9999.
- Moving or renumbering preserves UUID identity and every reference follows the object. Copying creates a new UUID and deep-copies the definition while retaining live Group and Preset dependencies.
- Editing or replacing an object in place updates live references. Deleting it atomically embeds its last valid definition into every Cue or Playback reference; those references then become self-contained snapshots and never attach to another object placed in the old slot.
- Target-bound Dynamics use one singleton runtime instance. Targetless Dynamics create independent instances per target scope and source.
- The server owns definition validation, target projection, runtime identity, clocks, phase, Random streams, priority, sampling, transitions, Cue tracking, Preload, output contribution, and authoritative feedback.
- Object edits are typed, revisioned, idempotent intent updates. Live instance operations are ordered WebSocket actions with matching plain HTTP action URLs under `docs/engineering/api-rules.md`.
- High-rate phase and sampled-value feedback is bounded telemetry or client interpolation, never one durable semantic event per output frame.
- The production output scheduler consumes immutable Dynamic contribution batches through the prepared engine seam.

## Patent-avoidance boundary

Dynamics must avoid the multi-parameter path method claimed in [US 10,638,583 B1](https://patents.google.com/patent/US10638583B1/en) and [DE 10 2019 107 669 B4](https://patents.google.com/patent/DE102019107669B4/en). This is an engineering constraint, not a legal conclusion.

- Every lane is one scalar attribute over time.
- Do not store, fit, traverse, or sample a shared Pan/Tilt, multi-component Color, Preset-to-Preset, or other multi-attribute path.
- A regular Preset reference resolves to the matching scalar value independently for each lane and target before evaluation.
- Multi-lane selection, shared timestamps, Random groups, synchronized starts, and atomic output assembly are coordination metadata; they do not create combined attribute nodes.

## Settled terminology

- **Dynamic**: one portable pool definition.
- **Dynamic instance**: one running use of a Dynamic definition or embedded snapshot.
- **Dynamic On**: a tracked per-attribute value linking lanes to one instance identity.
- **Dynamic Off**: a targeted tombstone that ends one identified instance across its linked lanes and targets.
- **Current**: the continuously resolved winning ordinary static value before Dynamic and `FixAT` evaluation.
- **Value**: an authored scalar source in the editor. Do not label this source Fixed.
- **FixAT**: command-line text for the Fixed At value kind.
- **FAT**: operator button/help label for Fixed At.
- **Pause**: freezes sampled output and runtime phase.
- **Hidden**: a losing Dynamic continues running without contributing output.
- **Size**: scales authored lane excursion around the lane's mode-native pivot.
- **Master**: the playback output master.
- **Activation mix**: the independent wet/dry transition between the underlying value and a Dynamic instance.

## Global acceptance boundary

The feature is complete only when:

- definitions, dependencies, target bindings, instance identity, Cue tracking, Preload, deletion snapshots, and all supported operations survive save/reload and restart;
- independent scalar lanes and the patent boundary are proven in domain tests;
- Dynamic/FAT priority, `Current`, wet/dry transitions, Size/Master, pause, hidden fallback, Dynamic Off, full-control auto-off, and exact virtual-time sampling are proven against authoritative resolved values and DMX;
- target-bound singleton control stacks and targetless independent instances are proven across Programmer, multiple Cuelists, and multiple Playbacks;
- Stage, Fixture Sheet, running-source feedback, UI, command line, HTTP/WebSocket, OSC, and attached hardware agree;
- malformed definitions, missing targets, missing Preset values, unsupported attributes, deleted references, and stale revisions fail visibly and never retarget silently; and
- the user has accepted the production UI before its detailed automation and screenshots are frozen.
