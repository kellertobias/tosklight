# Cuelists and tracking

A Cuelist stores ordered Cues. A playback supplies the controls that run it. One Cuelist can be assigned to several playbacks.

## Build a sequence

1. Record the first look into a Cuelist or an empty playback.
2. Change the Programmer for the next look and record another Cue.
3. Clear the Programmer, then run the Cuelist to check its stored and tracked values.

Use `[REC][CUE][CUE] 4 [ENT]` to append to Cuelist 4. It becomes the selected Cuelist; `[REC][CUE][ENT]` appends the following Cue there. To insert Cue 2.1, use `[REC][CUE][CUE] 4 [CUE] 2.1 [ENT]`.

## Tracking and Cue only

A Cue stores changes. Values from an earlier Cue continue through later Cues until another value or Release replaces them. For example, Cue 1 can set a wash to blue and Cue 2 can change only its intensity. Cue 2 still uses the tracked blue.

Hold `[REC]` to open Record Settings and enable **Cue only** for a change that should end at the following Cue. That next Cue restores the previous tracked value, or releases the address if no previous value existed. Turn **Cue only** off for normal tracking records.

**Release** ends the Cuelist's ownership of an attribute. It can reveal another playback or the default value. A recorded 0% intensity continues to own the intensity at zero. See [Release commands](01-command-line.md#releasing-values).

## Edit the Cuelist

Open its Cues from the Cuelist Pool. Edit the name or timing cell directly, and open **Cuelist Settings** for Sequence or Chaser mode, priority, wrap/restart behavior, timing overrides, and renumbering.

Cue numbers are paths. `2`, `2.0`, `2.1`, and `2.1.0` are distinct. Renumbering preserves Cue identities, names, content, and timing; check the displayed order before confirming.

Use [Record and Update](04-programming-cues.md#edit-cue-contents) to change stored values. Update **Tracked** can change an earlier Cue supplying the current look. Inspect the preview before confirming.

The [Cuelist window reference](../30-Windows/03-cues-and-playbacks.md) explains the Cue table, pane following, and settings. [Cue timing](17-cue-timing.md) explains fades and delays; [Cue triggers](13-triggers-chasers-and-speed.md) covers automatic steps.
