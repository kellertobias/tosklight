# 23 — Housekeeping: source-size "new" violations + opportunistic test-module policy

## Context (verified 2026-07-23)

`node tools/check-source-size.mjs` currently reports four **new** violations (beyond the
legacy ratchet):

1. `docs/plans/Later/62-media-server-integration-and-rust-migration.md` — 1345 lines.
   This sits in the user's dirty worktree (moved from `Next/`); it is a planning doc, not
   code. Either split it or add docs to the checker's exemptions — small maintainer call,
   ask in the result note if unclear.
2. `apps/control-ui/src/api/ServerContext.tsx:52` `ServerProvider` — 323 lines.
   **Handled by chunk 22** — do not fix here; skip if 22 hasn't landed.
3. `apps/control-ui/src/components/control/useSoundCapture.ts:45` — 152 lines. Extract
   phases (device selection, analyser wiring, teardown) into named helpers.
4. `apps/control-ui/src/components/modals/QuickSetupModal.tsx:180` — 153 lines. Extract
   step components/sections.

After fixing, run the checker with `--ratchet` to tighten the stored baseline (the tool
itself suggests this once new violations are gone).

Also record (README standing rule, no work here): `crates/server/src/runtime/tests/`
still holds **82 modules** (largest: `playback_v2_route_tests.rs` 84 KB,
`active_show_programmer_object_tests.rs` 38 KB) — migrate into feature-local unit tests
**opportunistically when touching a feature**, never big-bang. Chunks 09–20 each touch
several of these; move the relevant modules as part of those chunks.

## Work

1. Refactor `useSoundCapture` and `QuickSetupModal` below the 20-line-function /
   size thresholds without behavior change (sound capture has e2e coverage via SOUND
   scenarios; QuickSetup via SHOW/recovery flows).
2. Resolve the 1345-line doc violation (split or exempt-with-note).
3. `node tools/check-source-size.mjs --ratchet` once clean; commit the ratchet file.

## Definition of done

- `node tools/check-source-size.mjs` reports zero new violations (ServerProvider excepted
  iff chunk 22 is still pending — note it); ratchet tightened.

## Verification

```sh
node tools/check-source-size.mjs
npm run test:unit
npm run test:e2e   # full suite gate
```

## Decisions

None blocking; the docs-file exemption question can ride in the result note.
