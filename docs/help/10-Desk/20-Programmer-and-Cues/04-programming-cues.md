# Programming Cues

A Cue stores what is currently in the programmer. Values that are merely visible from a playback, defaults, Highlight, or resolved output are not recorded. A Cue is one stored step inside a Cuelist; a playback is a control that may be assigned to that Cuelist, not the Cuelist itself.

## Record the first Cue

1. Select fixtures and build the intended look in the programmer.
2. Press `[REC]` and choose a Cuelist/playback target, or enter an explicit Cue address.
3. Name the Cue and set fade, delay, and trigger behavior in Cuelist View.
4. Clear the programmer and run the Cue to prove that the stored data is sufficient.

> [!danger] Missing graphic
> Add a Cue lifecycle diagram showing Programmer values being recorded into a Cue, the Cue inside a Cuelist, assignment to a playback, and playback output after the Programmer is cleared.

Recording onto an empty Cuelist creates its first Cue without assigning it to a playback. Recording onto an empty playback creates a Cuelist, records its first Cue, and assigns it. On touch, the whole visible playback is the Record target. On an attached desk, only its topmost playback button is the target; if no button is assigned, use the visible on-screen section. The fader is never a Record target, so it remains usable while programming.

When a target Cuelist contains exactly one Cue, the desk asks whether to **Add Cue**, **Merge Cue**, or **Overwrite Cue**. Once it contains two or more Cues, recording onto the Cuelist or its playback always appends a new Cue.

From the command line, the default target is a Cuelist. `[REC][CUE][CUE] <Cuelist-number> [ENT]` appends there and selects it; later use `[REC][CUE][ENT]` to append to the selected Cuelist. `[REC][PBK] <playback-number> [ENT]` records through a physical playback, and `[REC][PBK][PBK] <virtual-playback-number> [ENT]` records through a Virtual Playback. A playback number without a dot uses the current page; `<page>.<playback>` selects an explicit page.

To choose the Cue number, use `[REC][CUE][CUE] <Cuelist-number> [CUE] <Cue-number> [ENT]`, or omit the Cuelist address for the selected Cuelist. An existing Cue is overwritten; an unused number is inserted. Cue numbers are paths, not decimals: `2`, `2.0`, `2.1`, `2.1.0`, and `2.2` are all distinct and sort in that order.

## Edit Cue contents

Normal Record overwrites the addressed Cue, `[REC][+]` merges programmer values, and `[REC][-]` removes their fixture/attribute addresses. Copy, Move, and Delete use explicit addresses. Renumbering and edits are protected as one show mutation; check the final Cue order before proceeding.

Use `[^REC]` to update existing programming. **Update** changes values stored directly in the current Cue; **Tracked** changes the earlier Cue supplying each tracked value; **Known** writes values into the current Cue only when their addresses already occur somewhere in the Cuelist; **All** also introduces new addresses. The command forms are plain `[^REC]`, `[^REC][-]`, `[^REC][+][+]`, and `[^REC][+]` respectively. Press `[^REC]` twice while holding Shift, then release Shift, to open the complete Update modal. The preview explains eligible, ignored, source-Cue, and destination results before confirmation. See [Updating Existing Programming](01-command-line.md#updating-existing-programming) for exact targets and Preset updates.

For a temporary change, hold `[REC]` to open **Record Settings** and enable **Cue only** before recording. The following Cue automatically restores each Cue-only address to its previous tracked value, or releases an address that had no earlier value. Turn **Cue only** off again for ordinary tracking records. The setting and generated restoration data survive a show refresh or reopen.

## Timing and triggers

Cue **In Fade** and **In Delay** provide the existing timing fallbacks. For decreasing or released Intensity, **Out Fade** and **Out Delay** can retain explicit independent values. Out Fade can instead link to the desk's **Release** timing master, and Out Delay can link to that Cue's effective **In Fade**. A linked cell names its source and current effective time; changing the source updates the transition immediately. Returning to explicit timing restores the Cue's remembered explicit value. Individual values can retain their own fade and start delay and remain authoritative unless **Force Cue Timing** is enabled. **Disable Cue Timing** snaps both directions. Chasers retain their single X-fade percentage timing. Manual GO, Follow, timed delay, timecode, and Link triggers determine where and when playback moves. Follow timing begins after the latest incoming or outgoing work settles. Link is stored on its source Cue and jumps to a destination Cue by stable identity after that same actual completion point plus the optional Link delay. Renumbering therefore changes the displayed destination number without changing the Link. Missing destinations, self-links, and Link cycles are rejected before the show changes. Pause freezes a running transition; releasing a whole playback still removes its ownership immediately rather than applying Cue out timing to the whole playback.

When a Cue appears inside a Cuelist clip in the Timecode editor it is drawn as two stacked bands: the out timing above the in timing, each a hollow delay block followed by its solid fade block, so one Cue reads as a single stepped shape. The boundary between a delay and its fade sets the delay, and the far edge of the fade sets its duration. These handles edit the same Cuelist-owned timing values. The Timecode clip stores its placement and playback behavior, not a duplicate of the Cue timing. A saved drag is therefore visible immediately in Cuelist View, and a later Cuelist edit changes the ranges shown in Timecode. Linked timing remains linked unless the operator explicitly chooses an independent value.

For exact commands and edge cases, see [Command Line Reference](01-command-line.md). For execution semantics, see [Cues and Playbacks](10-cues-and-playbacks.md).

![Cuelist Cue table and playback execution surface](../../assets/screenshots/cuelist-playback.png)

The clip itself is dragged by the handle across its top third, which carries the Cuelist name and its playback number. Dragging either end of that handle scales the clip: the Cue delays and fades inside it, and any placed transition points, all stretch or compress in proportion, so a section keeps its shape at a new length. With a clip selected, **Prev Cue** and **Next Cue** step through the Cues it spans, and the encoders address the selected Cue as In delay, In fade, Out delay, Out fade, with the Cue selection beside them.
