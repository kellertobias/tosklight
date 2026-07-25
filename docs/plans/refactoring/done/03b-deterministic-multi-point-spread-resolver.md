# 03b — Shared deterministic multi-point spread resolver (adopts Done/50)

## Context

Adopted from [`../../Done/50-deterministic-multi-point-value-spreading.DONE.md`](../../Done/50-deterministic-multi-point-value-spreading.DONE.md)
(the full normative spec — read it before executing; this file only anchors it in the
queue). Adopted because chunks 03/05 already consolidate every spread computation onto
the server: building that consolidation on today's resolver and then changing the rule
later means touching all the same paths twice.

Verified 2026-07-23: the current shared resolver
`crates/server/src/runtime/cue_speed_commands.rs:30-38` (`spread_position`) does plain
linear interpolation — no exact anchor placement and no half-position expansion — so the
plan-50 rule (`100 THRU 0 THRU 100` over 6 items → `100, 50, 0, 0, 50, 100`) is a real
behavior change to exactly the function chunks 03/05 route everything through. The spec
explicitly sanctions the compatibility-visible output change (release notes + operator
help callout required). No open maintainer decisions in the spec.

## Work

1. Implement the deterministic anchor rule from the spec (exact integer/rational anchor
   comparisons, half-position expansion, equal-step interpolation between anchors) as one
   shared resolver at the lowest common Rust layer usable by both engine rendering of
   stored `AttributeValue::Spread` and server-side assignment (`crates/core` or a module
   both `crates/engine` and `crates/server` reach). Replace the body of `spread_position`
   with it (or delete `spread_position` in favor of the shared function).
2. Reject `P > N` (more control points than selected items) with a visible, actionable
   error and no partial mutation, on every entry path.
3. Route all paths through it — after chunks 03/05 these are: command line
   (`programmer_selection_values.rs`, `programmer_fixture_commands.rs`,
   `programmer_group_commands.rs`), the values wire `Spread` handling, and wherever the
   engine resolves stored group `AttributeValue::Spread` at recall/render time. The client
   no longer computes spreads at all (chunk 03's outcome) — no frontend formula to remove
   beyond what 03/05 already deleted.
4. Unit vectors: the normative table (4/5/6/10 items × `100 THRU 0 THRU 100`) plus the
   asymmetric, reversed-selection, and boundary cases from the spec's acceptance list
   items 1–3.
5. Cross-surface acceptance: extend `PROG-002` per the spec (items 4–8) — command line,
   encoder modal both layouts, OSC/hardware keypad, live-Group/Preset/Cue recall after
   membership edit, `DEGRP` freeze, legacy-show `AttributeValue::Spread` load without
   migration.
6. Docs: replace the even-selection open question in
   `docs/help/99-Development/01-open-questions.md` with the normative rule; update the
   selecting/setting-values help and the `PROG-002` testing text. `npm run manual`.

## Definition of done

- One resolver, all surfaces byte-identical for the same control points and selection;
  normative table pinned by unit vectors; `P > N` rejected everywhere; PROG-002 extended
  and green; help updated.

## Verification

```sh
cargo test -p core -p engine -p server -p programmer
npm run test:unit
bash tools/test.sh e2e --grep "PROG-002"
npm run test:e2e   # full suite gate
npm run manual
```

## Decisions

None — the spec is normative, and it explicitly authorizes the output change for existing
multi-point spreads (call it out in the result note and release notes).

Sequence: after 03 (server-side spread routing), together with or before 05.

## Result

- **Resolver:** `light_core::resolve_spread` implements the deterministic anchor rule
  with exact rational anchor placement (integer → exact item, exact half → both
  adjacent items, otherwise nearest) and symmetric weighted interpolation so reversed
  selections resolve to exactly mirrored bytes. `spread_position` now delegates to it;
  the engine's duplicate (`value_for_ordered_position`) and the server's command/group/
  fixture/SetSelection paths all route through the one function. For stored legacy
  spreads with more points than items the resolver deliberately degrades to the old
  linear sampling so existing shows keep rendering.
- **P > N rejection (≥3 points):** command line (selection, fixture-range, and group
  commands — via `ensure_spread_fits`) and the v2 `set_selection` wire (fallible
  decode) reject with a visible error and no partial mutation; pinned by
  `multi_point_spread_with_more_points_than_selection_is_rejected_without_mutation`.
  Remaining gap: `set_group` via the values wire can't see membership size
  (environment carries only id sets) — filed in `pending/03b-b`.
- **Unit vectors:** the full normative table (4/5/6/10 × `100 THRU 0 THRU 100`),
  asymmetric three- and four-point vectors, reversed-control-point mirroring, and the
  boundary set (empty/one item/single point/plateau/degradation/stability) in
  `light_core::attributes::spread_tests`.
- **PROG-002:** extended with the five-item normative multi-point command-line case
  (`AT 100 THRU 0 THRU 100` → DMX `255,128,0,128,255`); focused run 7/7 green. The
  remaining cross-surface acceptance (encoder modals both layouts, OSC keypad, recall
  matrix after membership edits, DEGRP freeze, legacy-show load) split to
  `pending/03b-b-spread-acceptance-surfaces.md`.
- **Docs:** the open-questions even-selection entry replaced with the resolved rule;
  selecting-and-setting-values help documents repeated `THRU`, midpoint expansion, and
  the rejection; `npm run manual` regenerated cleanly.
- **Compatibility-visible change (as the spec authorizes):** existing multi-point
  spreads over selections where anchors move now resolve differently (e.g. 6 items ×
  `100 THRU 0 THRU 100` was `100,60,20,20,60,100`, now `100,50,0,0,50,100`). Release
  notes must call this out.
- **Surprise:** chunk 03 had removed `deny_unknown_fields` from the two touched values
  enums per the standing rule, which silently broke the deliberate forged-field guard
  `programmer_values_wire_rejects_transient_or_mode_fields` (its cargo gate ran before
  the last wire edit). Restored `deny_unknown_fields` on both enums here; chunk 08 must
  reconcile tolerant typing with that forged-`mode` rejection contract deliberately.
- Suite numbers: core 5, engine 74+1, programmer 92, server 413 all passed; PROG-002
  focused 7/7; full e2e **276 passed / 11 skipped / 1 failed** (pre-existing user-dirty
  product-demo) — no net new regressions.
