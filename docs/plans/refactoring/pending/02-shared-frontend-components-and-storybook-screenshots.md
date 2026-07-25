# Shared Frontend Components and Deterministic Screenshots

## Status and source contract

Pending. Component ownership, geometry, package boundaries, and Storybook-first sequencing are
defined by
[`../../Next/58-shared-frontend-libraries.md`](../../Next/58-shared-frontend-libraries.md).
This queue item adds the required documentation-screenshot migration after accepted stories exist.

Estimated effort: 2–4 Codex days, including application adoption and screenshot migration.

## Required work

### Extract production components

1. Recreate `packages/ui` as a tracked workspace package from existing production sources—not from
   the generated `dist/` or `storybook-static/` remnants currently ignored in that directory.
2. Characterize current labels, CSS cascade, 24×18 grid geometry, pointer/touch/keyboard behavior,
   modal stacking, Fixture Sheet tables, faders, encoders, playbacks, and pool grids.
3. Move directly reusable components and split coupled components into package views plus
   `apps/light-desktop` adapters.
4. Keep the package free of server APIs, Tauri, application contexts/controllers, persisted desk
   state, and `WindowRegistry`.
5. Add deterministic Storybook stories and interaction tests in the order specified by plan 58.
6. Review and accept the Storybook package before replacing the live application imports.
7. Adopt the accepted package in the desktop app, preserving exact operator behavior and removing
   superseded app-local presentation code.

### Generate documentation screenshots from accepted stories

1. Define a tracked screenshot manifest mapping stable help-image filenames to Storybook story IDs,
   viewport, theme, hardware/software mode, and optional interaction state.
2. Build Storybook in CI, serve the static build, and capture the manifest serially with Playwright.
3. Use deterministic mock view models, fonts, time, animation state, images, and callback outcomes;
   no screenshot story may require a live server or mutable show.
4. Preserve current filenames under `docs/help/assets/screenshots` so Help Markdown, PDF, HTML
   manual, and website consumers do not fork.
5. Make the Help screenshot CI job consume the Storybook build and upload the same reviewed image
   artifact used by the manual and Pages jobs.
6. Retain a smaller real-app screenshot/visual acceptance set for end-to-end composition,
   connectivity, Tauri, and workflows that cannot truthfully be represented by a component story.
7. Fail CI for a missing manifest entry, blank story, console error, unstable dimensions, or an
   unreviewed screenshot diff.
8. Update `docs/help/99-Development/04-manual-and-help-screenshots.md`,
   `docs/engineering/build-and-test-commands.md`, and root scripts to make Storybook the documented
   deterministic screenshot source.

## Acceptance and verification

- `packages/ui` has tracked source, package tests, typecheck, Storybook build, and explicit exports.
- Every reusable component rendered in Storybook is the same implementation consumed by an app.
- Mock stories cover software and hardware presentation where both exist.
- A clean CI run regenerates the complete screenshot manifest without launching the Light server.
- Manual and Pages builds consume those images and remain visually correct.
- Focused live-app Playwright and packaged desktop checks prove adapter integration.

## Sequencing rule

Do not create the integration worktree or switch the live app before the extracted Storybook
components have been tested, reviewed, and accepted.
