# Live Cue Preset References

TL-685 design and implementation checkpoint, 2026-10-08. Source and compatibility regressions pass; packaged UI, OSC and emitted-output acceptance remain pending.

## Problem and compatibility

Preset recall currently materializes values into the programmer. Cue recording stores those values. Updating the source Preset cannot change the Cue because no retained source identity exists. This behavior is explicitly documented in `fixture-independent-programming.md`, so the release requirement introduces new persisted semantics rather than repairing an existing reference resolver.

Do not infer links by matching values, and do not rewrite historical literal Cue values when a Preset changes. Equal literals may have been authored independently. Existing Cues without reference metadata must retain their exact behavior.

## Proposed minimal representation

Keep every existing CueChange and GroupCueChange materialized value as a fallback. Add an optional reference identifying the stable stored Preset object, its source owner (fixture, Group or universal), and the source attribute. References belong to individual addresses, not to whole Cues. Cue timing, programmer order, fixture isolation and Group membership semantics remain owned by the Cue.

Track provenance in the programmer only when an actual Preset recall supplies an address. Explicit value edits, Release, Clear and subsequent literal writes detach that address. Capture provenance together with values under the existing programmer mutation gate. Normal, blind/preload pending and active capture paths must receive the same treatment. Record Merge preserves untouched references; literal Merge or Update detaches only replaced addresses.

The persisted reference format uses feature contract 2, separately from the contract-1 semantic value format. Literal-only Shows remain contract 1; authored references require 2. An older runtime refuses unsupported live-reference semantics. A newer writer never downgrades a previously higher marker. Immutable Preset instance identities are seeded once in portable migration and persist across edits and Move; Copy creates a fresh identity.

## Resolution and active playback

Resolve a reference against one coherent current Preset catalog before Cue tracking and contribution evaluation. Resolve the correct source owner and attribute, not any matching value in the Preset. Apply existing semantic Color, Position and Zoom values through current fixture profiles; do not resolve and persist DMX bytes.

A missing Preset or address retains the recorded materialized fallback and produces actionable local dependency information. Deleting or moving a Preset must preserve this predictable behavior and must not silently bind the reference to a newly created unrelated object at the same pool number.

A committed source Preset change invalidates only dependent Cuelist tracks, including already-running Cues. It must not invoke GO, reset Cue timers, disturb unrelated contributions, or require programmer values to remain present. Snapshot/reopen compilation must perform the same resolution. Preset edit transactions must not partially update resolved output if persistence fails.

## Command prerequisite

Unify parsing of documented named family addresses for Record and Update while preserving existing numeric addresses. The real single shifted family key currently produces `INTENSITY` or `COLOR`; double family produces `INTENSITY PRESET` or `COLOR PRESET`. Record accepts numeric dotted addresses only. Update requires the PRESET word. Mixed family `ALL` must remain distinguishable from Update All mode.

Use the existing typed Preset recording service for Record Overwrite/Merge. Do not add an unrelated legacy storage path. Preset Subtract is a separate deliberate implementation, not a Merge alias, and is outside this initial live-reference patch unless explicitly included.

## Required verification

- Legacy literal Shows and unknown fields survive load/save unchanged; old runtime rejects the new capability marker.
- Actual recall, first-cue Record, Clear, replay, source Record Merge and source Update produce current Preset values on Art-Net and sACN.
- Source edits change already-running Cue output without GO or time reset.
- Explicit edits detach only their addresses; unrelated fixture/attribute values stay unchanged.
- Normal, live Group, frozen fixture, blind/preload and supported OSC paths share provenance semantics.
- Save/reopen preserves references, fallback and stable source identity.
- Fixture replacement preserves semantic color/aim where the existing identity-preserving replacement contract allows it.
- Delete/move/recreate source cases cannot accidentally rebind a reference; failed transactions preserve Cue/programmer/output state.

## Shared-file coordination

The concurrent solo-fade repair owns playback contribution sampling and native Color transition interpolation. Cue model/recording metadata and coherent Preset dependency compilation have been integrated in the source checkpoint, with current-cue refresh preserving activation rather than invoking GO. Local source warnings read existing authoritative show objects and wait for collection readiness. Actual operator and protocol verification is recorded separately in ignored acceptance evidence; this document is not release approval.
