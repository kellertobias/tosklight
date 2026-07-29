---
slug: playback-runtime
title: "Playback Runtime: Cues, Masters, Speed, and Arbitration"
components: [backend, engine, control-ui, programmer]
order: 36
---

# Playback Runtime: Cues, Masters, Speed, and Arbitration

Operator contract: `docs/help/40-Running-a-Show/01-cues-and-playbacks.md`,
`docs/help/40-Running-a-Show/02-htp-ltp-and-ownership.md`, and
`docs/help/40-Running-a-Show/04-triggers-chasers-and-speed.md`. PBK-001 through PBK-006, CUE, MIB,
and playback-selection acceptance lives under `tests/07-playback-configuration.spec.ts`,
`tests/playbackConfiguration/`, and `tests/02-cues-tracking-and-arbitration.spec.ts`.

A Playback is a control-surface slot assigned to a Cuelist. Its portable assignment and its running
Cue/transition are different lifetimes.

## Addressing

Software, OSC, attached hardware, and HTTP adapters resolve either current-page Playback N or
explicit Page P / Playback N. `crates/light/src/playback/command.rs` preserves that distinction
through the service boundary. A page change retargets only current-page addressing.

Physical Playbacks use page-relative numbers 1–1000. Dedicated Virtual Playbacks use
stable show-wide numbers in 300-control banks: page 1 is 1001–1300, page 2 is
1301–1600, and page `p` starts at `1001 + 300 × (p - 1)`. The explicit page qualifier
is validated against the number's bank. Follow Main and Pinned panes only project
those identities; desks do not own or copy their assignment or runtime.

## Controls and runtime

`crates/light/domain/playback/src/controls/` owns GO, pause, resume, back, GOTO, Load, On, Off, Flash, Temp, Swap,
fader, and X-fade semantics. `crates/light/domain/playback/src/runtime/` owns active/loaded Cue, transitions,
Chaser phase, and replaceable telemetry.

Manual controls and `crates/light/domain/playback/src/automatic.rs` produce the same semantic transition.
`crates/light/src/playback/event.rs` publishes after the domain lock is released.

Preload captures the stable Virtual Playback number and validated page qualifier, then
commits its ordered actions through one atomic batch. Exclusion zones store playback
numbers directly and release the deduplicated union of numbered peers inside the same
serialized action. Activation provenance retains the source desk for audit, while
restart normalization applies the same show-owned zones regardless of which desk
accepted the winning action.

## Contributions and arbitration

`crates/light/domain/playback/src/contribution/` turns tracked Cue state into fixture/head/attribute
contributions. The engine merges them with Programmer, Preload, Highlight, and other sources:

- intensity normally uses HTP;
- non-intensity lanes use LTP/ownership;
- Programmer authority remains separate from Playback arbitration;
- independent overlapping Group Masters use the highest applicable active level;
- Grand Master applies once above resolved Group Masters;
- Blackout forces the global intensity result to zero; and
- Speed Groups scale timing without becoming value masters.

Move in Black and fades are transition policies over the reconstructed Cue state. See
[Cue Tracking and Goto](tour:cue-tracking-and-goto) for the stored/tracked side.

## Projection and feedback

`crates/light/src/playback/projection.rs` provides immutable runtime state. Typed event
routes let a visible Cuelist or Playback subscribe narrowly; high-rate fader progress uses bounded
telemetry rather than one lossless event per render sample. The frontend implementation starts in
`apps/light-desktop/src/features/playbackRuntime/`.

OSC feedback maps the same projection to the frozen hardware indices. It never reads browser-local
selection or guesses that a command succeeded.

## Failure path

Deleting or remapping the active object reconciles runtime in the same active-show transaction.
Exclusion-zone activation releases peers atomically and reports related projections. A reconnect
gap installs the Playback snapshot before incremental events resume.

## Exercise

Read `tests/playbackConfiguration/pbk006GroupMasterHtp.ts`. Write the expected level for a fixture
belonging to two assigned Group Masters plus the Grand Master, then follow the assertion through the
engine result.
