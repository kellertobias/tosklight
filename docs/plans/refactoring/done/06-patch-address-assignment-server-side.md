# 06 — Patch-address auto-assignment: decide, then possibly move server-side

## Context (borderline api-rules §4 case, verified 2026-07-23)

Client-side per-fixture DMX-address layout during patch placement:

- `apps/control-ui/src/components/setup/fixturePatch/placementBatch.ts:214-217` — iterates
  split addresses, incrementing each by its footprint (`item.address += item.split.footprint`)
  to lay out consecutive fixtures; feeds `commitPlacementBatch` (`:219`).
- `apps/control-ui/src/components/setup/fixturePatch/fixtureIds.ts:72` —
  `address + index * footprint`.
- `apps/control-ui/src/components/setup/UniverseMap.tsx:20` — `address += length`.

api-rules §4 explicitly names "patch addresses" as a server-side spread. But unlike chunks
03–05, this feeds an **object-intent patch commit** (not live control), and the computed
addresses are shown to the operator in the placement preview **before** commit — so the
client needs the numbers for display either way.

The compliant v2 patch surface already exists: `crates/server/src/runtime/show_patch_http.rs:19-21`
(`/api/v2/shows/{show_id}/patch`, `/patch/fixtures`) with request-identity via
`crates/application/src/show_patch/` (replay: `show_patch/replay.rs:15-44`).

## DECIDED (maintainer, 2026-07-23)

**Option 1, with a client-side preview.** The client keeps showing the placement preview
(display concern), but it is *only* a preview: the commit request carries the placement
intent (start address + ordered fixture list + placement mode), and the **server computes
the actual patch addresses** at commit. The server result is authoritative; the preview is
an estimate.

## Work

1. Add/extend the v2 patch commit body (`show_patch_http.rs` surface, request identity
   already present via `crates/application/src/show_patch/`) so it carries the placement
   intent instead of client-computed final addresses; server computes + validates
   (conflicts, footprints).
2. Keep the client preview rendering (the increment loops in `placementBatch.ts` /
   `fixtureIds.ts` may survive **as display-only estimate code**), but the commit path no
   longer sends their output. Guard against divergence: a test feeds the same inputs to
   the preview and the commit and asserts the server's assignment matches what the
   preview showed (same deterministic layout rule on both sides — server rule is the
   spec, preview mirrors it).
3. Delete any client computation that only existed to build the commit body.

## Definition of done

- Commit sends placement intent; server assigns final addresses; preview still shown and
  provably consistent with the server's assignment for the same inputs; conflict/footprint
  validation happens server-side at commit.

## Verification

```sh
npm run test:unit
npm run test:e2e -- tests/<patch spec>
npm run test:e2e   # full suite gate
```

## Decisions

Decided (see above) — preview client-side, actual patching server-side. No open
decisions remain.

## Result

- Added a v2 patch placement intent containing the ordered fixture IDs, per-split base
  addresses, placement mode, and sparse operator overrides. The application layer now
  resolves authoritative mode footprints and computes every non-overridden address
  before the existing patch validation and replay path commits the revision.
- Kept placement estimates in the client for display only. Address/universe changes
  rebase the preview, independently moved proposals become sparse overrides, and the
  final fixture writes deliberately omit the preview's computed split assignments.
  Generic desired-state updates and unpatching send an empty placement-intent list.
- Regenerated the TypeScript and JSON-schema wire contracts and added focused
  application, server-route, wire, and client tests. Added `PATCH-PLACEMENT-001` to the
  real fixture-address screen coverage; it arranges the preview as `1, 50, 3`, commits,
  and compares the authoritative v2 patch snapshot with the displayed addresses.
- Verification passed: application 29 tests, server route 7 tests, wire 80 tests plus
  the generated-contract test, focused client 46 tests, full `npm run test:unit`
  (including 1,990 control-ui tests), focused fixture-address E2E 2 tests, and full
  `npm run test:e2e` at 283 passed / 11 skipped / 0 failed. This is above the recorded
  baseline of 281 passed / 12 skipped / 0 failed. `cargo fmt --all -- --check` and
  `git diff --check` also passed.
- The original `UniverseMap.tsx` claim was stale and referred to display-row wrapping,
  not fixture assignment. The operator help also requires independently movable
  proposals, so the selected consecutive mode retains sparse server-applied overrides.
  The first E2E draft used a synthetic drag that did not exercise the supported
  interaction; the final test uses proposal selection followed by tapping a free
  address.
- No new follow-up chunk was needed: pending chunk 08 already owns tolerant unknown-field
  logging, and pending chunk 12 already owns de-scoping the route layer.
