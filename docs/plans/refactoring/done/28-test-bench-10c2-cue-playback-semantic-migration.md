# 10c2 — Cue and Playback semantic migration

## Outcome

Migrate the remaining ordinary Cue, Cuelist, Playback, Preload, MIB, and Cue
navigation UI cases.

## Scope

- `02-cue-semantic-contracts.spec.ts`
- `02-cues-tracking-and-arbitration.spec.ts`
- `06-cuelist-view-and-settings.spec.ts`
- `06-preload-modes-and-virtual-playbacks.spec.ts`
- `07-move-in-black.spec.ts`
- `07-playback-configuration.spec.ts`
- `09-cue-go-to-load.spec.ts`

## Done gate

- All 32 pending inventory rows in scope are migrated or narrowly justified.
- Cue tracking, timing, identity, Page addressing, momentary controls, Preload,
  and MIB boundaries retain their exact operator contracts.
- Focused API/UI/OSC/wire cases, architecture, inventory, and parallel stress
  pass.

## Result

- Re-verified all 32 pending inventory rows across the seven named root specs.
- The scope spans 2,120 lines of root entrypoints and a further 2,561 lines in
  the Cue semantic-contract support modules, with independent recording,
  timing, tracking, Cuelist, Preload, MIB, and Playback Configuration contracts.
- Split execution into `10c2a` through `10c2g` so each operator-contract family
  can retain its API/UI/OSC/wire oracle and pass focused parallel verification
  before landing.
