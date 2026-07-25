# 10b1 — Desktop, Show, and clock/DMX representative migration

## Outcome

Migrate one existing root UI workflow for each of the Desktop/screenshot, Show,
and deterministic clock/fixture-aware DMX helper families.

## Work

- Preserve each selected case's exact docs/testing contract and artifact names.
- Move operator action bodies to the semantic world without raw Page, Locator,
  ApiDriver, selectors, fixture UUIDs, or hidden clock/output transport calls.
- Improve the public helper when an acceptance step cannot be expressed.
- Mark fully migrated root specs with `// @bench-semantic-world` and update the
  generated inventory status.

## Done gate

- Three representative existing UI cases read as linear semantic intent.
- Architecture enforcement accepts their marked files.
- Focused cases, inventory check, parallel bench stress, and full E2E pass.
