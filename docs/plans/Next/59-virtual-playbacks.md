# Virtual Playbacks

## Status

**Specification only.** This plan records future playback-runtime and UI behavior. It does not implement playback storage, runtime state, UI changes, API behavior, OSC behavior, or executable tests.

## Goal

Define virtual playbacks as real playback runtime objects that behave like ordinary playbacks without taking over or shifting the physical desk's playback positions.

Virtual playbacks are not independent from the desk state: they still participate in playback arbitration, release, cue timing, output ownership, and feedback. They should, however, have their own addressing model so they do not overlap with the playback buttons and faders shown on actual desks.

## Required behavior

- decide which virtual playbacks are shown in the UI and why;
- give virtual playbacks their own page and numbering scheme, or an equivalent namespace, so they cannot collide with physical playback positions;
- keep virtual playback runtime behavior compatible with regular playbacks, including GO, stop, release, fade, restart, and state feedback;
- make current-page physical playback addressing distinct from virtual playback addressing;
- ensure Preload Go can start or stage virtual playbacks without creating sources that cannot be turned off; and
- expose virtual playback state consistently in software UI, command/API, OSC, and attached hardware feedback where applicable.

## Acceptance coverage

1. Virtual playback identity never changes the visible physical playback position on the current desk page.
2. Starting, releasing, and stopping a virtual playback follows the same runtime semantics as a regular playback.
3. Virtual playback page and number labels are unambiguous to the operator.
4. Physical playback page changes do not accidentally retarget virtual playbacks.
5. Active virtual playbacks appear in running-source feedback and can be stopped deliberately.
