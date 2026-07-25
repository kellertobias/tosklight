# Documentation map

This directory is the source of truth for product, engineering, acceptance, and
planning documentation across the Tosk application suite.

## Documentation categories

- `help/` contains operator-facing help and the manual source. Number chapter
  directories and pages in reading order, keep each chapter opener in
  `index.md`, and make the first H1 suitable as its visible title.
- `testing/` contains human-readable acceptance scenarios. Executable
  Playwright coverage and its shared bench live at the repository root under
  `tests/`.
- `engineering/` contains architecture, API, build, testing, and contributor
  documentation.
- `plans/` contains proposed and active work. Plans describe intent and
  acceptance criteria; completed behavior belongs in help and engineering
  documentation.
- Markdown files directly in `docs/` are cross-cutting contracts, audits, or
  references that do not belong to one category above.

## Product areas

Use these area names consistently in documentation:

- `suite` for repository-wide behavior shared by multiple products.
- `light` for the lighting-control desktop and headless applications.
- `media` for the Media server and its web interface.
- `viz` for the visualization editor and renderer.

## Plan locations and names

Plans move through `plans/Next`, `plans/Later`, `plans/Manual Work`, and
`plans/Done`. Refactoring plans follow the workflow documented in
`plans/refactoring/README.md`.

Use a single numeric sequence across plan statuses and this filename grammar:

```text
NN-kebab-case[.minor][.DONE].md
```

- `NN` is the plan number.
- Use lowercase kebab-case for the subject.
- Add `.minor` only for a small, independently deliverable polish item.
- Add `.DONE` only after implementation and verification are complete.

Do not classify changes to live output, persisted data or migrations, timing,
public APIs, OSC or hardware parity, or broad settings as minor work. Preserve
explicit stop and manual-verification gates when moving a plan. Existing
historical filenames may predate this scheme; use this convention for new plans
and when an active plan is deliberately renamed.
