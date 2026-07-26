# UI Library and Application Components in Storybook

## Status

Completed on 2026-07-26. This governing handoff moved the production UI workspace out of the
superseded `packages/ui` location and established the contained application Storybook.

## Workspace decision

Move the actual UI library workspace from `packages/ui` to `apps/ui-library`.

Keep its existing production package identity, `@tosklight/ui`, so application imports do not need
to encode the repository layout.

Contain the Storybook application inside that workspace:

```text
apps/
  light-desktop/
    src/
      functional ToskLight components and colocated stories

  ui-library/
    package.json                 @tosklight/ui production library workspace
    src/
      reusable low-level components and colocated stories
    storybook/
      config/                    Storybook main, preview, manager and global CSS
      fixtures/                  deterministic data builders
      providers/                 deterministic application provider harnesses
      tests/                     Storybook Playwright tests
      playwright.config.ts
      build.mjs
```

The Storybook subdirectory is a contained visual-catalog application and screenshot host. It does
not own production visual components and does not need to be a second npm workspace package.

After the move:

- remove the empty `packages/ui` path;
- remove the root `playwright.storybook.config.ts`;
- move root Storybook-only tests and build helpers into `apps/ui-library/storybook`;
- remove the root `packages` directory if no other package uses it;
- update workspace globs, lockfile paths, TypeScript references, architecture checks, CI, artifact
  helpers, and documentation; and
- keep only thin root commands that delegate to the UI library workspace.

## Ownership boundary

Low-level reusable components live in `apps/ui-library/src`, including:

- the form and field system;
- common controls and standard search;
- modal stack, modal frames, title bars, and modal inputs;
- generic tables and Fixture Sheet-capable table primitives;
- touch faders and direct-value inputs;
- individual touch/hardware encoders and encoder-section primitives;
- generic pool cards and pool-grid primitives;
- shared window chrome and scrolling primitives; and
- shared desktop geometry where it is genuinely application-independent.

Functional ToskLight components remain in `apps/light-desktop`, including:

- the Dock and complete shell;
- the real Stage 2D and 3D views;
- the command section and its Programmer/Playbacks modes;
- parameter, playback, keypad, Highlight, Step, Preload, and hardware-connected compositions;
- complete application windows and workflows; and
- product controllers, providers, APIs, persistence, Tauri, and `WindowRegistry`.

Do not move a product component into the UI library merely to render it in Storybook.

## Storybook discovery

The contained Storybook application must discover both:

- `apps/ui-library/src/**/*.stories.tsx`; and
- `apps/light-desktop/src/**/*.stories.tsx`.

Package stories cover low-level component contracts. Application stories render the same
functional components imported by the live desktop application.

Add application stories for at least:

- the complete Dock and its important modes;
- representative desktop arrangements;
- the real Stage 2D and deterministic 3D views;
- the command section in Programmer and Playbacks modes;
- software-only and hardware-connected control layouts;
- parameter families, encoders, playback banks, keypad, command line, Programmer Fade, Preload,
  Highlight, and Step controls;
- Group, Preset, Cuelist, Cues, Virtual Playbacks, Fixture Sheet, Channels, Dynamics, File Manager,
  Text Editor, DMX, Help, Patch, Setup, and Development windows;
- application-owned modal workflows; and
- complete Help and marketing compositions.

## Storybook harness boundary

Storybook may own deterministic fixtures and provider harnesses, but never a parallel visual
implementation.

Prefer the narrowest truthful harness:

1. Render the production component directly when it accepts typed props.
2. Supply deterministic versions of existing application providers when the component consumes
   context or feature stores.
3. Split controller logic from rendering inside `apps/light-desktop` when a provider graph would
   otherwise duplicate the runtime.
4. Mock the component's data boundary, not its rendered output.

Application stories must perform no REST, WebSocket, OSC, filesystem, Tauri, or mutable-show work.

The architecture rule is intentionally asymmetric:

- `apps/ui-library/src` must never import `apps/light-desktop`;
- `apps/light-desktop` may import `@tosklight/ui`;
- `apps/ui-library/storybook` may load desktop stories and deterministic application harnesses;
  and
- production application code must never import the Storybook subdirectory.

## Styling and visual fidelity

- Load the same UI-library and application stylesheet order used by the desktop application.
- Use the exact application background, fonts, icons, assets, density, and breakpoints.
- Preserve hardware/software and Programmer/Playbacks mode distinctions.
- Render the production 3D Stage with fixed scene data, fixed camera, disabled animation,
  deterministic asset readiness, and a stable screenshot-ready signal.
