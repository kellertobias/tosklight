---
slug: ordered-selection
title: "Ordered Selection: Fixtures, Heads, Groups, and DEGRP"
components: [programmer, backend, control-ui]
order: 24
---

# Ordered Selection: Fixtures, Heads, Groups, and DEGRP

Operator contract: `docs/help/30-Programmer/01-command-line.md`,
`docs/help/30-Programmer/02-selecting-and-setting-values.md`, and
`docs/help/20-Show-Setup/05-groups-and-presets.md`. Acceptance landmarks are GROUP-003 through
GROUP-005 and PROG-001/002 in `tests/01-foundational-dimmers-and-groups.spec.ts`.

Selection is ordered authority. It is not a JavaScript `Set`, a sorted fixture list, or a visual
highlight cache.

## Four input surfaces

Visible Stage/Fixture Sheet gestures, command-line tokens, and exact OSC key phases converge on
`crates/application/src/programming/service/selection.rs`. The frontend's optimistic layer is
`apps/control-ui/src/features/programmingInteraction/selectionWriter.ts`; it predicts feedback but
does not become authority.

The command parser and stable expression live in `crates/programmer/src/command_line.rs` and
`crates/programmer/src/selection.rs`. Fixture and logical-head identities survive recompilation.

## Groups and ranges

`crates/programmer/src/groups.rs` resolves a Group in stored membership order. A stored empty Group
is a valid object with zero members. An absent Group is an error when addressed directly. Missing
IDs inside a range are skipped rather than invented as empty Groups.

Add, subtract, toggle, and range operations preserve the surviving order. Overlapping Groups do not
deduplicate by sorting; the selection expression owns the deterministic result.

## Live reference versus DEGRP

A selected Group may remain a live reference: later membership edits change which fixtures a
subsequent value reaches. `DEGRP` dereferences the expression into its current ordered fixture/head
members. From that point the selection is frozen and future Group edits do not alter it.

This distinction is why the parser has a first-class DEGRP token rather than treating
`GROUP GROUP` as an alias.

## Event order and repair

The selection transition is published before the related show/runtime projection so attached
software and OSC surfaces see the correct target for the following value. A reconnect or sequence
gap installs the authoritative selection snapshot; it never replays local keypresses.

`apps/control-ui/src/features/programmingInteraction/selectionPrediction.ts` and
`selectionWriter.ts` reject late work after desk/session/show replacement.

## Unpatched fixtures

Patch state does not filter selection. An unpatched fixture remains selectable, programmable,
groupable, and recordable; only fixture projection omits physical output.

## Exercise

Read `crates/programmer/src/tests/selection_and_sessions.rs` and
`tests/support/foundational/supplementalGroups.ts`. For the sequence Group 1, subtract Fixture 2,
add Group 2, then DEGRP, write the expected stable identity order before checking the assertions.
