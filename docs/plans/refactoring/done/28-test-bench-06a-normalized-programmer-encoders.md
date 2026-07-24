# 06a — Normalized Programmer encoders

## Outcome

Add the enum-backed normalized encoder object tree, typed Programmer expressions, absolute API
intents, relative API detents, visible value entry, and live software slot resolution. Hardware,
discrete, special-dialog, and Programmer Fade work continues in 06b and 06c.

## Encoder API

```ts
const pan = t.encoder.position.pan;

await pan.set(50);
await pan.add(3);
await pan.subtract(2);

await pan.set([
  100,
  ProgrammerToken.Thru,
  0,
  ProgrammerToken.Thru,
  100,
]);
```

The normal shape is `encoder.<group>.<attribute>.<operation>`. Group and group-specific attribute
enums generate this typed tree; misspellings and an attribute beneath the wrong group fail type
checking. Typed test recipes may index the same object dynamically:

```ts
const pan = t.encoder[EncoderGroup.Position][PositionAttribute.Pan];
```

Initial groups mirror the canonical product families: Intensity, Color, Position, Beam, Shapers,
Focus, Control, and Media. Each group has a distinct attribute enum backed by stable logical
Programmer IDs.

## Operations and expressions

- `.set(value)` uses the explicit absolute value-entry path;
- `.set(tokens)` enters a typed Programmer expression;
- `.add(steps)` and `.subtract(steps)` perform relative encoder detents;
- `.press(...)` and `.pressTurn(...)` exist only for attributes with those hardware semantics;
- `.via.touch`, `.via.ui`, `.via.api`, and `.via.osc` constrain the route.

Initially the expression type accepts numbers separated by `ProgrammerToken.Thru`, matching the
real encoder modal. Reject leading, trailing, or repeated `Thru` during registration. Discrete
and special encoders expose their own typed value enums rather than arbitrary strings.

## Central encoder catalog and resolver

For every group/attribute pair, keep one source of truth for:

- stable logical Programmer attribute;
- valid value type and operations;
- eligible routes;
- software and hardware group control;
- page/cycle order and repeated group presses;
- live encoder slot after the page is active;
- visible label and normalized observation.

The adapter:

1. ensures Programmer mode;
2. observes the active group/page and current encoder projection;
3. selects or cycles the group until the requested attribute page is active;
4. verifies the logical attribute and resolves its current Encoder 1–6 slot;
5. performs relative or explicit value entry through the requested route;
6. waits for Programmer/output evidence.

Prefer live feedback over blindly replaying a press count. If a shortcut, page order, or slot
changes later, update only the catalog/adapter.

## Other Programmer helpers

- `timing.programmerFade.set(duration)`;
- `timing.programmerFade.via.fader.set(duration)`;
- `timing.programmerFade.via.valueEntry.set(duration)`;
- `timing.programmerFade.double()`, `.half()`, and `.off()`;
- set, release, and clear semantic values;
- ordered spreads;
- Programmer Fade;
- attribute-family special dialogs;
- Position Return Home;
- alignment and Dynamics controls where documented;
- normalized and discrete Programmer assertions including source ownership.

Programmer values retain LTP semantics. Software encoder changes are relative by default; the
explicit `.set` path remains separate.

Programmer Fade uses a typed duration:

```ts
await t.timing.programmerFade.set("4s");

await t.timing.programmerFade.via.fader.set("4s");
await t.timing.programmerFade.via.valueEntry.set("4s");

await t.encoder.intensity.dimmer.set(100);
await t.clock.advanceBy("4s");
```

The unqualified `set` chooses reproducibly between eligible routes and reports the selected
method. `.via.fader` performs a real pointer/touch slide on the visible Programmer Fade fader to
the requested time. `.via.valueEntry` presses the visible **Set value** action and enters the
typed duration through its value dialog. It must not fill the underlying range input directly or
substitute an API mutation while claiming either UI route.

Hardware/OSC routes operate the corresponding desk control when available. Every route waits for
authoritative timing feedback before the next value is written. The recorded Programmer value
retains the resolved fade as documented.

## Helper-contract scenarios

1. Set, add, and subtract a normalized Dimmer through every eligible route.
2. Resolve Pan after changing groups and prove the helper does not assume Encoder 1.
3. Reach an attribute page requiring repeated group navigation and verify live feedback.
4. Enter two-point and multi-point `Thru` expressions over ordered selection.
5. Reject malformed token expressions before mutation.
6. Prove group-specific attribute typing with compile-time contract tests.
7. Exercise one discrete encoder with a typed value enum.
8. Preserve selection stepping and Programmer LTP behavior.
9. Return Position heads to independent profile homes through the documented dialog.
10. Change the catalog's test mapping and prove scenarios remain unchanged.
11. Set, double, halve, and turn off Programmer Fade, then prove a subsequently written value
    reaches its exact deterministic boundary and retains that timing when recorded.
12. Set the same Programmer Fade through a real fader slide and through **Set value**, assert the
    same authoritative result, and verify that an unqualified seeded route reports and replays
    which method it chose.

## Done gate

- Scenario files contain no encoder labels, physical slots, group press counts, or modal strings.
- Absolute and relative semantics are visibly distinct.
- The helper-contract suite proves typed normalized expressions and current software-slot
  resolution.

## Result

- Added group-specific enums and stable logical Programmer IDs for the six normalized encoder
  families currently exposed by the foundation.
- Added validated single-, two-, and multi-point value expressions with pre-mutation rejection of
  leading, trailing, repeated, or missing `THRU` separators.
- Added absolute and relative API Programmer intents with current selection and configured timing,
  plus visible value entry that resolves the live family and software encoder without exposing a
  slot to scenarios.
- Added seeded unqualified route reports and explicit `.via.api` / `.via.ui` ports.
- Passed 20 focused catalog/programmer tests, TypeScript typechecking, and three focused browser
  scenarios for Dimmer, Pan, relative detents, and multi-point spreading.
- The full Playwright regression passed the three new scenarios and every existing scenario except
  for one Stage Shift-click observation that reached only part of its range within the old
  two-second helper deadline. Widened that observation window for loaded regression runs; its
  focused browser scenario then passed.
- Deferred hardware/discrete/special encoders to 06b and Programmer Fade routing to 06c.
