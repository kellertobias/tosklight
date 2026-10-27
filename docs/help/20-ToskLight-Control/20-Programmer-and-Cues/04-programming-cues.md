# Cue recording

## Quick summary

1. Build a look in the Programmer, record it into a Cue, then press **CLR twice** (first clears selection, second clears Programmer values) and run that Cue.
2. Set **In Fade/Delay** on the Cue being entered. Set **Out Fade/Delay** on the Cue being left for decreasing or released Intensity.
3. Check [Cue timing](17-cue-timing.md) before rehearsing fades.
4. For automatic steps, choose [Cue triggers](13-triggers-chasers-and-speed.md).

A Cue stores what is currently in the programmer. Values that are merely visible from a playback, defaults, Highlight, or resolved output are not recorded. A Cue is one stored step inside a Cuelist; a playback is a control that may be assigned to that Cuelist, not the Cuelist itself.

## Record the first Cue

1. Select fixtures and build the intended look in the programmer.
2. Press `[REC]` and choose a Cuelist/playback target, or enter an explicit Cue address.
3. Name the Cue and set fade, delay, and trigger behavior in Cuelist View.
4. Clear the programmer and run the Cue to prove that the stored data is sufficient.

Recording onto an empty Cuelist creates its first Cue without assigning it to a playback. Recording onto an empty playback creates a Cuelist, records its first Cue, and assigns it. On touch, the whole visible playback is the Record target. On an attached desk, only its topmost playback button is the target; if no button is assigned, use the visible on-screen section. The fader is never a Record target, so it remains usable while programming.

When a target Cuelist contains exactly one Cue, the desk asks whether to **Add Cue**, **Merge Cue**, or **Overwrite Cue**. Once it contains two or more Cues, recording onto the Cuelist or its playback always appends a new Cue. This is the **Smart** Record behaviour.

Press `[REC]` twice to choose how this Record stores the programmer: **Smart**, **Merge** (into the Cue the playback is on, programmer values win), **Add Existing** (only what that Cue does not store yet), or **Add Cue** (always a new Cue). Press title-bar **Record** to use the choice once, or turn on **Set as default** first to make it the desk's Record default for every following plain `[REC]`. Choose **Smart** with **Set as default** on to return to the regular behaviour. See [Choosing How to Record](01-command-line.md#choosing-how-to-record-rec-rec).

From the command line, `[REC][CUE][CUE] <Cuelist-number> [ENT]` appends to that Cuelist and selects it. `[REC][CUE][ENT]` appends the next Cue there. `[REC][PBK] <playback-number> [ENT]` records through the selected surface's effective page. Use `<page>.<playback>` for an explicit address. Every surface uses the same Playback assignments; repeated `[PBK]` presses keep PBK.

To choose the Cue number, use `[REC][CUE][CUE] <Cuelist-number> [CUE] <Cue-number> [ENT]`, or omit the Cuelist address for the selected Cuelist. An existing Cue is overwritten; an unused number is inserted. Cue numbers are paths, not decimals: `2`, `2.0`, `2.1`, `2.1.0`, and `2.2` are all distinct and sort in that order.

## Edit Cue contents

Normal Record overwrites the addressed Cue, `[REC][+]` merges programmer values, and `[REC][-]` removes their fixture/attribute addresses. Copy, Move, and Delete use explicit addresses. Renumbering and edits are protected as one show mutation; check the final Cue order before proceeding.

Use `[^REC]` to update existing programming. **Update** changes values stored directly in the current Cue; **Tracked** changes the earlier Cue supplying each tracked value; **Known** writes values into the current Cue only when their addresses already occur somewhere in the Cuelist; **All** also introduces new addresses. The command forms are plain `[^REC]`, `[^REC][-]`, `[^REC][+][+]`, and `[^REC][+]` respectively. Press `[^REC]` twice while holding Shift, then release Shift, to choose how to update: **Smart**, **Merge** (All), **Add Existing** (Known), or **Add Cue** (a new Cue in the touched Cuelist), once or as the desk's Update default. Its title-bar **Targets** opens the complete Update Targets modal. The preview explains eligible, ignored, source-Cue, and destination results before confirmation. See [Updating Existing Programming](01-command-line.md#updating-existing-programming) for exact targets and Preset updates.

For a temporary change, hold `[REC]` to open **Record Settings** and enable **Cue only** before recording. The following Cue automatically restores each Cue-only address to its previous tracked value, or releases an address that had no earlier value. Turn **Cue only** off again for ordinary tracking records. The setting and generated restoration data survive a show refresh or reopen. Record Settings saves each change immediately; press title-bar **Done** to close it. Each option explains its effect below its label: **Record default** is the desk's stored Record choice (**Smart**, **Merge**, **Add Existing**, or **Add Cue**) that every plain `[REC]` uses, and **Cue only** is kept on this screen.

Continue with [Cuelists and tracking](16-cuelists-and-tracking.md), [Cue timing](17-cue-timing.md), and [playback operation](10-cues-and-playbacks.md).
