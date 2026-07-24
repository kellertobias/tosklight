# 23 — Housekeeping: source-size "new" violations + opportunistic test-module policy

## Context (verified 2026-07-24)

Before the source-size checker was narrowed to production code, this plan identified
the following code targets beyond the legacy ratchet:

1. `apps/control-ui/src/api/ServerContext.tsx:52` `ServerProvider` — 323 lines.
   **Handled by chunk 22** — do not fix here; skip if 22 hasn't landed.
2. `apps/control-ui/src/components/control/useSoundCapture.ts:45` — 152 lines. Extract
   phases (device selection, analyser wiring, teardown) into named helpers.
3. `apps/control-ui/src/components/modals/QuickSetupModal.tsx:180` — 153 lines. Extract
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
2. `node tools/check-source-size.mjs --ratchet` once clean; commit the ratchet file.

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

Claimed on 2026-07-24 with the existing production-only source-size tooling edits
preserved as user-owned work.

## Result

Completed on 2026-07-24.

- Split sound observation publication, analyser lifecycle, and per-group capture
  reconciliation into named hooks/helpers. The production sound-capture module is
  226 lines and has no function above the 150-line hard limit.
- Split Quick Setup state/workflow from its primary sections and extracted its
  stacked dialogs into `QuickSetupDialogs.tsx`. The original 1,486-line module is
  now 678 lines, the extracted dialog module is 856 lines, and neither contains a
  function above the hard limit.
- Ran `node tools/check-source-size.mjs --ratchet`. The production-only baseline
  was already fully tightened, so the command made no tracked baseline change and
  confirmed zero files above 1,200 lines and zero functions above 150 lines.
- Retained the opportunistic feature-local test-module policy from the context;
  no server feature test modules were touched by this housekeeping-only chunk.

Verification:

- `node tools/check-source-size.mjs`
- `node tools/check-architecture.mjs`
- `node tools/test-command-boundaries.mjs`
- `npm run typecheck`
- focused Vitest: 2 files, 12 tests passed
- `npm run test:unit`: all Rust suites and 277 frontend files / 1,992 tests passed
- `npm run test:e2e`: 287 passed, 9 skipped
