# 27a — Restore or retire the missing shared UI package gates

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
