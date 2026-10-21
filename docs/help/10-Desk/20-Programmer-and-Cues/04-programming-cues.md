# Programming Cues

A Cue stores Programmer values, not merely the look visible from playback, defaults or Highlight.
A Cuelist holds the sequence; its assigned Playback runs it.

## Record the first Cue

1. Select fixtures and build the intended look in the programmer.
2. Press `[REC]` and choose a Cuelist/playback target, or enter an explicit Cue address.
3. Name the Cue and set fade, delay, and trigger behavior in Cuelist View.
4. Clear the programmer and run the Cue to prove that the stored data is sufficient.

![Programmer, Cue recording and playback verification](../../assets/diagrams/cue-lifecycle.svg)

Recording onto an empty Cuelist creates its first Cue without assigning it to a playback. Recording onto an empty playback creates a Cuelist, records its first Cue, and assigns it. On touch, the whole visible playback is the Record target. On an attached desk, only its topmost playback button is the target; if no button is assigned, use the visible on-screen section. The fader is never a Record target, so it remains usable while programming.

When a target Cuelist contains exactly one Cue, the desk asks whether to **Add Cue**, **Merge Cue**, or **Overwrite Cue**. Once it contains two or more Cues, recording onto the Cuelist or its playback always appends a new Cue. This is the **Smart** Record behaviour.

Press `[REC]` twice to choose how this Record stores the programmer: **Smart**, **Merge** (into the Cue the playback is on, programmer values win), **Add Existing** (only what that Cue does not store yet), or **Add Cue** (always a new Cue). Press title-bar **Record** to use the choice once, or turn on **Set as default** first to make it the desk's Record default for every following plain `[REC]`. Choose **Smart** with **Set as default** on to return to the regular behaviour. See [Choosing How to Record](01-command-line.md#choosing-how-to-record-rec-rec).

For explicit Cuelist/Cue addresses, insertion and page-aware playback commands, see
[Recording Cues](01-command-line.md#recording-cues). Cue numbers are paths, not decimals:
`2`, `2.0`, `2.1`, `2.1.0` and `2.2` are distinct and sort in that order.

## Edit Cue contents

Normal Record overwrites the addressed Cue, `[REC][+]` merges programmer values, and `[REC][-]` removes their fixture/attribute addresses. Copy, Move, and Delete use explicit addresses. Check Cue order after renumbering or moving Cues.

Use **Update** for existing programming. Review its target preview before confirmation:

| Mode | Destination |
| --- | --- |
| **Update** | Values stored directly in the current Cue. |
| **Tracked** | The earlier Cue supplying each tracked value. |
| **Known** | Current Cue, only for addresses already used in the Cuelist. |
| **All** | Current Cue, including new addresses. |

See [Updating Existing Programming](01-command-line.md#updating-existing-programming) for commands
and the target chooser.

For a temporary change, hold `[REC]` to open **Record Settings** and enable **Cue only** before recording. The following Cue automatically restores each Cue-only address to its previous tracked value, or releases an address that had no earlier value. Turn **Cue only** off again for ordinary tracking records. The setting and restoration data survive reopening. Record Settings saves immediately;
**Done** closes it. **Record default** chooses the operation used by later plain REC presses.

## Timing and triggers

### Which Cue owns the timing?

When moving from Cue 1 to Cue 2, **Cue 2's In Fade and In Delay** govern incoming changes. **Cue 1's Out Fade and Out Delay** govern decreasing or released Intensity. Editing Cue 2's Out timing changes what happens when you later leave Cue 2; it does not change the fall from Cue 1 into Cue 2. Other attributes use incoming timing.

For example, Cue 1 has one fixture at 80% and Cue 2 takes it to 20%. With Cue 1 Out Delay **0.3 seconds** and Out Fade **1 second**, it holds for 0.3 seconds after GO, then falls over 1 second, provided no stored per-value timing overrides those times. Another fixture rising into Cue 2 can fade at the same time using Cue 2's In timing. Independent delays can also leave a black gap.

### Per-value timing and timing masters

Cue timing is the fallback for values without their own fade or delay. A stored per-value fade or delay takes priority independently: an explicit fade of **0 seconds** snaps rather than inheriting the Cue fade. Enable **Force Cue Timing** in Cuelist Settings when the Cue's timing should govern all values instead. **Disable Cue Timing** is a rehearsal bypass that makes Cue and per-value timing immediate without changing the stored values; it takes priority over Force Cue Timing.

For a sequence Cue whose In Fade is zero, the desk's **Cue Fade** supplies the fallback. Explicit nonzero Cue In Fade remains its own time. Out Fade can link to the desk's **Release** master, and Out Delay can link to that Cue's effective **In Fade**. Linked cells show the source and current time. Returning to explicit timing restores the remembered explicit value. When Out timing has no independent value, it inherits the effective fade/delay fallback. Chasers use their single X-fade percentage timing.

**Programmer Fade** controls programming transitions and supplies the GO-time transition for queued Programmer values in Preload. It does not globally override ordinary Cue playback. Preloaded playback actions keep explicit Cue timing; without explicit timing they use the Programmer Fade captured at Preload GO. See [Preload](12-preload.md).

### Choose when to advance

Manual GO, FOLLOW, TIME, timecode, and Link determine when playback moves. FOLLOW waits for the latest actual incoming or outgoing work to settle, then adds the follow delay stored on the next Cue. TIME counts from the preceding Cue's start. [Triggers, Chasers, and Speed Groups](13-triggers-chasers-and-speed.md) gives a worked example.

Link is stored on its source Cue and jumps to a destination Cue by stable identity after actual completion plus its optional delay. Renumbering changes the displayed destination number without changing the Link. Missing destinations, self-links, and Link cycles are rejected before the show changes. Pause freezes a running transition. Releasing a whole playback removes its ownership immediately; Cue Out timing does not fade the release of the entire playback.

When a Cue appears inside a Cuelist clip in the Timecode editor it is drawn as two stacked bands: the out timing above the in timing, each a hollow delay block followed by its solid fade block, so one Cue reads as a single stepped shape. The boundary between a delay and its fade sets the delay, and the far edge of the fade sets its duration. These handles edit the same Cuelist-owned timing values. The Timecode clip stores its placement and playback behavior, not a duplicate of the Cue timing. A saved drag is therefore visible immediately in Cuelist View, and a later Cuelist edit changes the ranges shown in Timecode. Linked timing remains linked unless the operator explicitly chooses an independent value.

For exact commands and edge cases, see [Command Line Reference](01-command-line.md). For execution semantics, see [Cues and Playbacks](10-cues-and-playbacks.md).

![Cuelist Cue table and playback execution surface](../../assets/screenshots/cuelist-playback.png)

The clip itself is dragged by the handle across its top third, which carries the Cuelist name and its playback number. Dragging either end of that handle scales the clip: the Cue delays and fades inside it, and any placed transition points, all stretch or compress in proportion, so a section keeps its shape at a new length. With a clip selected, **Prev Cue** and **Next Cue** step through the Cues it spans, and the encoders address the selected Cue as In delay, In fade, Out delay, Out fade, with the Cue selection beside them.
