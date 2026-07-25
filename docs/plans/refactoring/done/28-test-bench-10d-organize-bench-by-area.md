# 28 test bench 10d — Organize bench modules by operator area

## Outcome

Replace the flat `apps/control-ui/e2e/bench` module directory with cohesive area
folders so maintainers can find bench infrastructure by the larger operator concern it
supports. Preserve all test behavior and public helper semantics.

This chunk is explicitly prioritized by the maintainer ahead of the remaining 10c
scenario migrations and chunk 29.

## Work

- Define a small, durable folder taxonomy for shared bench infrastructure and
  operator areas such as show setup, window system, command and selection,
  programmer, encoders, groups and presets, playbacks, output, and hardware.
- Move each bench source and its focused unit test together.
- Update every internal and external importer, path-sensitive test configuration,
  architecture boundary, source-size rule, and current documentation reference.
- Keep imports explicit; do not add a broad barrel file that hides dependency
  direction or preserves the flat layout by indirection.
- Make no behavior changes and preserve the root Playwright scenario locations.

## Done gate

- No TypeScript source remains directly in the bench root.
- Every moved module lives in a cohesive area folder and every importer resolves.
- Bench TypeScript, focused acceptance-intent unit tests, architecture/source-size
  checks, and the full Playwright suite pass with no net new regressions.
- Repository searches find no stale references to the former flat module paths except
  historical completed plans whose paths describe the tree at completion time.

## Result

Completed 2026-07-25.

- Moved all 92 bench TypeScript files into 11 cohesive area folders: `core`,
  `window-system`, `show`, `show-setup`, `command-selection`, `programmer`,
  `encoders`, `groups-presets`, `playbacks`, `output`, and `hardware`.
- Updated internal imports, every current external importer, the explicit
  acceptance-intent Vitest inventory, bench typecheck command, architecture boundary,
  current testing documentation, and code-tour references. Historical completed plans
  retain the paths that were accurate when those chunks landed.
- Kept focused tests beside their implementation. The acceptance-intent inventory
  remains 26 test files and now passes all 161 tests. Its command-line mock also gained
  the current `Locator.isVisible()` seam exposed by production command handling.
- Verification passed: `npm run test:bench-types`; the acceptance-intent Vitest suite
  (26 files, 161 tests); `npm run test:architecture`; and the control-ui TypeScript/Vite
  build. The full Playwright run finished with 326 passed, 9 skipped, and one
  `BENCH-GROUP-001` revision-observation timeout; the required isolated rerun passed
  (1/1), confirming a suite-only timing flake rather than a structural regression.
- No follow-up plan was required. Concurrent chunk-29 compiler work remained separate
  from this commit.
