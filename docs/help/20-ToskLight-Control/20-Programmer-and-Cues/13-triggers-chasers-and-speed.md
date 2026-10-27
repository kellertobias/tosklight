# Triggers, Chasers, and Speed Groups

## Quick summary

- **GO** waits for you to advance the Cuelist.
- **FOLLOW** waits for the current Cue's transition to finish, then waits its follow delay before starting the next Cue.
- **TIME** counts from the current Cue's start, so the next Cue may start before that transition finishes.
- Set FOLLOW or TIME and its **Trigger Time** on the **Cue you want to start automatically**. A Link is different: it belongs to the Cue you are leaving.
- Rehearse automatic sequences with the production pause, looping, speed, and timecode settings.

Cuelists can advance manually, after a follow delay, at a timed delay, through a Link, or from configured timecode.

## Follow and timed triggers

The trigger belongs to the destination Cue. To make Cue 2 start automatically after Cue 1, edit **Cue 2**, choose **FOLLOW** or **TIME**, and enter its **Trigger Time**. Trigger Time is the automatic wait; it is separate from **In Delay**, which holds a value after the Cue starts and before its fade begins.

**FOLLOW** starts its wait after the preceding Cue's latest actual incoming or outgoing transition has finished, including applicable value delays and fades. A zero follow delay starts the next Cue as soon as that work settles. **TIME** starts its wait when the preceding Cue starts; it can interrupt a transition still in progress.

For example, suppose Cue 1 starts at 0 seconds and all its transitions finish at 3 seconds. Set Cue 2 to **FOLLOW**, Trigger Time **2 seconds**: Cue 2 starts at 5 seconds. Change Cue 2 to **TIME**, Trigger Time **2 seconds**: it starts at 2 seconds, while Cue 1 is still fading. If outgoing Intensity instead finishes at 4 seconds, FOLLOW starts Cue 2 at 6 seconds; TIME still starts it at 2 seconds. These examples assume normal timing is enabled and no pause, manual interruption, Link, or authoritative timecode intervenes.

Confirm pause, release, loop, and end-of-list behavior for every automatic sequence.

## Link triggers

LINK belongs to the source Cue. After that Cue's latest real incoming or outgoing value has finished, the stored Link delay runs and playback jumps to the destination Cue. The destination is stored by stable Cue identity, so renumbering is safe. The desk rejects missing destinations, self-links, and direct or indirect Link cycles instead of guessing a fallback Cue. An explicitly loaded Cue remains the displayed effective next Cue and is retained for the next manual GO.

**Disable Cue Timing** treats both Cue completion and Link delay as zero without rewriting either value. A live configured Timecode source remains authoritative and suppresses Link execution; this prevents a held timecode frame and a Link from repeatedly moving playback back and forth. Link resumes when authoritative timecode is absent.

## Timecode

Use the [Timecode editor](18-timecode.md) to build a timed show. Its chapter also covers source selection, frame rate, source loss, and audio synchronization.

## Chasers and speed groups

Chaser mode steps through its Cues using the configured interval or assigned Speed Group. Speed Groups A-E can be set numerically, tapped, or synchronized. Directly setting or tapping either synchronized group breaks that relationship and returns it to independent control.

Use `[^TIME]` for the `SPD GRP` command-line workflow documented in [Command Line Reference](01-command-line.md). A Cuelist, Playback, or Dynamic can be assigned to a Speed Group by addressing that object, pressing `[AT]`, addressing the Speed Group, and pressing `[ENT]`.

For microphone-driven tempo, see [Sound to Light](21-sound-to-light.md).
