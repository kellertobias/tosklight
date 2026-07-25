# 10a — Existing UI-test inventory and enforcement

## Outcome

Create a checked, human-readable inventory of every repository-root Playwright
case and establish the architecture boundary that distinguishes migrated semantic
scenarios from justified low-level protocol/layout/process cases.

## Work

- Generate or maintain one inventory row per test case with source file, scenario
  identity, contract, surfaces, helper family, artifacts, constraints, and status.
- Classify low-level helpers as keep, wrap, replace, or remove.
- Add a narrow architecture check for files explicitly marked migrated. It rejects
  Playwright `Page`/`Locator`, `ApiDriver`, raw HTTP, reducer actions, CSS/internal
  pane selectors, fixture UUID resolution, coordinate clicks, encoder slots, and
  raw show-object writers.
- Document how authors mark a scenario migrated and how protocol/layout/process
  exceptions remain explicit.
- Update `docs/testing/README.md` with the recipe, route-seed, reporting, and
  migration-boundary author contract.

## Done gate

- Every current root case appears in the inventory.
- Inventory generation/checking fails when a root case is added without a row.
- The architecture rule has positive and negative unit fixtures.
- Existing low-level cases are not falsely called migrated.
- Architecture, unit, source-size, and focused inventory checks pass.

## Result

Completed on 2026-07-25.

- Added a deterministic generated inventory covering 298 cases across all 37
  default-catalog root specs, plus the two intentionally excluded serial
  screenshot/video entrypoints. Each row records its contract, surfaces, helper
  family, artifacts, execution constraint, and migration status.
- Classified semantic bench adapters as internal keepers, normalized test oracles
  as temporary wrappers, root low-level interaction helpers for replacement, and
  selectors/coordinates/UUIDs/slots/raw show writers for removal after their final
  consumer.
- Added the opt-in `// @bench-semantic-world` architecture boundary. Marked root
  specs cannot import or use Playwright interaction types, `ApiDriver`, raw HTTP,
  reducers, selectors, coordinates, fixture UUIDs, encoder slots, or mutable show
  object helpers. Positive, negative, and unmarked-boundary unit fixtures pass.
- Updated the testing author guide with recipes, seeded route replay, failure
  evidence, inventory regeneration, the migration marker, and explicit justified
  low-level boundaries.

Verification:

- inventory write/check: 298 active root cases;
- semantic-world boundary tests: 3 passed;
- architecture, command-boundary, private-boundary, source-size, bench type, and
  Control UI build gates: passed;
- full unit run passed through all preceding crates, then exposed one reproducible
  pre-existing server test assumption about a parent fixture expanding to logical
  heads. The focused rerun confirmed it; repair is isolated in pending chunk 10a1.
