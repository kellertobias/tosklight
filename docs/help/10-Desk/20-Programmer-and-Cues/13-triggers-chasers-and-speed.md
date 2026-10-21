# Triggers, Chasers, and Speed Groups

## Follow and timed triggers

The trigger belongs to the destination Cue. To make Cue 2 start automatically after Cue 1, edit **Cue 2**, choose **FOLLOW** or **TIME**, and enter its **Trigger Time**. Trigger Time is the automatic wait; it is separate from **In Delay**, which holds a value after the Cue starts and before its fade begins.

**FOLLOW** starts its wait after the preceding Cue's latest actual incoming or outgoing transition has finished, including applicable value delays and fades. A zero follow delay starts the next Cue as soon as that work settles. **TIME** starts its wait when the preceding Cue starts; it can interrupt a transition still in progress.

For example, suppose Cue 1 starts at 0 seconds and all its transitions finish at 3 seconds. Set Cue 2 to **FOLLOW**, Trigger Time **2 seconds**: Cue 2 starts at 5 seconds. Change Cue 2 to **TIME**, Trigger Time **2 seconds**: it starts at 2 seconds, while Cue 1 is still fading. If outgoing Intensity instead finishes at 4 seconds, FOLLOW starts Cue 2 at 6 seconds; TIME still starts it at 2 seconds. These examples assume normal timing is enabled and no pause, manual interruption, Link, or authoritative timecode intervenes.

Confirm pause, release, loop, and end-of-list behavior for every automatic sequence.

## Link triggers

LINK belongs to the source Cue. After that Cue's latest real incoming or outgoing value has finished, the stored Link delay runs and playback jumps to the destination Cue. The destination is stored by stable Cue identity, so renumbering is safe. The desk rejects missing destinations, self-links, and direct or indirect Link cycles instead of guessing a fallback Cue. An explicitly loaded Cue remains the displayed effective next Cue and is retained for the next manual GO.

**Disable Cue Timing** treats both Cue completion and Link delay as zero without rewriting either value. A live configured Timecode source remains authoritative and suppresses Link execution; this prevents a held timecode frame and a Link from repeatedly moving playback back and forth. Link resumes when authoritative timecode is absent.

## Timecode

Choose exactly one Timecode source under **Desk Setup > General**: the desk's internal generator or one exact external source identity. There is no automatic priority list. Select whether loss of the chosen external source continues on the internal clock, pauses running Timecodes, or stops them; the selected source takes authority again as soon as it returns. The desk Timecode rate can follow the DMX rate or use an explicit whole-frame rate. A known incoming rate is converted and reported; an unusable sample is rejected rather than guessed.

The same Setup section selects the server audio-output device and its signed latency trim. Rehearse loss and return with the exact adapter and device used for the show; visible frames alone do not prove that the intended source or audible output is synchronized.

![Configured timecode source and loss policy](../../assets/screenshots/workflows/desk-setup-timecode.png)

## Chasers and speed groups

Chaser mode steps through its Cues using the configured interval or assigned Speed Group. Speed Groups A-E can be set numerically, tapped, or synchronized. Directly setting or tapping either synchronized group breaks that relationship and returns it to independent control.

Use `[^TIME]` for the `SPD GRP` command-line workflow documented in [Command Line Reference](01-command-line.md). A Cuelist, Playback, or Dynamic can be assigned to a Speed Group by addressing that object, pressing `[AT]`, addressing the Speed Group, and pressing `[ENT]`.

## Sound to Light

1. Choose the microphone under **Desk Setup → Network & Inputs → Inputs** and grant permission.
   This browser remembers it for the desk; it does not travel with the show.
2. Switch the lower section to Playbacks. Hold, Shift-click or right-click Speed Group **A–E**
   for its settings. An ordinary tap performs tap tempo instead.
3. Choose source **Sound to Light**, analysis **Tempo / BPM**, then a preset frequency band or
   custom range (20–20,000 Hz).
4. Watch input/band meters and adjust gain. Set confidence threshold, smoothing and min/max tempo.
5. Choose the Sound speed ratio (0.125×–8×), Signal-loss hold, then **Apply** in the title.

![Selected Sound-to-Light audio input and granted capture permission](../../assets/screenshots/workflows/desk-setup-sound-input.png)

The live panel reports permission/capture state, usable signal, detected BPM, confidence and the
effective speed. If the signal disappears or becomes unreliable, the group holds its accepted rate
for **Signal-loss hold**, then returns to its stored manual BPM. Disabling Sound to Light also
returns to manual. A direct BPM command or Learn tap deliberately takes manual ownership.

**÷2**, **×2** and **Pause/Resume** act immediately. Pause holds phase without forgetting the rate.
The Speed Master scales the Sound ratio afterward. Closing edited settings offers Save, Discard
or Stay.

A Speed Group may instead use **Manual** or follow another **Speed Group**. Cycles are rejected.
Tap tempo starts a new calculation after a gap exceeding 10 seconds, or 30 seconds below 10 BPM;
the old rate stays until enough new taps arrive.
