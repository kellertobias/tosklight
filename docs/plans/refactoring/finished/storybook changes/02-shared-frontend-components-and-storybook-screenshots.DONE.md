# Shared Frontend Components and Deterministic Screenshots

## Status and source contract

Completed in the Storybook lane. Component ownership, geometry, package boundaries, and
Storybook-first sequencing
are defined by
[`../../../Next/58-shared-frontend-libraries.md`](../../../Next/58-shared-frontend-libraries.md).
This queue item adds the required documentation-screenshot migration after accepted stories exist.

The current workspace and Storybook location decision is recorded in
[`storybook-application-components.DONE.md`](storybook-application-components.DONE.md).
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
- A clean CI run validates the complete screenshot manifest and regenerates every
  Storybook-owned candidate without launching the Light server. Entries explicitly owned by
  `live-app` remain in the focused runtime capture gate required below.
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

Compatibility modules and thin adapters kept the current application paths building during
adoption. The checkpoint was accepted, and the screenshot manifest, Help screenshot CI migration,
production Storybook adoption, and superseded presentation-code cleanup are complete.

## Result

- Created `apps/ui-library` as the single `@tosklight/ui` production package. The desktop now
  imports its public component families directly; the temporary app-local re-export adapters and
  the retired `?ui-kit=1` catalog route are gone.
- Moved shared component CSS into package-owned entry points and added an architecture audit that
  rejects exact package/app rule duplication. The final audit removed 157 verbatim duplicate
  application rules while retaining feature- and window-specific overrides.
- Centralized application and package modal registration, ordering, top-only Escape, focus
  restoration, and portal ownership. Registered application layers now portal to the shared stack
  root, so deeply nested workflows cannot fall behind ancestor overflow or stacking contexts.
- Added `docs/help/screenshot-manifest.json` as the complete 47-file contract. Twenty images now
  map to real deterministic application stories; twenty-seven remain explicitly marked
  `live-app` with null story IDs and a reviewed runtime/workflow reason instead of a substitute
  composition.
- Added a serial static-Storybook Playwright capture gate. It verifies manifest completeness and
  current story IDs, freezes time and animations, applies declared interaction metadata, rejects
  REST/WebSocket traffic and browser errors, checks PNG dimensions and non-blank content, writes
  inspection candidates below `.artifacts/test/help-screenshots/storybook`, and rejects
  unreviewed pixel differences.
- Root commands now separate the non-mutating Storybook check,
  `test:help-screenshots:update`, and the smaller `test:help-screenshots-live` production
  composition path. The live path can update only entries explicitly owned by `live-app`.
- CI now builds and checks screenshots in a dedicated job and uploads one reviewed
  `help-screenshots` artifact. Both the manual and Pages jobs consume that same artifact.
- Added deterministic production stories for the application shell and controls, complete
  production windows, Setup sections, Text Editor, Virtual Playbacks, marketing composition, and
  representative modal workflows. Public-export coverage requires every package component to name
  a representative story.
- Re-audited all 47 screenshots after the last component and modal changes. The Storybook-owned
  Text Editor and Virtual Playbacks images now fill their documented frames, and Virtual
  Playbacks uses its configured rows and columns to fill the pane without changing square sizing
  for ordinary pool cards.

Final verification on 2026-07-26, including the post-completion O2/UI-refactoring audit:

- The Storybook behavior/catalog run passed 219 of 219 checks; the separately rerun reviewed
  screenshot check passed, completing the 220-check Storybook gate.
- `npm run test:help-screenshots` — the complete 47-entry non-mutating manifest check passed.
- `npm run test:help-screenshots-live` — the 27-image production composition/workflow set passed.
- `npm run test:unit` — architecture, bench, UI package, desktop, hardware-controls, Rust workspace,
  generated wire contracts, and doc tests passed.
- UI package tests passed 108 of 108; desktop tests passed 1,993 of 1,993.
- UI package and desktop typechecks passed.
- `npm run manual` built and verified the 142-page PDF and offline HTML manual.
- `LIGHT_REUSE_MANUAL=1 npm run pages:generate` built the complete Pages output.
- The 47-image contact sheet was visually reviewed with no blank, clipped, or substitute
  screenshot accepted, and `git diff --check` passed.
- `npm run open` built both Tauri applications and opened the packaged development desktop.
  `/api/v2/readiness` returned ready and `/api/v2/bootstrap` returned HTTP 200.

The remaining 27 `live-app` images are not unfinished Storybook work. They intentionally cover
complete-desk composition, authoritative session/show state, or production workflows whose
runtime dependencies a deterministic story must not fabricate.

Completion commit: `feat: complete shared frontend Storybook migration`.
