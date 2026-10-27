# Cue timing

Open a Cuelist, then touch an In/Out Fade or Delay cell to enter its time. Out Fade offers **Use Release**; Out Delay offers **Use In Fade**. An explicit number makes a linked field independent again.

Record a Cue master fade from the command line with `[REC][CUE][CUE] 4 [CUE] 2.1 [TIME] 3 [ENT]`. This stores Cue 2.1 in Cuelist 4 with a three-second In Fade. [Per-value TIME](01-command-line.md#recording-fade-and-delay-times) stores separate timing on each programmed value.

## Incoming and outgoing timing

When moving from Cue 1 to Cue 2, **Cue 2's In Fade and In Delay** govern incoming changes. **Cue 1's Out Fade and Out Delay** govern decreasing or released Intensity. Editing Cue 2's Out timing changes what happens when you later leave Cue 2; it does not change the fall from Cue 1 into Cue 2. Other attributes use incoming timing.

For example, Cue 1 has one fixture at 80% and Cue 2 takes it to 20%. With Cue 1 Out Delay **0.3 seconds** and Out Fade **1 second**, it holds for 0.3 seconds after GO, then falls over 1 second, provided no stored per-value timing overrides those times. Another fixture rising into Cue 2 can fade at the same time using Cue 2's In timing. Independent delays can also leave a black gap.

## Per-value timing and timing masters

Cue timing is the fallback for values without their own fade or delay. A stored per-value fade or delay takes priority independently: an explicit fade of **0 seconds** snaps rather than inheriting the Cue fade. Enable **Force Cue Timing** in Cuelist Settings when the Cue's timing should govern all values instead. **Disable Cue Timing** is a rehearsal bypass that makes Cue and per-value timing immediate without changing the stored values; it takes priority over Force Cue Timing.

For a sequence Cue whose In Fade is zero, the desk's **Cue Fade** supplies the fallback. Explicit nonzero Cue In Fade remains its own time. Out Fade can link to the desk's **Release** master, and Out Delay can link to that Cue's effective **In Fade**. Linked cells show the source and current time. Returning to explicit timing restores the remembered explicit value. When Out timing has no independent value, it inherits the effective fade/delay fallback. Chasers use their single X-fade percentage timing.

**Programmer Fade** controls programming transitions and supplies the GO-time transition for queued Programmer values in Preload. It does not globally override ordinary Cue playback. Preloaded playback actions keep explicit Cue timing; without explicit timing they use the Programmer Fade captured at Preload GO. See [Preload](12-preload.md).

## Choose when to advance

Manual GO, FOLLOW, TIME, timecode, and Link determine when playback moves. FOLLOW waits for the latest actual incoming or outgoing work to settle, then adds the follow delay stored on the next Cue. TIME counts from the preceding Cue's start. [Triggers, Chasers, and Speed Groups](13-triggers-chasers-and-speed.md) gives a worked example.

Link is stored on its source Cue and jumps to a destination Cue by stable identity after actual completion plus its optional delay. Renumbering changes the displayed destination number without changing the Link. Missing destinations, self-links, and Link cycles are rejected before the show changes. Pause freezes a running transition. Releasing a whole playback removes its ownership immediately; Cue Out timing does not fade the release of the entire playback.

For timed timeline editing, see [Timecode](35-timecode-cue-editing.md). The [command reference](01-command-line.md#cue-timing-and-triggers) gives complete keypad forms.

Timecode uses the same Cue times. Cue Out starts at the next Cue handover; a final Cue's Out timing does not run after clip end. Use the separate **Clip Out** gain fade for the clip ending. [Cue timing on the timeline](35-timecode-cue-editing.md) explains the grips, Trigger placement, and Hold/Release behavior.
