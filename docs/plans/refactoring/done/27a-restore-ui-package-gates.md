# 27a — Restore or retire the missing shared UI package gates

Status: complete.

## Context

Chunk 27's acceptance audit found that the root scripts added before the refactor capstone target a
workspace that does not exist in the tracked repository:

```sh
npm run storybook:build
npm run test:ui-package
```

Both resolve `--workspace @tosklight/ui`, but `packages/ui/` has no tracked source or
`package.json`. Its local ignored content is only generated `dist/` and `storybook-static/`
artifacts and cannot be treated as recoverable source. The Control UI package also has no current
Storybook scripts or configuration.

This is not evidence that the runtime refactor is wrong, but it makes two documented release gates
non-executable and leaves the intended ownership of the shared UI library ambiguous.

## Decision needed

Choose one endpoint:

1. restore the intended `@tosklight/ui` source package, manifest, Storybook configuration, stories,
   tests, and Control UI consumption; or
2. retire the nonexistent workspace and its root scripts, then explicitly document which current
   Control UI component tests/build replace those gates.

Do not reconstruct source from ignored generated JavaScript/declarations or Storybook output.

## Decision

Option 2, selected during execution on 2026-07-24: retire the nonexistent workspace and document
the current app-local Control UI component tests, typecheck/build, and browser acceptance as the
supported contract. The tracked repository has no package source, manifest, Storybook
configuration, or consuming imports that would justify restoring option 1.

## Verification

For option 1:

```sh
npm run storybook:build
npm run test:ui-package
npm run test:unit
npm run test:e2e-ui
```

For option 2:

```sh
npm run test:unit
npm run test:e2e-ui
```

Update `README.md`, `docs/engineering/build-and-test-commands.md`, and the CodeSafari UI Library
component guide to describe the chosen, executable contract.

## Result

Completed on 2026-07-24 with option 2.

- Removed the nonexistent root npm workspace plus the `storybook`, `storybook:build`, and
  `test:ui-package` scripts. The lockfile no longer contains the stale `@tosklight/ui` workspace or
  its Storybook-only dependency graph; `npm ci --dry-run --ignore-scripts` accepted the result and
  reported that 239 stale packages would be removed.
- Documented the supported app-local contract in `README.md`,
  `docs/engineering/build-and-test-commands.md`, and the CodeSafari UI Library guide:
  reusable primitives remain under Control UI and are protected by component tests,
  typecheck/production build, and real-browser UI acceptance.
- `npm run test:unit` passed on a complete rerun: 282 Vitest files / 2,000 tests plus architecture,
  Control UI production build, and all Rust suites. One selection WebSocket test failed once during
  the first parallel Rust run, passed in a corrected focused rerun, and passed again in the complete
  gate.
- `npm run test:e2e-ui` completed with 103 passed / 5 intentionally skipped and one unrelated
  TEXT-015 cleanup/recovery failure; the exact `tests/15-text-editor.spec.ts` rerun passed 2/2.
- The pinned CodeSafari validator had passed the 12-tour/47-step tree before this prose-only guide
  update. Its re-fetch was denied by the external execution approval boundary after a sandbox DNS
  failure; no frontmatter, step, source anchor, or link was changed in this chunk.
