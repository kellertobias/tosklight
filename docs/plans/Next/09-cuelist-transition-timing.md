> [!CAUTION]
> **NOT YET IMPLEMENTABLE — STOP.** This is a prerequisite product contract for
> Timecode. Its transition model, tracking interaction, runtime arbitration,
> editing, and acceptance criteria must be settled before implementation.

# Cuelist Transition Timing

This plan precedes [Timecode](10-timecode.md). It makes automated Cuelist
transitions sufficiently explicit for reliable Timecode playback.

## Required model

Cue transitions need independent timing for Intensity entering and leaving a
look. Entering Intensity uses an in-delay and in-fade. Leaving Intensity may
hold for an out-delay, then use an independent out-fade. This must allow
intentional overlap, delayed release, and an implicit black gap between scenes.
For example, an outgoing moonlight can remain after a warm full-stage look
begins, then fade on its own schedule.

This is not limited to manual operation: the timing is part of Cue completion.
A following Cue begins only after the latest-running work of its predecessor
has completed, including independent delays and fades.

## Timecode dependency

Timecode Cuelist clips must use these transition semantics deterministically.
The plan must define which attributes use separate out timing, how that timing
interacts with tracking, cue-only versus state starts, releases, manual GO,
Follow, Link, and multiple simultaneous actions. It must also define operator
controls, stored data, undo/redo, and regression scenarios for overlap and
black gaps.
