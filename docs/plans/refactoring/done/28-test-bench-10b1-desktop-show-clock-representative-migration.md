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

## Result

Completed on 2026-07-25.

- Promoted the existing Desktop/screenshot, visible Show lifecycle, and deterministic
  clock/fixture-DMX semantic scenarios into one enforced root catalog spec without
  changing their scenario IDs, operator intent, or screenshot artifact names.
- Marked the root spec with `// @bench-semantic-world`; the architecture boundary
  accepts it without raw Playwright, API-driver, selector, coordinate-click,
  fixture-UUID, encoder-slot, or mutable-show access.
- Extended the generated migration inventory to recognize marked root specs. It now
  records 301 active root cases across 38 files and identifies all three promoted
  cases as `migrated-semantic-world`.

Verification:

- Control UI typecheck, architecture checks, inventory check, and diff check: passed;
- focused migrated spec: 3 passed using three workers;
- migrated spec plus five-bench isolation stress: 5 passed using four workers;
- full E2E rerun: 325 passed and 9 intentionally skipped, with two unrelated timing
  failures; exact focused rerun of both failures: 2 passed.
