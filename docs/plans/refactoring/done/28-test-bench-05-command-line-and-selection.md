# 05 — Command line and selection

## Outcome

Add typed command/keypad primitives and semantic fixture/Group selection across visible UI,
touch, typed API, and OSC routes.

Read `docs/help/30-Programmer/01-command-line.md` and the selection help before implementation.
Exact key order and command-line ownership remain product contracts.

## Command helpers

- `command.execute(text)` — enter and execute through real logical desk keys;
- `command.type(text)` — enter without executing;
- `command.clear()` and `command.expect(text)`;
- `keypad.press(keys)` — retain exact key order for gesture-specific acceptance cases;
- `hardware.connect/disconnect()` — simulated OSC hardware lifecycle.

The UI route clicks the visible `ENT` key. It does not substitute raw keyboard Enter.
Complete command strings remain available when command-line parsing itself is under test, not as
the normal way to express selection.

## Typed selection chunks

```ts
await t.selection.fixtures.item(4);
await t.selection.fixtures.items(1, 3, 2);
await t.selection.fixtures.range(1, 5);
await t.selection.groups.range(1, 3);

await t.selection.targets(
  fixtureRange(1, 5),
  group(2),
  groupRange(4, 6),
  dereferencedGroup(9),
);

await t.selection.range(fixture(101, 2), fixture(105, 2));
await t.selection.add(fixture(9), group(4));
await t.selection.remove(fixtureRange(3, 5));

await t.selection.next();
await t.selection.previous();
await t.selection.all();
```

Also provide `previous`, `next`, `all`, `clear`, and `expect.selection(...)`.

Matching typed kinds are required for generic ranges. Multi-head fixtures use
`fixture(number, head)` rather than decimal strings. Preserve ordered membership and step basis.
Skip missing Group IDs in a range; distinguish live Group references from dereferenced captures.

### Highlight and selection stepping

Highlight state remains independent from selection stepping:

```ts
await t.highlight.on();
await t.highlight.off();
await t.highlight.toggle();

await t.selection.next();
await t.selection.previous();
await t.selection.all();
```

- `highlight.on()` turns HIGH on for the actual selection.
- `highlight.off()` turns it off without restoring ALL or changing the remembered step source.
- `highlight.toggle()` toggles that same independent HIGH state.
- `selection.next()` and `selection.previous()` enter or move the authoritative stepped
  selection and wrap.
- `selection.all()` re-resolves the remembered live source, restores its complete ordered
  membership, and leaves single-step mode.

All five actions support visible software controls, the authenticated typed action, and OSC where
currently available. Their assertions expose Highlight active/output state separately from
selection mode, active index, total, and active fixture/head.

The current operator help and implementation define plain ALL but no distinct Shift+ALL action.
Reserve `selection.shiftAll()` in the plan until its intended product behavior is specified; do
not silently make it an alias for `selection.all()`.

## Routes

```ts
await t.selection.fixtures.range(1, 5);
await t.selection.fixtures.via.ui.range(1, 5);
await t.selection.fixtures.via.touch.range(1, 5);
await t.selection.fixtures.via.api.range(1, 5);
await t.selection.fixtures.via.osc.range(1, 5);

await t.selection.fixtures.via.keypad.range(1, 5);
await t.selection.fixtures.via.fixtureSheet.range(1, 5);
await t.selection.fixtures.via.stage.range(1, 5);
await t.selection.groups.via.pool.range(1, 3);
```

Visible actions whose modifier gesture is itself under test expose semantic click routes:

```ts
await t.selection.fixtures.via.click.item(1);
await t.selection.fixtures.via.shiftClick.item(5);
```

The second call performs a real Shift-click range gesture from the current visible anchor.
Equivalent semantic helpers may expose `.via.click` and `.via.shiftClick` when the product
assigns different behavior to them. Do not add a general locator-based click helper.

An unqualified route chooses reproducibly among eligible implementations in the current coverage
run. It reports candidates, selected route, action index, and seed. It never chooses an
implementation incapable of representing the target.

## Helper-contract scenarios

1. One fixture, ordered explicit fixtures, inclusive fixture range, and multi-head range.
2. Group selection, Group ranges with absent IDs, and intentionally empty Groups.
3. Mixed fixture/Group chunks with add and remove.
4. Previous/next stepping while preserving programmer state.
5. Unqualified selection reports and replays its seeded route.
6. Exact Fixture Sheet, Stage, pool, keypad, API, and OSC paths reach the same ordered oracle.
7. UI execution proves the visible `ENT` gesture.
8. Unsupported route/target combinations fail before mutation.
9. Different desk aliases remain isolated while one desk alias and its attached OSC path share
   the authoritative command line.
10. Highlight on, off, and toggle do not change selection step state.
11. Selection next, previous, and all preserve wrapping and remembered live-source semantics.
12. Plain click and Shift-click produce the documented single/range behavior on each supported
    visible selection surface.

## Done gate

- Normal selection scenarios contain no complete command strings, locators, coordinates, or UUID
  lookup.
- All explicit routes are truthful and independently verified.
- Selection order, step state, Group references, and empty/absent behavior match operator help.

## Result

Implemented the core semantic seams as the first executable slice of this chunk:

- command entry now tokenizes complete parser-test text into real logical software keys, and UI
  execution clicks the visible `ENT` key; exact keypad sequences remain separately available;
- typed fixture/head, fixture range, Group range, live Group, and dereferenced Group targets
  resolve through the authenticated programming-selection action and expose an ordered,
  UUID-free observation oracle;
- absent Group IDs are skipped within ranges while intentionally empty Groups remain valid
  sources; ordered add/remove, clear, previous/next/all, and normalized assertions are covered;
- Highlight power is independent from stepping and has UI, typed API, and OSC ports;
- simulated OSC hardware owns an explicit subscribe/unsubscribe lifecycle and never infers that
  the desk-wide connected flag must become false while peer controllers may remain.

The visible Fixture Sheet, Stage, Group pool, touch, keypad-selection, OSC-selection, seeded
unqualified-route, and desk-isolation matrix was split into
`05b-visible-selection-routes.md`. Keeping that matrix pending avoids falsely aliasing API
selection as a visible route or weakening route-specific acceptance proof.

Verification passed with 21 focused acceptance-intent unit tests, two focused public Playwright
scenarios, control-ui TypeScript, formatting, and the architecture/source-size ratchets. The
required full Playwright regression run completed with **300 passed / 9 skipped / 0 failed**.
The first sandboxed focused run could not bind localhost (`EPERM`); the identical approved
localhost run exercised the real server and OSC sockets successfully.
