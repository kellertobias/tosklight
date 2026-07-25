# Playback Assignment

## Status

**Specification only.** This chunk records how Dynamics can be assigned to Playbacks. It does not implement playback storage, runtime state, UI changes, API behavior, OSC behavior, or executable tests.

## Goal

Allow a Dynamic to be assigned to a Playback and run independently from Programmer editing.

This is the fourth chunk because it depends on the Dynamic object model and runtime instance semantics from the earlier chunks.

## Assignment Model

A Playback assignment references a Dynamic by stable identity or stores a copy/snapshot according to the runtime chunk's final reference policy. The operator-facing assignment must make that policy clear enough that updating or deleting the original Dynamic cannot surprise the show.

Assigned Dynamics participate in the same playback arbitration, release, source ownership, feedback, and output paths as other playback content. They are not an external effect runner hidden beside the normal playback engine.

## Playback Operation

The implementation must define:

- what GO, release, stop, restart, load, and flash mean for a Dynamic assignment;
- whether a Dynamic starts immediately, joins synchronized transport, or waits for the next boundary;
- how the assigned Dynamic reports active, pending, released, failed, or blocked state;
- how multiple Playbacks assigned to the same Dynamic share or isolate runtime instances;
- how a Dynamic assignment appears in running-source feedback;
- how Preload can stage or start Dynamic assignments;
- how playback page changes and explicit playback addressing interact with Dynamic assignments; and
- how unsupported or invalid Dynamic content blocks assignment or execution.

Dynamic playback assignment must also account for the future [Playback Auto-Off Behavior](../../Next/75-playback-auto-off-behavior.md) policy. Before Dynamic playbacks support those settings, this chunk must define whether fader-zero and Flash-release auto-off stop, pause, release, or hide the Dynamic runtime instance.

## Playback UI and Feedback

Playback tiles, faders, buttons, command/API responses, OSC feedback, and attached hardware feedback must identify a Dynamic assignment unambiguously.

The operator must be able to stop or release a running Dynamic deliberately. A Dynamic started from a Playback must not create a source that can only be cleared by restarting the show.

## Acceptance Coverage

1. A Dynamic can be assigned to a Playback by stable identity or the final documented copy/snapshot policy.
2. Starting, releasing, stopping, and restarting the assigned Dynamic follow documented playback semantics.
3. Assigned Dynamics appear in running-source feedback and can be stopped deliberately.
4. Multiple assignments of the same Dynamic have documented shared-versus-independent runtime behavior.
5. Preload can stage Dynamic playback actions without creating unreleaseable sources.
6. Page and playback addressing remain unambiguous.
7. Invalid, deleted, or unsupported Dynamics do not silently retarget to another object.
8. Dynamic playback auto-off behavior is either explicitly unsupported or fully defined for fader-zero and Flash-release triggers.
