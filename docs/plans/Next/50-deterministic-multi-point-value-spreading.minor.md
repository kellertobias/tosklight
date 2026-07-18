# Deterministic Multi-Point Value Spreading

## Status

**Specification only.** This minor feature defines deterministic multi-point spread behavior and the coverage required to prove it. It does not implement runtime behavior, executable tests, or operator-help changes.

## Goal

Make every scalar value spread use one predictable rule across the authoritative ordered selection. A two-point spread keeps its established meaning: the first selected item receives the first value, the last selected item receives the last value, and intervening items are equally interpolated.

For a spread with three or more explicit control points, every control point must occur on a real selected item. When an interior control point lies exactly between two selected items, expand that point across both items instead of interpolating past a midpoint that does not exist.

The same result must be produced whether the operator enters the spread through the command line, OSC or attached-hardware keypad input, a software encoder value modal, or a hardware-connected encoder value modal. Intensity and scalar position, color, beam, and other encoder attributes use the same resolver; a surface must not implement its own competing interpolation formula.

## Deterministic anchor rule

Resolve a spread from `P` ordered control points across `N` items in the authoritative selection order. Number both from zero.

1. The first and last control points anchor the first and last selected items.
2. Interior control point `j` has the ideal ordered position `j × (N - 1) / (P - 1)`.
3. If that position is an integer, the item at that position receives the control point exactly.
4. If that position is exactly halfway between two item positions, both adjacent items receive the control point exactly. This is the required expansion of a nonexistent midpoint.
5. Otherwise, the nearest item receives the control point exactly.
6. Values between the right edge of one anchor and the left edge of the next anchor are linearly interpolated in equal steps. Rounding or DMX quantization happens only after the normalized per-item values have been resolved.

Use exact integer or rational comparisons for anchor placement so floating-point error cannot change whether a midpoint expands. Reversing the selection reverses the resolved item sequence; fixture IDs, patch addresses, and hash-map iteration order never participate.

Examples for `100 THRU 0 THRU 100` are normative:

| Selected items | Resolved percentages |
| --- | --- |
| 4 | `100, 0, 0, 100` |
| 5 | `100, 50, 0, 50, 100` |
| 6 | `100, 50, 0, 0, 50, 100` |
| 10 | `100, 75, 50, 25, 0, 0, 25, 50, 75, 100` |

The rule applies unchanged to asymmetric and longer expressions such as `10 THRU 80 THRU 20` and `0 THRU 100 THRU 25 THRU 75`; it is not a special case for a symmetric three-point spread.

A multi-point expression with more control points than selected items cannot place every explicit point. Reject it with a visible, actionable error and make no programmer mutation. Preserve the established single-item behavior for ordinary one-value and two-point inputs; do not silently collapse a multi-point expression to a subset of its points.

## Authoritative data and mutation semantics

Implement one shared spread resolver at the lowest common Rust domain layer that can be used by engine rendering and server-side fixture assignment. Remove equivalent interpolation formulas from command handlers and frontend encoder submission paths, or make them call the same authoritative server operation. Frontend preview or formatting may mirror the result only when it is covered against the shared resolver with the same vectors.

The ordered input is always the resolved current selection order. In particular:

- a live Group uses its stored ordered membership and retains the spread control points as `AttributeValue::Spread`, so membership edits, Preset recall, Cue recall, and subsequent rendering resolve the same rule against the current membership;
- `DEGRP` and other fixture-scoped operations resolve the complete per-fixture values once and do not remain attached to later Group membership changes;
- direct fixture selections, multi-head expansion, and unpatched fixtures retain their existing authoritative order and participation rules; and
- all per-fixture assignments produced by one confirmed spread land atomically as one programmer mutation with one undo step. Invalid input or a failed assignment must not leave a partial spread.

This feature changes spread resolution, not the persisted representation. Existing shows containing spread control points must load without migration and resolve through the deterministic rule. Because this intentionally changes the output of existing multi-point spreads over some selection sizes, release notes and operator help must call out the compatibility-visible correction.

## Surface parity

The implementation is incomplete until the same control-point sequence produces the same normalized values through every applicable path:

- command-line entry against an explicit fixture selection, the current retained selection, a live Group, and `DEGRP`;
- software/touch keypad entry and OSC or attached-hardware keypad entry into that same desk-local command line;
- the main desk's scalar attribute encoder value modal;
- the hardware-connected encoder value modal;
- authenticated REST or WebSocket programmer operations that accept `AttributeValue::Spread`; and
- live Group values recalled directly, from a Preset, and from a Cue after Group membership changes.

Repeated `THRU` input must remain visible and editable until confirmation. Unsupported or impossible input must report the same reason on operator surfaces and protocol responses rather than being ignored, truncated, or partially applied.

Compound color-picker gestures, fixture-location vector gestures, and other inputs that do not use scalar `THRU` control points keep their separately documented interpolation spaces. If such a path stores `AttributeValue::Spread`, however, its ordered resolution must use this anchor rule.

## Acceptance coverage

Extend the focused `PROG-002` contract rather than creating a separate competing spread scenario. Coverage must prove normalized programmer values before asserting quantized DMX output.

At minimum, add:

1. shared-resolver unit vectors for 4, 5, 6, and 10 selected items with `100 THRU 0 THRU 100`, matching the normative table exactly;
2. asymmetric three-point and four-point vectors, integer anchors, half-item anchor expansion, non-half nearest anchors, ascending and descending endpoints, and reversed selection order;
3. boundary coverage for empty selections, one selected item, equal adjacent points, more control points than items, unpatched members, and expanded multi-head selections;
4. paired API/UI command-line coverage that enters repeated `THRU` tokens through production controls and asserts one atomic mutation, exact normalized values, final DMX bytes, and one-step undo;
5. encoder-modal coverage for both software-only and hardware-connected layouts, including intensity and at least one non-intensity scalar attribute such as Pan or Tilt;
6. an OSC or attached-hardware keypad case proving that physical input continues the shared desk command and lands the same multi-point result exactly once;
7. live Group, Preset, and Cue recall coverage before and after changing ordered Group membership, plus a `DEGRP` case proving frozen per-fixture values do not follow that edit; and
8. compatibility coverage that loads an existing show containing a multi-point `AttributeValue::Spread` without schema migration and resolves it using this rule.

Property coverage should additionally prove that every representable explicit control point appears in the result, endpoints are exact, interpolation is monotonic between adjacent anchors, results contain no non-finite values, repeated evaluation is byte-for-byte stable, and equivalent surfaces do not diverge.

## Documentation and verification

When implementing this plan, replace the unresolved even-selection question in `docs/help/99-Development/01-open-questions.md` with the normative rule and examples in the operator-facing selecting and setting values help. Keep the command-line help explicit that repeated `THRU` supplies multiple control points, and update the `PROG-002` testing text with cross-surface and stored-live-Group expectations.

Verification should begin with focused resolver, programmer, server, and frontend tests, then run the focused `PROG-002` Playwright cases for API, software UI, OSC or attached hardware, and both encoder layouts. Finish with the relevant broader suites, manual generation, and the authoritative packaged `./build open` path because operator-visible command and encoder behavior changes.
