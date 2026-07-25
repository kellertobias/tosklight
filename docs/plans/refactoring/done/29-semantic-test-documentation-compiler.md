# 29 — Semantic test documentation compiler

## Outcome

Compile the marked semantic Playwright scenarios into deterministic, reviewable
documentation without importing or executing test modules. Publish a versioned JSON
contract and a self-contained searchable HTML view for maintainers and operators.

This chunk is explicitly prioritized by the maintainer ahead of the remaining 10c
scenario migrations. It does not change their order or scope.

## Work

- Parse TypeScript source with the compiler API; do not start Playwright, browsers,
  servers, or the desktop application.
- Discover every root spec marked `// @bench-semantic-world` and include each
  `scenario(...)` registration exactly once.
- Translate public `t.*` calls through one centralized narration catalog. Keep unknown
  helpers and non-static expressions as visible diagnostics rather than inferred prose.
- Record scenario ID, title, repository-relative source location, narrated steps,
  expected outcomes, tested surfaces, and the status from the generated migration
  inventory.
- Optionally merge Playwright JSON results into a distinct last-run field without
  conflating observed execution status with expected outcomes.
- Add deterministic write/check commands, repository scripts, stale-output enforcement,
  focused unit coverage, and checked generated JSON/HTML artifacts.

## Done gate

- Every marked semantic scenario appears exactly once in both generated artifacts.
- Static unresolved-expression fixtures prove diagnostics stay visible and deterministic.
- Narration, migration-inventory matching, result merging, HTML escaping/search data, and
  stale-output checks have focused unit coverage.
- Architecture and relevant unit/tooling checks pass without executing Playwright or
  changing existing test behavior.

## Result

Completed on 2026-07-25.

- Added a TypeScript-compiler-AST pipeline that discovers the ten marked root specs
  without importing them and documents all 27 scenario registrations exactly once.
- Added an explicit narration catalog for every current semantic helper call. The
  generated catalog contains 345 human-readable action steps, 168 expected outcomes,
  tested-surface evidence, exact migration-inventory matches, and source locations.
- Published deterministic schema-v1 JSON and a self-contained searchable HTML page.
  Forty-nine dynamic-expression and control-flow diagnostics remain intentionally
  visible; no helper path is missing from the narration catalog.
- Added optional Playwright JSON result merging to a required alternate output
  directory, keeping observed last-run state separate from expected outcomes and out of
  the checked source-only artifacts.
- Added write/check/test npm commands and made the architecture gate enforce both
  generated outputs statically. The previous Playwright-backed inventory check remains
  available as an explicit generator but is no longer part of the architecture gate.

Verification:

- semantic documentation tests: 7 passed;
- semantic documentation write/check: 27 scenarios, deterministic and current;
- architecture gate: passed, including 27 supporting Node architecture tests and the
  source-size ratchet;
- semantic-world boundary tests: 3 passed;
- bench TypeScript contract: passed;
- `git diff --check`: passed.

No Playwright process, browser, Light server, or desktop application was started.
