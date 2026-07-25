---
slug: recording-and-live-references
title: "Recording and Live References: Groups, Presets, and Cues"
components: [programmer, backend, engine, control-ui]
order: 32
---

# Recording and Live References: Groups, Presets, and Cues

Operator contract: `docs/help/20-Show-Setup/05-groups-and-presets.md`,
`docs/help/30-Programmer/03-programming-cues.md`, and
`docs/help/30-Programmer/02-selecting-and-setting-values.md`. GROUP, PROG, CUE, and UPDATE
acceptance lives in `tests/01-foundational-dimmers-and-groups.spec.ts`,
`tests/cueSemanticContracts/`, and `tests/updateHighlight/`.

The Programmer is per-user scratch state. Record and Update are the explicit boundary where part of
that state becomes portable show data.

## Starting authority

`light-programmer::ProgrammerRegistry` owns ordered selection, semantic values, timing, modes,
Preload, and mutation-only history. Highlight is a separate transient overlay and is never included
in a record candidate.

Group recording enters `crates/light/src/programming/service/group_recording.rs`; Preset and
Cue paths sit beside it under `programming/service/`. Each action carries desk/user/session source,
request identity, and the show revision it observed.

## Embedded values and live references

An embedded value stores the resolved fixture/head/attribute assignment. A live Group or Preset
reference stores stable object identity plus the semantic assignment, so later membership or value
edits can change recall.

DEGRP/dereference/freeze replaces a live Group expression with its current ordered members. That
choice affects spread recall and must happen before the record transaction is built.

## Candidate and commit

Capability code resolves the target, update mode, filters, dependencies, and exact owned delta.
`crates/light/src/programming/*_active_show.rs` prepares the lossless object change and sends
it through `ActiveShowService`. Concurrent additions are assigned by the server at execution time;
stale revisions repair and reapply instead of force-overwriting.

The same transaction updates the portable object, compiles runtime state, installs one generation,
publishes one semantic event, and adds the recording step to Programmer undo. Ordinary navigation
never enters undo history.

## Recall and Update

Group/Preset recall returns to the Programmer authority. Cue playback contributes through Playback
runtime. Update analyzes which live objects own the current values and presents explicit targets;
it does not rewrite every object visible on stage.

`crates/light/src/programming/update/` owns target analysis and typed changes. The UI
presentation is a workflow over that authority, not a second record algorithm.

## Failure path

A revision conflict changes nothing. The surface refreshes the narrow object authority and lets the
deliberate operator action reapply. Replaying the same request ID returns the stored outcome without
duplicating an object, history entry, audit record, or event.

## Exercise

Read `tests/cueSemanticContracts/recordingScenarios.ts` and
`tests/updateHighlight/updateGroupScenario.ts`. Identify which assertions distinguish an embedded
value, a live reference, and Highlight exclusion.
