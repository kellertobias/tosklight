# 07 — Groups and Presets

## Outcome

Add typed Group and Preset helpers using the selection, Programmer, encoder, and observation
layers completed earlier.

## Public helpers

- `group.store(number, { mode })`;
- `group.select/recall(number)`;
- `group.edit(number, properties)`;
- `group.delete(number)`;
- `group.expect(number).fixtures(...)`, `.present()`, `.absent()`, `.empty()`, and metadata
  assertions;
- equivalent typed `preset.store/recall/edit/delete/expect` operations with a `PresetFamily`
  enum;
- `.via.pool`, `.via.keypad`, `.via.api`, and `.via.osc` where truthful.

Store modes are enums rather than strings:

```ts
await t.group.store(10, { mode: StoreMode.Overwrite });
await t.group.via.pool.store(10, { mode: StoreMode.Merge });
await t.group.via.keypad.store(10, { mode: StoreMode.Subtract });
```

## Operator contracts

- Preserve ordered Group membership where it affects spreading or intent.
- Distinguish an intentionally stored empty Group from an absent/deleted Group.
- Skip missing Group IDs in ranges.
- Preserve live Group references versus dereferenced fixture captures.
- Unpatched fixtures remain selectable, programmable, and storable in Groups and Presets.
- Preset families remain explicit; Mixed is not an aggregate display of every family.
- Record/update/delete UI paths use visible pool and modal actions with exact wording.

## Helper-contract scenarios

1. Overwrite, merge, and subtract an ordered Group through pool, keypad, API, and OSC where
   supported.
2. Store and recall an intentionally empty Group, then distinguish it from deletion.
3. Preserve an unpatched fixture in stored membership.
4. Demonstrate live Group reference versus dereferenced capture after membership changes.
5. Edit name, color, icon, and documented settings through visible UI.
6. Store and recall one Preset in each family.
7. Apply a Preset while stepped and prove it affects only the actual singleton selection.
8. Verify portable Preset behavior against fixture profiles rather than fixed fixture UUIDs.
9. Produce clear conflict/error diagnostics without hiding revision handling in the scenario.

## Done gate

- Scenario authors express Group/Preset intent without record-modal recipes or raw persisted
  objects.
- Empty, absent, ordered, live-reference, dereferenced, unpatched, and family semantics have
  focused contract coverage.
- UI and API routes use one normalized oracle.

## Result

- Added typed `group` and `preset` scenario worlds with deterministic unqualified routing and
  explicit pool, keypad, API, and OSC paths.
- Covered ordered membership, overwrite/merge/subtract, stored-empty versus absent Groups,
  missing IDs in ranges, metadata editing, live Group references, and dereferenced captures.
- Covered all five Preset families, visible pool customization, keypad/API/OSC recall, and
  singleton-selection application.
- Reused the foundational regression coverage for unpatched Group membership, portable
  fixture-profile Preset semantics, and revision/conflict diagnostics.

Verification:

- `npm run test:e2e -- tests/testBench/07-groups-and-presets.spec.ts`: 3 passed.
- `npm run test:unit`: 283 Vitest files and 2,007 tests passed; Rust suites passed.
- `npm run test:e2e`: 319 passed, 9 skipped.
- Control UI typecheck, bench typecheck, architecture boundaries, source-size policy, and
  `git diff --check` passed.