- Do not replace WebGL/3D output with a diagram or static image in a story claiming to represent
  the Stage.
- Do not create Storybook-only visual states that the live application cannot produce.

## Remove mock complete windows

Delete the current mock complete-window stories after equivalent desktop application stories
exist.

Remove `MockWindow`, fabricated DMX and Help layouts, the virtual-playback fader-bank substitute,
approximate complete pool windows, and other parallel implementations. Primitive UI-library
stories remain.

## Screenshot consumers

The deterministic screenshot manifest may reference:

- UI-library stories for focused component documentation; and
- desktop application stories for Help panes, complete workflows, Dock, command section, real
  Stage views, operator layouts, and marketing compositions.

The same reviewed image artifact should feed Help, the manual, Pages, and marketing when they need
the same state. Keep a smaller packaged-app acceptance set for connectivity, native windows,
Tauri, cross-surface routing, persistence, and workflows Storybook cannot prove.

## Root cleanup

Root files should be limited to repository-wide orchestration. Storybook-specific implementation
belongs below `apps/ui-library/storybook`.

Root `package.json` may retain convenience commands such as:

```json
{
  "storybook": "npm run storybook --workspace @tosklight/ui",
  "storybook:build": "npm run storybook:build --workspace @tosklight/ui",
  "test:storybook": "npm run test:storybook --workspace @tosklight/ui"
}
```

Do not retain duplicate root Storybook configs, Playwright configs, tests, or build scripts after
the migration.

## Verification

- `apps/ui-library` remains resolvable as the single `@tosklight/ui` workspace.
- Desktop production imports continue using `@tosklight/ui` and never use relative paths into the
  library.
- Storybook index contains UI-library and desktop application story roots.
- Architecture checks enforce the asymmetric import rules above.
- No production code imports Storybook configuration, stories, fixtures, or providers.
- Every functional story renders a component imported by the live application.
- No complete application window is duplicated in UI-library or Storybook-only markup.
- Every story makes no unexpected REST or WebSocket requests and emits no console errors.
- Repeated clean Storybook builds produce deterministic fixture output.
- Root Storybook-specific files are removed and root commands delegate successfully.
- CI, Pages, Help/manual screenshots, and marketing screenshots consume the relocated build
  without changing artifact ownership or published paths.

## Relationship to focused corrections

Generic pool grids, standard search, encoder release, table separators, forms, modals, inputs,
faders, and encoders remain UI-library work under `apps/ui-library/src`.

Virtual Playbacks, DMX, Help, complete pool windows, Dock, Stage, command controls, and other
functional surfaces remain desktop-application work and are loaded by the contained Storybook
application. Focused correction notes in this folder must follow this boundary.

## Result

Completed result as of 2026-07-26:

- `apps/ui-library` is the single `@tosklight/ui` workspace and its contained Storybook discovers
  package stories plus colocated `apps/light-desktop` application stories.
- Production presentation seams now cover the real shell (`AppShellView`), Product Demo
  composition (`ProductDemoSurfaceView` and `DemoApplicationScreenView`), deterministic DMX
  presentation (`DemoDmxGridView`), and demo playback surface (`DemoPlaybackControlsView`).
  Runtime controllers still supply those same views in the live application.
- `ToskLight/Marketing / Complete Product Demo` composes the production Product Demo layout,
  shell, Dock, desktop grid, Stage 2D and 3D views, DMX view and output grids, playback controls,
  command line, and keypad from deterministic typed fixtures. It mounts `AppProvider` but not
  `ServerRuntime`, Patch transport, REST, WebSocket, OSC, filesystem, or Tauri boundaries.
- `ToskLight/Modal workflows` renders the production Playback Configuration and existing-target
  Record workflows. The Playback story supplies the existing Show Objects store boundary and
  exercises Function, Behavior, and Layout tabs without mutable-show writes.
- Application-owned stories now cover the required Dock, representative desktop, Stage, command
  and control modes, windows, modal workflows, Help, and marketing families. The former fabricated
  complete-window story module was removed after production stories replaced it.

Verified evidence:

- `npm run typecheck --workspace @tosklight/light-desktop` passed.
- `npm run typecheck --workspace @tosklight/ui` passed.
- `npm run test:storybook --workspace @tosklight/ui -- --grep 'ToskLight/(Marketing|Modal workflows)'`
  built Storybook and passed all 3 deterministic render checks; the shared request guard observed
  no REST or WebSocket requests and the console/error guard remained empty.
