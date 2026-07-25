# 08a — Cue and Playback recording and runtime

## Outcome

Add the typed recording, Cue editing, concrete Playback runtime, configuration, and normalized
observation surface from parent step 08.

## Public helpers

- `record.playback(number, options?)`;
- `record.cue({ playback, cue, mode, timing })`;
- `cue.update/delete/move/copy/goto/load/select(...)`;
- `playback.go/goBack/on/off/toggle/release(...)`;
- `playback.fader(number, value)`;
- `playback.select(number)`;
- `playback.configure(number, definition)`;
- normalized Cue, Playback, and active-Cue assertions.

Closed product vocabularies use enums. Concrete Playback targets remain stable across page
changes. Recording uses the Programmer helpers from step 06 and never writes raw show objects
from a scenario.

## Helper-contract scenarios

1. Record two Cues, run them, and assert logical DMX at exact clock boundaries.
2. Update, move, copy, and delete a Cue through visible and typed routes.
3. Address a concrete Playback across a page change.
4. Configure Playback buttons and faders through typed definitions.
5. Exercise GO, GO BACK, ON, OFF, TOGGLE, RELEASE, and a fader while retaining Playback/Cue HTP
   semantics.

## Done gate

- Recording and runtime actions have typed, unambiguous concrete targets.
- Scenario files contain no raw show-object writes or transport paths.
- UI and API routes converge on normalized Cue and Playback observations.
