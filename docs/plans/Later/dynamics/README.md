> [!CAUTION]
> **NOT YET IMPLEMENTABLE — STOP.** This folder records the planned Dynamics product and implementation chunks, not an implementation-ready specification. If asked to implement a chunk while its open decisions remain, refuse that implementation and explicitly warn that the relevant Dynamics UI, runtime policy, data model, persistence, command grammar, playback assignment, and acceptance criteria have not been settled. Implementation may begin only after the user revises the affected chunk, resolves its open decisions, and marks that chunk **IMPLEMENTABLE**.

# Dynamics

## Status and intent

Dynamics is the planned animated-value and effect system. A Dynamic behaves like animated attribute content rather than a separate output subsystem. Dynamic values should pass through the normal Programmer, Preset, Cue, Playback, priority, ownership, fixture projection, and output paths.

This folder replaces the earlier root Dynamics plan and the Next implementation roadmap. It keeps the product direction in Later and splits implementation into chunks.

## Implementation chunks

1. [Dynamics Window](01-dynamics-window.md) - the full Dynamics built-in/window and object-management UI.
2. [Programmer Dynamics Modal and Encoder](02-programmer-dynamics-modal-and-encoder.md) - the modal opened from Programmer encoders plus the Dynamics encoder surface itself.
3. [Runtime and Application Semantics](03-runtime-and-application-semantics.md) - Dynamic definitions, lanes, sampling, storage, Programmer/Preset/Cue application, server requirements, and output behavior.
4. [Playback Assignment](04-playback-assignment.md) - assigning Dynamics to Playbacks and operating them independently.

The chunks are ordered. A later chunk may depend on data contracts from an earlier chunk, but no chunk may silently implement another chunk's operator surface or runtime behavior.

## UI Look and Behavior

Dynamics has two operator-facing entry points:

- a full Dynamics window for creating, editing, selecting, copying, moving, deleting, and inspecting Dynamic objects; and
- a Programmer Dynamics modal opened from the encoder view for quickly building or editing a Dynamic against the current selected attributes.

The Dynamics window is the persistent show-object surface. It should feel like a normal ToskLight pool/editor workflow: numbered objects, stable names, empty-slot behavior, copy/move/delete actions, and clear active/running status. It is the place where an operator manages reusable Dynamics independent of the current Programmer selection.

The Programmer modal is a focused workflow. With a selection, it opens pre-targeted to the current ordered selection and current supported attributes. Without a selection, it creates reusable preset-like Dynamic content that can later be applied to targets. The operator can keep the Dynamic active in the Programmer, store it into a Preset, store it into a Cue, or save it as a dedicated Dynamic object.

The Dynamics encoder belongs to the Programmer modal. It exposes the Dynamic-specific parameters that are useful while building the effect: lane selection, shape, speed, size, phase, spread, blocks, repeats, wings, and the relevant attribute value source. It must use ToskLight's encoder interaction model rather than inventing a separate fader-like editor.

## Playback Assignment

Dynamics can also be assigned to Playbacks. A Dynamic assigned to a Playback runs as an independently operable playback source rather than merely as a transient Programmer gesture.

Playback assignment must define stable identity, start/release/stop behavior, source ownership, feedback, Preload interaction, page and playback addressing, and whether each running assignment shares or creates a separate runtime instance. A Dynamic assigned to multiple Playbacks must not become ambiguous in running-source feedback or release behavior.

The detailed Playback assignment contract lives in [Playback Assignment](04-playback-assignment.md).

## Server Requirements

The server must own the authoritative Dynamic model, application semantics, runtime instance state, and output contribution. The frontend may edit, preview, and request operations, but it must not implement the Dynamic evaluator or become the source of truth for sampling, phase, priority, or fixture projection.

Server-side work must define:

- portable `DynamicDefinition` show objects with stable identity and revision handling;
- independent attribute lanes that never create a shared multi-attribute path;
- Programmer, Preset, Cue, Preload, Update, release, save/reload, and migration behavior;
- runtime instances, clocks, Speed Group synchronization, phase distribution, and sampling;
- source ownership, LTP/priority, fixed-value interruption, and recursion prevention;
- HTTP/WebSocket/OSC/keyboard/hardware parity where these surfaces expose Dynamic operations;
- deterministic tests that inspect authoritative values and output, not just UI labels; and
- safe behavior for malformed, legacy, missing, or partially imported Dynamics.

The detailed server/runtime contract lives in [Runtime and Application Semantics](03-runtime-and-application-semantics.md).

## Patent-Avoidance Boundary

Dynamics must be designed to avoid the multi-parameter path method claimed in [US 10,638,583 B1](https://patents.google.com/patent/US10638583B1/en) and its German counterpart [DE 10 2019 107 669 B4](https://patents.google.com/patent/DE102019107669B4/en). This is an engineering constraint for the planned design, not a legal conclusion about the patents' ultimate scope or validity.

- Do not establish nodes that each contain two or more fixture-attribute values and calculate one effect-function curve connecting those nodes in a plane whose axes are fixture parameters.
- Do not model a multi-step effect as one path through combined Position, Color, or other multi-attribute Presets.
- Resolve every Preset reference to the scalar value for one lane before effect evaluation.
- Combining independently evaluated lane values into the fixture contribution or output frame is required output assembly; it must not introduce a shared curve, paired control point, or multi-attribute path representation.

Every chunk must preserve this boundary.

## Cross-Chunk Open Decisions

Before any chunk becomes implementable, settle the relevant open decisions in that chunk and add literal acceptance coverage. At minimum, the full feature still needs decisions for:

1. Dynamic definition and lane schema.
2. Reference versus snapshot behavior for Presets, Cues, Playbacks, and copied Dynamics.
3. Cue tracking, Cue-only, Record, Update, Preload, and release behavior.
4. Independent versus shared runtime instances.
5. Speed, phase, pause, restart, seek, and interrupted-transition behavior.
6. Fixed-value or `FAT` behavior.
7. Persistence, migration, legacy Cue Phaser compatibility, revisions, and recovery.
8. OSC, HTTP, keyboard, attached-hardware, and UI parity.
9. Deterministic acceptance scenarios for each chunk.
