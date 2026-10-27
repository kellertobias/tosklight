# Align and value fans

**Align** is available beside every standard parameter encoder group. It modifies future relative encoder movement; activating it does not change a fixture value or create an Undo step. Select fixtures in the required order, then choose **Left**, **Right**, **Out**, or **In**. The first logical encoder you move becomes the bound attribute and captures every supported fixture's current value as its anchor. Choosing an Align mode with no fixtures selected changes nothing: Align stays **Off**, without a notice or Undo step.

- **Left** applies none of the signed movement to the first fixture and all of it to the last, interpolating between them.
- **Right** applies all of the movement to the first fixture and none to the last.
- **Out** applies all of the movement at both ends and none at the middle.
- **In** applies none at both ends and all at the middle.

For an even selection, both central fixtures receive the middle weight. With four fixtures, Out is `100, 0, 0, 100` and In is `0, 100, 100, 0`. One fixture receives the complete movement in every mode; with two fixtures, Out gives both the complete movement and In holds both at their anchors. Negative movement uses the same weights in the opposite direction. Each fixture keeps its own starting value, and the attribute's normal limits, wrap behavior, fine/coarse resolution, and profile support remain authoritative.

Choosing another Align mode keeps the result already produced and re-anchors from the current values; only later encoder movement uses the new mode. **Off**, Escape, Clear, Record, Preload, and Preload Go turn Align off without reverting values. Moving a different logical encoder first turns Align off, then applies that encoder movement normally. The desk's software, keyboard/command, OSC, and attached-hardware paths share this state and the same encoder transaction.

For absolute endpoint values, use a [THRU spread](01-command-line.md#value-ranges). Align weights future relative encoder movements.
