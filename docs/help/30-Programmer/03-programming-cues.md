# Programming Cues

A Cue stores sparse changes inside a Cuelist. Unchanged values track from earlier Cues unless cue-only behavior restores them.

## Record the first Cue

1. Select fixtures and build the intended look in the programmer.
2. Press `[REC]` and choose a Cuelist/playback target, or enter an explicit Cue address.
3. Name the Cue and set fade, delay, and trigger behavior in Cuelist View.
4. Clear the programmer and run the Cue to prove that the stored data is sufficient.

Recording a Cuelist in its pool does not assign it to a playback. Assign it explicitly from playback configuration.

## Edit Cue contents

Normal Record overwrites the applicable stored data, Record Plus merges programmer values, and Record Minus subtracts them. Copy, Move, and Delete use explicit addresses. Renumbering and edits are protected as one show mutation; check the final Cue order before proceeding.

For a temporary change, hold `[REC]` to open **Record Settings** and enable **Cue only** before recording. The following Cue automatically restores each Cue-only address to its previous tracked value, or releases an address that had no earlier value. Turn **Cue only** off again for ordinary tracking records. The setting and generated restoration data survive a show refresh or reopen.

## Timing and triggers

Cue master Fade and Delay provide fallbacks. Individual values can retain their own fade and start delay. Manual GO, Follow, timed delay, and timecode triggers determine when the next Cue runs. Pause freezes a running transition; release removes playback ownership according to its configured behavior.

For exact commands and edge cases, see [Command Line Reference](01-command-line.md). For execution semantics, see [Cues and Playbacks](../40-Running-a-Show/01-cues-and-playbacks.md).

![Cuelist Cue table and playback execution surface](../assets/screenshots/cuelist-playback.png)
