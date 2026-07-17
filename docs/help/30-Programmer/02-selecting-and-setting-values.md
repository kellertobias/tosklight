# Selecting and Setting Values

The programmer holds temporary selection and attribute values for the current user. Nothing becomes show programming until it is recorded.

## Select fixtures

Select from Stage, Fixtures, a Group pool, or the command line. Selection is additive across touch surfaces until replaced or explicitly cleared. Use Thru, Plus, Minus, and Division for ordered ranges and subsets. Multi-head fixture IDs expand according to the rules in [Fixtures and Patch](../20-Show-Setup/01-fixtures-and-patch.md).

## Set values

Use `[AT]` for intensity, recall a Preset, or use the attribute encoders for color, position, beam, and other parameters. Fixture Sheet and Channels show the current value and its source. Per-value fade and delay can be entered from the command line.

## Highlight and Step Through

Highlight is a temporary identification output. Select fixtures from Stage, Fixtures, or a Group, then use the controls beside the command line:

- **HLT** or `Alt+H` turns Highlight on or off. Turning it on captures the current ordered selection when no selection has already been captured, and initially identifies every captured fixture.
- **CAP** or `Alt+C` deliberately replaces the remembered selection with the current ordered selection. Duplicate fixture references are retained only once, in their first authoritative selection position.
- **Previous** and **Next**, or `Alt+Left` and `Alt+Right`, enter Step mode and make one remembered fixture the working selection. The status shows the one-based step position, total, fixture number, and name.

Step Through stops at both ends; it never wraps. A disabled Previous or Next button and the status message show that the start or end has been reached. If a remembered fixture is removed from the show, it is skipped without reordering the remaining fixtures.

The complete captured selection is separate from the working selection. Moving Position, Color, Beam, or another attribute on the current fixture does not shorten or replace that capture, including while Next and Previous move through several fixtures. Only **CAP** replaces it. Turning HLT off removes the temporary output, resets the active step, and reveals the programmer or playback underneath; the remembered selection remains available until it is replaced or the desk/user context ends.

Highlight output is never a programmer value and is not included by Record, Update, a Preset, or a Cue. Values you deliberately program while stepping remain in the programmer after HLT is turned off. For a highlighted profile channel, its exact Highlight raw value temporarily replaces the normally resolved programmer/playback value regardless of their normal priority; turning HLT off reveals that underlying winner in the next frame. Highlight snaps on and off without a programmed fade, while channel inversion, raw resolution/limits, virtual-intensity behavior, and the channel's configured sequence, Group, and Grand Master reactions still apply exactly once.

A multipatched fixture remains one logical Highlight item: selecting its parent identifies every physical copy without adding a step for each copy. A compound fixture's logical heads are separate items when those heads are selected; selecting the physical parent identifies the complete fixture. Overlapping Groups and duplicate selection members contribute only their first occurrence, preserving the desk's authoritative selection order. Unpatched fixtures still remain in the remembered selection and can be stepped like any other show fixture, although they emit no DMX until patched. A fixture with no Intensity channel also participates: it identifies through any deliberately configured safe Highlight raw look, such as an open shutter or emitter value. If none of its profile channels has a useful Highlight value, stepping remains valid but that fixture produces no useful identification emission.

Blackout always suppresses intensity and configured color-system emission; hazardous fixtures use their explicit safe values after Highlight, so Highlight cannot defeat that safety state. A disabled output route remains disabled. There is no separate parked-output source in the current desk model: Highlight neither creates nor clears parked data. Entering Blind, Preview, or Preload suppresses an already-live Highlight immediately; the controls show **CAPTURE ONLY**, so the selection and step can be prepared but no live Highlight output is emitted.

One user owns live Highlight output on a desk at a time. Other sessions for that same user and desk share the authoritative state. Software, OSC, and attached hardware also share one 150 ms repeat guard, so one physical press cannot advance the step twice through two adapters. If the last remembered fixture becomes invalid, its stale live-output ownership is released immediately. A different user can prepare a capture in Blind/Preview, but cannot silently take over live output; the controls identify the current owner and show an actionable error.

## Clear and undo

The first `[CLR]` clears selection while leaving programmed values; the blinking Clear state shows that values remain. Press `[CLR]` again to clear the programmer. Undo and redo cover programming mutations such as recording or renaming, not live fader travel or playback execution.

## Multiple users

Each user has an independent durable programmer. Two users can work against the same show without merging their temporary values. Record and clear deliberately, and verify source ownership when another user or playback is active.
