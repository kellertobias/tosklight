# 03 — Fan-out family: one server-side contract, proven on parameter range spreads

**Family note (maintainer 2026-07-23):** chunks 03/03b/04/05 are one concept — the server
computes per-fixture results from an ordered selection plus operation parameters — seen
from different angles. This chunk designs the **single shared request vocabulary** for
all of them (ordered selection + operation payload: scalar spread points | color
endpoints | position delta) and proves it on the parameter-range case. 03b supplies the
resolver math; 04 and 05 apply the same contract and must not invent parallel request
shapes.

## Context (api-rules §4 violation, verified 2026-07-23)

`apps/control-ui/src/components/control/parameterControls/parameterValueMutations.ts`:

- `spreadValue` (`:171-180`) interpolates a per-fixture value across the ordered selection
  client-side. It is a line-for-line duplicate of the server's `spread_position`
  (`crates/server/src/runtime/cue_speed_commands.rs:30-38`).
- `setParameterRangeMutations` uses it for the **non-group** branch (`:64-70`), emitting one
  `set_fixture` mutation per fixture with a precomputed normalized value (`:148-165`, `:161`).
- The **group** branch already does it right (`:59-63`): it sends
  `{ kind: "spread", value: points }` and lets the server compute.

Server support already exists end-to-end:

- Wire: `crates/wire/src/v2/programming.rs:20` `Spread(Vec<f32>)`;
  core `crates/core/src/attributes.rs:278`.
- HTTP values wire: `crates/server/src/command_http/values_wire.rs:203,244`
  (and preload: `preload_values_wire.rs:173,220`).
- WS handler accepts `AttributeValue::Spread`: `crates/server/src/runtime/ws_programmer_handlers.rs:28-37`.
- Command line `AT X THRU Y` computes server-side:
  `crates/server/src/runtime/programmer_selection_values.rs:25-51`.

Writes go to `POST /api/v2/users/{user_id}/programmer-values/actions` via
`ParameterValuesMutationPort.batch` (`parameterValueMutations.ts:131`;
URL: `apps/control-ui/src/api/ProgrammerValuesTransport.ts:170`).

## Work

1. Design the shared fan-out wire shape first (ordered selection + typed operation
   payload), sized so 04 (position delta → stage layout) and 05 (color endpoints →
   programmer values) fit the same vocabulary without new plumbing. Then make the
   non-group branch of `setParameterRangeMutations` send one mutation carrying the
   ordered selection and `{ kind: "spread", value: points }`, mirroring the group branch.
   If the current `set_fixture` mutation shape can't carry an ordered fixture list + spread,
   extend the v2 programmer-values wire contract (typed, tolerant per api-rules §5) and
   regenerate contracts.
2. Verify the server's spread ordering matches the previous client behavior (selection order,
   not fixture-id order) — the command-line path at `programmer_selection_values.rs:48` maps
   `(index, fixture_id)` over the ordered selection; reuse that.
3. Delete `spreadValue` from the client once unused.

## Definition of done

- No client code path computes per-fixture spread values for parameter ranges.
- `spreadValue` removed from `parameterValueMutations.ts`.
- Encoder/typed range entry over an ordered multi-fixture selection produces the same
  fixture values as before (pin with a test at the API level).

## Verification

```sh
npm run test:unit          # parameterValueMutations + transport unit tests
npm run test:e2e-api
npm run test:e2e           # full suite gate
```

Manual: `npm run open`, select 3+ fixtures in order, type a range on an encoder, confirm
the spread lands identically (compare against `AT 0 THRU 50` on the command line).

## Decisions

None. Chunk 03b (deterministic multi-point resolver, adopted from Next/50) follows
directly on this chunk — keep the server-side routing generic over N control points so
03b only has to swap the resolver math, not the plumbing.

## Result

- **Shared fan-out vocabulary landed:** new `SetSelection` variant (ordered
  `fixture_ids` + typed operation payload + timing) on both the v2 wire mutation and
  action enums (`crates/wire/src/v2/programming.rs`), documented as the contract 04/05
  reuse. `deny_unknown_fields` removed from both touched enums per the standing rule.
  Contracts regenerated (`generate-contracts`).
- **Server-side resolution:** the wire decode (`values_wire.rs::application_mutations`)
  expands `SetSelection` into per-fixture `SetFixture` mutations in the request's
  selection order — `Spread` control points interpolate via the shared math, any other
  value applies uniformly — so validation, replay fingerprints, and execution see plain
  per-fixture mutations unchanged.
- **One resolver, one home:** `spread_position` moved from
  `runtime/cue_speed_commands.rs` into `light_core::attributes` (re-exported for the
  command-line/group/fixture paths), giving 03b a single place to swap the resolver
  math.
- **Client:** the non-group branch of `setParameterRangeMutations` now sends one
  `set_selection` mutation with the ordered selection and spread points, mirroring the
  group branch; `spreadValue` and `setParameterMutationsForFixtures` deleted. The
  optimistic prediction deliberately leaves `set_selection` to the authoritative server
  outcome instead of duplicating interpolation. Mutation keys cover the new action.
- **Pin test:** `set_selection_resolves_the_spread_server_side_in_selection_order`
  (server, API level) proves 3 fixtures in deliberately non-sorted order resolve to
  0 / 0.25 / 0.5 for points `[0, 0.5]` — identical to the old client math and the
  command line's `AT 0 THRU 50`. Client unit test updated to the new single-mutation
  shape.
- Suite numbers: light-core 1 + light-server 412 passed (fresh test included);
  `test:unit` clean (pre-existing size-ratchet exit only); `test:e2e-api` 85 passed /
  1 skipped; full e2e **276 passed / 11 skipped / 1 failed** (pre-existing user-dirty
  product-demo) — no net new regressions.
- Note: the interactive `npm run open` comparison was not run in this session; the
  API-level pin test covers the value equivalence the manual check targets.
