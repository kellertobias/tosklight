# Selecting and Setting Values

The programmer holds temporary selection and attribute values for the current user. Nothing becomes show programming until it is recorded.

> [!danger] Missing graphic
> Add a Programmer-state diagram separating the ordered selection, stored Programmer values, temporary Highlight or Low Light, playback values, and resolved output.

## Select fixtures

Select from Stage, Fixtures, a Group pool, or the command line. Selection is additive across touch surfaces until replaced or explicitly cleared. Use Thru, Plus, Minus, and Division for ordered ranges and subsets. Multi-head fixture IDs expand according to the rules in [Fixtures and Patch](../10-Show-Setup/12-patch-fixtures-and-scenery.md).

## Set values

Use `[AT]` for intensity, recall a Preset, or use the attribute encoders for color, position, beam, and other parameters. For an ordered multi-fixture selection, enter `0 [THRU] 50 [ENT]` after `[AT]`, or enter the same range in an encoder's value modal, to give the first fixture 0%, the last fixture 50%, and every fixture between them an equally interpolated value. Repeated `[THRU]` supplies additional control points: `100 [THRU] 0 [THRU] 100` places every control point on a real fixture of the ordered selection — when an interior point falls exactly between two fixtures, both receive it (six fixtures resolve to `100, 50, 0, 0, 50, 100`) — and the desk rejects a spread with more control points than selected fixtures instead of applying part of it. The encoder modal spreads the attribute shown on that encoder; the command line spreads intensity. Fixture Sheet and Channels show the current value and its source. A stepped control - a shutter, a gobo wheel, a play mode - reads out the position it currently sits in, such as **Shutter open**, rather than the percentage of the channel that position happens to be; a value inside a continuous range, such as a strobe sweep, still reads as a percentage. A selection sitting in different positions reads as **Mixed**. Per-value fade and delay can be entered from the command line.

## Align future encoder changes

**Align** is available beside every standard parameter encoder group. It modifies future relative encoder movement; activating it does not change a fixture value or create an Undo step. Select fixtures in the required order, then choose **Left**, **Right**, **Out**, or **In**. The first logical encoder you move becomes the bound attribute and captures every supported fixture's current value as its anchor.

- **Left** applies none of the signed movement to the first fixture and all of it to the last, interpolating between them.
- **Right** applies all of the movement to the first fixture and none to the last.
- **Out** applies all of the movement at both ends and none at the middle.
- **In** applies none at both ends and all at the middle.

For an even selection, both central fixtures receive the middle weight. With four fixtures, Out is `100, 0, 0, 100` and In is `0, 100, 100, 0`. One fixture receives the complete movement in every mode; with two fixtures, Out gives both the complete movement and In holds both at their anchors. Negative movement uses the same weights in the opposite direction. Each fixture keeps its own starting value, and the attribute's normal limits, wrap behavior, fine/coarse resolution, and profile support remain authoritative.

Choosing another Align mode keeps the result already produced and re-anchors from the current values; only later encoder movement uses the new mode. **Off**, Escape, Clear, Record, Preload, and Preload Go turn Align off without reverting values. Moving a different logical encoder first turns Align off, then applies that encoder movement normally. The desk's software, keyboard/command, OSC, and attached-hardware paths share this state and the same encoder transaction.

## Highlight and Step Through

Highlight is a temporary highest-priority identification look. Select fixtures from Stage, Fixtures, a Group, or the command line, then press **HIGH** or `Alt+H`. The current valid ordered selection becomes the frozen original set: every member uses the default Highlight look of 100% intensity and white. The look is not a Programmer value and is never included by Record, Update, Merge, a Group, a Preset, or a Cue.

- **NEXT** or `Alt+Right` singles out the first member, then advances through the original order. The singled-out fixture stays in Highlight; every other original member uses Low Light at 10% intensity and blue.
- **PREV** or `Alt+Left` singles out the last member, then moves backward through the same order.
- **ALL** or `Alt+A` restores the complete frozen original set as the actual selection and returns every surviving member to Highlight.
- **HIGH** again removes the temporary Highlight and Low Light layers without clearing selection or Programmer values.

NEXT from the last valid item wraps to the first; PREV from the first wraps to the last. Fixtures outside the original set never receive either look. Editing a Group or making another selection while HIGH is active does not silently replace or grow the frozen set. Deleted fixtures are skipped without reordering the remaining members. Multi-head selection has already expanded into selectable heads; multipatched physical copies still share one logical step item.

The actual Programmer selection follows the Highlight focus. In All mode, encoders, Presets, special dialogs, and other value actions write every fixture in the original set. In Step mode they write only the singled-out fixture. The explicit value is retained in the Programmer. For each touched attribute, the real value immediately replaces that part of the temporary look, so it can be seen and tuned; untouched Highlight or Low Light attributes remain temporary. Moving away and back does not discard the explicit value.

The Fixture Sheet keeps the complete original set visible with a subdued treatment and marks the active step prominently. ALL returns every member to complete-selection styling. OSC and attached-hardware feedback report active state, All-versus-Step mode, index, total, and active fixture identity. Software, keyboard, OSC, attached hardware, and native extensions all call the same authoritative Highlight actions and share the repeat guard.

The transient layer is applied after ordinary Programmer, playback, and output resolution. Turning HIGH off therefore reveals the exact underlying winners in the next frame without an intervening default frame. Profile resolution, inversion, physical limits, Grand Master, Blackout, disabled routes, and hazardous safe values remain authoritative. Blind and Preload suppress live Highlight output while leaving its state inspectable.

The desk owns live Highlight output. Every surface shares the authoritative original set, position,
and touched-attribute state, because there is one of each. Saving or reopening a show never
restores live Highlight output or a stale step position.

## Clear and undo

The first `[CLR]` clears selection while leaving programmed values; the blinking Clear state shows that values remain. Press `[CLR]` again to clear the programmer. Undo and redo cover programming mutations such as recording or renaming, not live fader travel or playback execution.

## Multiple users

The desk has one durable Programmer. Every surface writes into it, so a value set on a wing is the
same value the main screen shows and the same value a Record captures — there is nothing to merge.
Record and clear deliberately, and check what is in the Programmer when a playback is also active.

A screen marked Not Editable, and an OSC remote on the remote-control path, can work playbacks
without touching the Programmer at all. See
[Surfaces, Sessions, and Recovery](../10-Show-Setup/05-users-sessions-and-recovery.md).
