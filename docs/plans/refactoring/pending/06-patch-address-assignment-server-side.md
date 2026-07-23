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

## DECISION NEEDED (maintainer)

Pick one:

1. **Move the assignment server-side.** The commit request carries start address + ordered
   fixture list + placement mode; the server computes final addresses. The placement
   *preview* either calls a server preview endpoint (like `update/preview`) or keeps a
   client-side display-only estimate that is discarded at commit (risk: preview/commit
   divergence).
2. **Declare it display-side by policy.** Record in `docs/engineering/api-rules.md` §4 that
   patch-placement address layout is an explicit exception (client computes, server
   validates conflicts at commit), because the preview is the product surface.

## Work (after decision)

- Option 1: add/extend the v2 patch commit body; server computes + validates; client
  placement preview switches to server preview or clearly-labeled estimate; delete the
  client increment loops where no longer needed for display.
- Option 2: add the documented exception to api-rules §4 (one paragraph, dated), and add a
  server-side conflict validation on commit if not already present.

## Definition of done

- The decision is recorded (in api-rules.md for option 2, or implemented for option 1).
- Either the server computes committed addresses, or the exception is documented and
  commit-time conflict validation is confirmed/tested.

## Verification

```sh
npm run test:unit
npm run test:e2e -- tests/<patch spec>
npm run test:e2e   # full suite gate
```

## Decisions

**DECISION NEEDED** — see above. Do not start implementation before the maintainer picks
option 1 or 2.
