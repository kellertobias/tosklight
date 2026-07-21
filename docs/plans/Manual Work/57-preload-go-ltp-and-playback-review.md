# Preload Go LTP and Playback Review

## Status

**Manual review and specification only.** This plan records a future operator-behavior audit. It does not implement preload behavior, playback behavior, API behavior, UI changes, OSC behavior, or executable tests.

## Goal

Verify that Preload Go behaves correctly when the staged content includes ordinary programmer LTP values, cue-like virtual scenes, real playbacks, and virtual playbacks.

The review must prove that Preload Go is not merely correct for programmer values. It must also cover playback-owned output, virtual playback runtime identity, and the later release or turn-off path for anything that Preload Go started.

## Review scope

- define the exact LTP ownership and timestamp behavior when Preload Go commits staged programmer values;
- define how a staged virtual scene or virtual cue differs from starting a playback;
- test whether staged playbacks can be turned off again without leaving hidden runtime ownership;
- test how virtual playbacks started by Preload Go release, fade out, restart, and interact with ordinary playback buttons;
- compare current output, next output, Follow Preload, Highlight, Grand Master, Blackout, and cue timing during the handoff; and
- identify any behavior that belongs in the runtime model rather than in the UI.

## Completion criteria

1. Programmer-only Preload Go, cue-like Preload Go, real-playback Preload Go, and virtual-playback Preload Go each have a written expected behavior.
2. Every started source has a clear release, stop, or ownership-replacement path.
3. LTP behavior is proven with competing programmer, cue, playback, and virtual-playback sources.
4. UI, command/API, OSC, and attached hardware surfaces describe the same active and staged state.
5. Any implementation work discovered by this review is split into focused plans before coding begins.
