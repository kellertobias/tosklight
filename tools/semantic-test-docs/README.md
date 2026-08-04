# Semantic test documentation compiler

This compiler reads the TypeScript AST of every repository-root Playwright spec
marked `// @bench-semantic-world`. It does not import test modules, register
Playwright tests, launch browsers, or start a Light server.

Run:

```sh
npm run docs:semantic-tests:write
npm run docs:semantic-tests:check
npm run test:semantic-test-docs
```

The write command generates, outside source-controlled documentation:

- `.artifacts/semantic-tests/semantic-test-catalog.v1.json`, the machine-readable catalog;
- `.artifacts/semantic-tests/semantic-test-catalog.html`, the self-contained searchable view.

`npm run pages:generate` also compiles both files into
`semantic-tests/` inside the assembled GitHub Pages artifact and links the HTML
catalog from the public landing page.

Pass `--results <playwright-json>` and an explicit alternate `--output-dir`
directly to `cli.mjs` to merge an existing Playwright JSON report. Run-specific
status and durations cannot overwrite the deterministic checked artifacts. Observed
`lastRun` status remains separate from the expected outcomes compiled from source:

```sh
node tools/semantic-test-docs/cli.mjs --write \
  --output-dir .artifacts/test/semantic-docs \
  --results report.json
```

`narration-catalog.mjs` is the single translation seam for public `t.*` helpers and
their tested surfaces. The compiler preserves unsupported helpers, dynamic values,
and control flow as source-linked diagnostics. Add an explicit narration or a safe
static-expression rule instead of guessing what an expression means.
