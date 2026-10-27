# Selecting and Setting Values

The programmer holds temporary selection and attribute values for the desk. Nothing becomes show programming until it is recorded.

## Select fixtures

Select from Stage, Fixtures, a Group pool, or the command line. Selection is additive across touch surfaces until replaced or explicitly cleared. Use Thru, Plus, Minus, and Division for ordered ranges and subsets. Multi-head fixture IDs expand according to the rules in [Fixtures and Patch](../../10-ToskLight-Architect/02-Fixtures-and-Patch/02-add-and-patch-fixtures.md).

## Set values

Use `[AT]` for intensity, recall a Preset, or use the attribute encoders for color, position, beam, and other parameters. For an ordered multi-fixture selection, enter `0 [THRU] 50 [ENT]` after `[AT]`, or enter the same range in an encoder's value modal, to give the first fixture 0%, the last fixture 50%, and every fixture between them an equally interpolated value. Repeated `[THRU]` supplies additional control points: `100 [THRU] 0 [THRU] 100` places every control point on a real fixture of the ordered selection, when an interior point falls exactly between two fixtures, both receive it (six fixtures resolve to `100, 50, 0, 0, 50, 100`), and the desk rejects a spread with more control points than selected fixtures instead of applying part of it. The encoder modal spreads the attribute shown on that encoder, including a Pan or Tilt angle, a Color component such as White Blend, Focus or Zoom; the command line spreads intensity, and after `[^2]`, `[^3]` or `[^7]` the Color, Pan/Tilt or Focus/Zoom values (see [Command Line](01-command-line.md#setting-non-intensity-values)). Fixture Sheet and Channels show the current value and its source. A stepped control - a shutter, a gobo wheel, a play mode - reads out the position it currently sits in, such as **Shutter open**, rather than the percentage of the channel that position happens to be; a value inside a continuous range, such as a strobe sweep, still reads as a percentage. A selection sitting in different positions reads as **Mixed**. Per-value fade and delay can be entered from the command line.

Position encoders work in degrees and metres: page 1 is Pan and Tilt, page 2 is Point and the X, Y and Z offsets. Switching pages never changes the active Angle or Target. The first Point, X, Y or Z edit activates Target together with its offset; the first Pan or Tilt edit while Target is active takes over the resolved pose you were shown. See [Programming Windows](08-position.md#position-encoders-and-special-dialog) for the Position Special Dialog.

## Align future encoder changes

Use [Align](26-align.md) to fan future relative encoder movement across the ordered selection. Use `[THRU]` for an absolute value spread.

## Highlight and Step Through

Use [Highlight and step through](27-highlight-and-step.md) to identify fixtures with HIGH, PREV, NEXT, and ALL, then adjust the active fixture. The identification look stays separate from recorded values.

## Clear and undo

The first `[CLR]` clears selection while leaving programmed values; the blinking Clear state shows that values remain. Press `[CLR]` again to clear the programmer. Undo and redo cover programming mutations such as recording or renaming, not live fader travel or playback execution.

## Multiple users

The desk has one durable Programmer. Every surface writes into it, so a value set on a wing is the
same value the main screen shows and the same value a Record captures, there is nothing to merge.
Record and clear deliberately, and check what is in the Programmer when a playback is also active.

A screen marked Not Editable, and an OSC remote on the remote-control path, can work playbacks
without touching the Programmer at all. See
[Surfaces, Sessions, and Recovery](../10-Show-Setup/05-users-sessions-and-recovery.md).
