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
