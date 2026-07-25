# 10b — Representative UI workflow migration

## Outcome

Migrate one complete representative workflow for each semantic helper family
listed in parent chunk 10, improving the public helper whenever the migrated
scenario would otherwise need a selector, raw transport, or local recipe.

## Work

Migrate one Desktop/screenshot, Show workflow, clock/DMX, command/selection,
Programmer/encoder, Group, Preset, two-Cue Playback/Page, OSC parity, and
product-demo/free-run case. Preserve exact docs/testing behavior and artifacts.

Update the inventory status after each accepted migration. Delete a compatibility
helper only when its final consumer is gone.

## Done gate

- Each representative family has one linear semantic-world scenario.
- Migrated files pass the 10a architecture rule.
- Focused API/UI/OSC/wire cases, the parallel bench stress gate, and full browser
  E2E pass.

## Result

Split on 2026-07-25 so each representative migration can improve its semantic
helpers and preserve its acceptance contract in a reviewable commit:

- 10b1: Desktop/screenshot, Show, and clock/DMX;
- 10b2: command/selection, Programmer/encoder, Group, and Preset;
- 10b3: two-Cue Playback/Page, OSC parity, and product-demo/free-run.

The children retain this plan's full done gate. This parent records only the
split and does not mark an inventory case migrated.
