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
