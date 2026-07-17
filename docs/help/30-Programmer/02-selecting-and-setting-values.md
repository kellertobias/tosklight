# Selecting and Setting Values

The programmer holds temporary selection and attribute values for the current user. Nothing becomes show programming until it is recorded.

## Select fixtures

Select from Stage, Fixtures, a Group pool, or the command line. Selection is additive across touch surfaces until replaced or explicitly cleared. Use Thru, Plus, Minus, and Division for ordered ranges and subsets. Multi-head fixture IDs expand according to the rules in [Fixtures and Patch](../20-Show-Setup/01-fixtures-and-patch.md).

## Set values

Use `[AT]` for intensity, recall a Preset, or use the attribute encoders for color, position, beam, and other parameters. Fixture Sheet and Channels show the current value and its source. Per-value fade and delay can be entered from the command line.

## Highlight and Step Through

Highlight is a temporary identification output, independent of selection stepping. The desk keeps three things separate: the actual ordered programmer selection, the remembered live selection source used while stepping, and whether Highlight is active. Select fixtures from Stage, Fixtures, a Group, or the command line, then use the num-block row directly above GRP, CUE, TIME, and DIV:

- **HIGH** or `Alt+H` toggles Highlight without changing the selection or entering or leaving Step mode.
- **NEXT** or `Alt+Right` remembers the current live ordered selection when the complete selection is active, selects its first item, and then advances through that order.
- **PREV** or `Alt+Left` remembers the current live ordered selection when the complete selection is active, selects its last item, and then moves backward through that order.
- **ALL** or `Alt+A` re-resolves the remembered live source, restores its complete current ordered membership as the actual selection, and leaves the single-item step position.

NEXT from the last valid item wraps to the first; PREV from the first wraps to the last. After ALL, NEXT starts again at the first item and PREV starts at the last. If there is no remembered source, ALL leaves an already-complete selection unchanged. PREV and NEXT remain available whenever the remembered source resolves to at least one valid item. REST, WebSocket, and OSC feedback report complete-versus-step mode, the step index and total, and the active fixture or head identity without requiring a separate on-desk status panel.

The remembered state preserves a live selection expression or source rather than only a fixture-ID snapshot. If stepping began from a Group and that Group is edited, ALL restores its current ordered membership. Every selection operation not caused by PREV, NEXT, or ALL replaces the old remembered source with the resulting actual selection and returns to the complete-selection state. This applies to Fixtures, Stage, Groups, command-line selection, and every other authoritative control surface. An additive or subtractive selection while stepped acts on the actual singleton according to the normal selection rules; the result becomes the new complete selection and later step basis.

Changing Position, Color, Beam, or another programmer value does not reset the step basis. While stepped, an encoder, Preset, special dialog, Group Record, or other programming action operates on the actual singleton selection. Earlier values programmed for other fixtures remain in the programmer. Press ALL before an operation that should use the complete restored selection.

While stepping, the Fixture Sheet keeps the complete remembered base visible with a deliberately subdued selected treatment and shows the actual current fixture or head with the prominent selected treatment and an additional non-color marker. PREV and NEXT move only that prominent current-step treatment. This selection visualization remains present with HIGH off and does not change when HIGH is toggled. Multi-head fixtures show the treatment on their actual head rows; when those rows are collapsed, the parent row indicates whether it contains the current step or another remembered-base member. ALL returns every member to normal complete-selection styling, and an external selection replaces both the base and current-step indications with the new complete selection.

When HIGH is active, the configured Highlight Look applies to exactly the actual selection. A complete selection highlights every selected fixture or head; a step highlights only its singleton. PREV, NEXT, ALL, or an external selection change moves Highlight immediately to the resulting actual selection. Highlight can remain active with an empty selection and then applies automatically when a later selection becomes non-empty. The HIGH button remains visibly lit while active even when the selection is empty or live output is safety-suppressed. Turning HIGH off removes only the transient contribution; it does not restore ALL, clear selection, remove programmer values, or change the remembered step source.

Highlight output is never a programmer value and is not included by Record, Update, Merge, a Group, a Preset, a Cue, or another stored object. Values deliberately programmed while stepping remain after HIGH is turned off. For a highlighted profile channel, its exact Highlight raw value temporarily replaces normal programmer, playback, Group Master, and other ordinary value contributions; turning HIGH off reveals the correct underlying winner in the next frame without an intervening default frame. Highlight snaps on and off without a programmed fade, while channel inversion, raw resolution/limits, virtual-intensity behavior, and applicable safety processing remain authoritative. Grand Master stays above Highlight and is applied exactly once.

Stepping uses the desk's ordinary authoritative ordered selection. A selected multi-head fixture has already expanded to its selectable non-master heads, and each selected head is an independent step item; Highlight adds no separate master-head rule. A multipatched logical fixture or head remains one step item whose physical copies share its contribution. Overlapping sources and duplicate members retain only their first authoritative occurrence. Unpatched fixtures and heads remain valid items but emit no DMX until patched. A fixture with no Intensity channel still participates and can identify through a deliberately configured safe Highlight raw look. Deleted or invalid items disappear from the live sequence without reordering the remaining items; if none remain, the actual selection becomes empty and Highlight stays active without emitting output.

Blackout suppresses the applicable Highlight output, disabled routes remain disabled, and hazardous fixtures use their explicit safe values above Highlight. Blind, Preview, and Preload suppress live Highlight output while leaving the authoritative Highlight and selection state inspectable. HIGH still contains only its label and active indication. There is no Highlight summary or suppression panel in the command bar and no dedicated Highlight display in the hardware simulator; suppression remains available in protocol feedback. An actionable ownership or Highlight failure appears in a dedicated dismissible alert above all panes and modal surfaces, including at supported software-only and hardware-connected window sizes, without resizing the num block or HIGH.

One user owns live Highlight output on a desk at a time. Other sessions for that same user and desk share the authoritative actual selection, step source, step position, and Highlight state. Software, keyboard, REST, WebSocket, OSC, and attached hardware also share one repeat guard, so one physical press cannot advance twice through two adapters. A different user cannot silently take over live output; the controls identify the current owner and show an actionable error. Saving or reopening a show never restores live Highlight output or a stale step position.

## Clear and undo

The first `[CLR]` clears selection while leaving programmed values; the blinking Clear state shows that values remain. Press `[CLR]` again to clear the programmer. Undo and redo cover programming mutations such as recording or renaming, not live fader travel or playback execution.

## Multiple users

Each user has an independent durable programmer. Two users can work against the same show without merging their temporary values. Record and clear deliberately, and verify source ownership when another user or playback is active.
