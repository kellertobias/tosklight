# Shared Frontend Components and Deterministic Screenshots

## Status and source contract

Working in the concurrent Storybook lane. This plan no longer blocks the independent numbered
backend queue. Component ownership, geometry, package boundaries, and Storybook-first sequencing
are defined by
[`../../../Next/58-shared-frontend-libraries.md`](../../../Next/58-shared-frontend-libraries.md).
This queue item adds the required documentation-screenshot migration after accepted stories exist.

The current workspace and Storybook location decision is recorded in
[`storybook-application-components.WORKING.md`](storybook-application-components.WORKING.md).
The UI library moves to `apps/ui-library`, and the contained Storybook application renders both
library stories and production `apps/light-desktop` components through deterministic providers.

## Queue separation

This work has an explicit operator-review checkpoint and is being completed by the frontend
component agent. Keeping it in the numbered queue's single `doing/` slot would serialize unrelated
backend refactoring without protecting any shared files or runtime state.

Until this Storybook lane is accepted and its application adoption is stable:

- plan 03 waits because it changes the shared pool-card and pool-grid presentation contracts;
- plan 04 waits because it changes application pool adapters and cross-surface pool interaction;
- plan 05 waits because it changes frontend providers, stores, hydration, and Storybook harness
  boundaries; and
- plan 06 is the first independent numbered plan and may proceed concurrently because it owns the
  server-side active-show mutation boundary and its focused backend verification.

Estimated effort: 2–4 Codex days, including application adoption and screenshot migration.

## Required work

### Extract production components

1. Create `apps/ui-library` as a tracked workspace package from existing production sources—not
   from generated `dist/` or `storybook-static/` remnants.
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

- `apps/ui-library` has tracked source, package tests, typecheck, Storybook build, and explicit
  exports while retaining the package identity `@tosklight/ui`.
- Every reusable component rendered in Storybook is the same implementation consumed by an app.
- Mock stories cover software and hardware presentation where both exist.
- A clean CI run regenerates the complete screenshot manifest without launching the Light server.
- Manual and Pages builds consume those images and remain visually correct.
- Focused live-app Playwright and packaged desktop checks prove adapter integration.

## Sequencing rule

Do not create the integration worktree or switch the live app before the extracted Storybook
components have been tested, reviewed, and accepted.

## Review checkpoint

The pre-adoption package is ready for operator review:

```sh
npm run test:ui-package
npm run test:storybook
npm run storybook
```

The stories cover production controls and forms, keyboard and numpad input, Window Kit
compositions, the real 24×18 desktop geometry, Fixture Sheet row states, software and hardware
faders, touch and hardware encoders, nested modals, touch and hardware playbacks, and group,
preset, and cuelist cards. They use package-owned deterministic models and make no REST or
WebSocket requests. The Storybook manager, docs canvas, and story canvas use the application's
authoritative `--bg` token (`#07090c`), and the Storybook gate compares the package token with the
desktop application token and the computed rendered background.

Compatibility modules and thin adapters keep the current application paths building during this
review phase. Per the source contract, full live-application adoption, screenshot-manifest
creation, Help screenshot CI migration, and removal of superseded app-local presentation code are
blocked until this checkpoint is explicitly accepted.