- The focused Playwright test
  `production marketing and application modal stories preserve their real compositions and workflows`
  passed, including the 2,048-cell DMX composition, real 3D Stage canvas, production shell/Dock,
  Playback Configuration tab transitions, and Record Merge action.
- The focused Product Demo playback unit suites passed 19 of 19 tests.

Deliberate residual boundary:

- Storybook renders the full visible shell through the production `AppShellView` with deterministic
  production surface slots. It does not mount the `AppShell` runtime controller itself because that
  controller intentionally owns layout persistence, screen clients, live command/programmer
  authorities, connection state, recovery, and other service-backed lifecycles. Those integrations,
  native overlays, persistence, Tauri routing, and live connectivity remain packaged-app acceptance
  responsibilities.
- The complete shared Storybook gate passed all 217 Playwright checks. Architecture, CI workflow
  validation, Pages generation, Help/manual generation, screenshot ownership, and marketing
  screenshot consumer checks also passed in the integrated run.

Final adoption follow-up:

- The command line now follows the same boundary. Package-owned `CommandLine` renders the
  complete command input, mode switch, DMX/timecode/blackout status, history, error, Record, and
  Preload surface without importing application state, server contexts, or transports.
  `CommandLineBar` is the runtime adapter that supplies authoritative values and actions, while the
  dedicated interactive Storybook story exercises the same component without issuing API or
  WebSocket requests. The former parallel desktop command-section components were removed.
- A live-versus-Storybook comparison caught parallel touch and hardware playback-card markup in
  the desktop adapter. The desktop now renders the package-owned playback card views directly and
  retains only its runtime/controller boundary and overlays.
- The application playback stories and the live adapter suite assert the package component
  markers for both touch and hardware modes. This turns the “same production component” boundary
  into executable evidence instead of relying on matching class names.
- The same audit removed the live desktop's parallel grid and pane markup. `DeskGrid` and `Pane`
  now provide application state and window content to `GridDesktop` and `PaneView`, which are the
  components rendered by the desktop Storybook stories.
- The command-area boundary now includes the complete lower desk, not only its command row.
  Package-owned `CommandSection` performs the Programmer/Playbacks and
  software/hardware-connected switch, while callback-driven `ProgrammerKeypadView`,
  `PlaybackToolsView`, and `HardwareControlSummaryView` keep live authorities in desktop
  adapters. The production `ControlSection` uses this shared composition directly.
- `ToskLight/Command section` provides a configurable serverless story plus fixed
  software Programmer, software Playbacks, hardware Programmer, and hardware Playbacks states.
  They include the command line, encoder families, keypad, representative cue-list/group/speed/
  Dynamic/special/empty playbacks, cue rows, Page selection, Programmer Fade, Cue Fade, command
  keys, and Speed Groups A–E. The application-shell and Product Demo stories reuse the same
  fixture instead of partial keypad or duplicate-encoder substitutes.
- The final production-parity review removed the fabricated “Intensity encoders” section,
  switched the Storybook fixture to the desktop `ParameterControlView`, made the encoder surface
  consume the available height, restored HIGH/PREV/NEXT/ALL and SET/SHIFT interaction feedback,
  rendered both configured playback rows with production Page chevrons, and made the global
  software/hardware context switch both `CommandLine` and the complete `CommandSection`.
- Hardware encoder target classes are now namespaced. This prevents the application-wide
  `.primary` style from painting a false cyan background inside the hardware encoder while
  preserving the standalone hardware encoder presentation.
- The Programmer keypad now owns the production semantic key treatment: command/action keys are
  amber, Enter is cyan, and Clear exposes idle, selected-fixtures, and blinking active-values
  states. The configurable Command Section story also controls PREV/NEXT availability and the
  Preload Go state, including the amber command line. The Programmer/Playbacks mode button no
  longer uses the negative left offset.
- The Preload Go button now keeps pending detail in its tooltip and renders only `PRELOAD GO`.
  The Programmer/Playbacks toggle uses the midpoint geometry: 116px wide with no horizontal
  offset.
- Dynamics now keeps only cyan text while idle and gains its cyan outline when active. Fixture
  Sheet and Patch stories use the repository-owned SVGs from `assets/icons/fixture-type`; the
  Patch table also uses that catalog-backed mapping in production instead of parallel inline
  glyphs.
