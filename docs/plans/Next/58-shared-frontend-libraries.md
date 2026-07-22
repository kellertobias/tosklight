# Shared Frontend Component Library

## Status

**Specification only.** This plan defines how existing Control UI components should be extracted into a reusable repository-local package. It does not move components, change Storybook, alter application imports, create a worktree, or change runtime behavior.

## Problem

The Control UI already contains the authoritative ToskLight controls, window chrome, grid-based desktop, tables, encoder and fader surfaces, modal conventions, and operator-specific interaction behavior. Reimplementing those components in a separate package produces a second visual and behavioral system that does not match the application.

The package must therefore be created primarily by extracting the existing implementation. Storybook must render those extracted components, not parallel approximations of them.

Not every file under `apps/control-ui/src/components` can move unchanged. Some components combine reusable rendering with application state, server access, feature controllers, persisted desk layout, or Tauri integration. Those components need a view/controller split: the existing visual and interaction surface moves into the package, while a thin application adapter retains product state and mutations.

## Goal

Create a private workspace package, `packages/ui`, that provides the composable operator-interface building blocks used by ToskLight and future repository applications.

At minimum, the package must own:

- all common buttons, form layouts, fields, validation states, selectors, pickers, switches, text inputs, number inputs, keyboard input, and numpad input;
- reusable input surfaces such as faders, individual encoders, value-entry controls, and touch/hardware display variants;
- the 24-column by 18-row desktop grid geometry and reusable pane/window placement surface;
- window frames, title bars, title information, grouped title actions, dropdown or custom toolbar slots, search, settings, navigation and information sidebars, bottom regions, and touch scrolling;
- modal and dialog frames, stacking, focus and Escape behavior, backdrop policy, explicit and programmatic close behavior, keyboard and numpad dialogs, and nested modal operation;
- generic and specialized table surfaces sufficient to reproduce the Fixture Sheet, including active, selected, nested or indented, expandable, empty, and keyboard-navigable rows;
- pool and button grids with the application's existing scaling behavior; and
- exact touch-expanded and hardware-reduced presentation surfaces for encoders and playbacks where those surfaces are reusable.

The application should compose these primitives with typed view models and callbacks. Product state, server communication, feature orchestration, and the registry of available ToskLight windows remain application-owned.

## Source-of-truth rule

Existing Control UI components and their current operator behavior are the source of truth.

1. Move an existing component when it is already reusable.
2. Split an existing component when reusable rendering is mixed with application state.
3. Add a new package component only when the required abstraction does not exist, such as a centralized modal stack manager.
4. Do not redesign, restyle, rename operator labels, or normalize geometry as part of extraction.
5. Do not maintain an app implementation and a separately rewritten Storybook implementation of the same component.

Temporary modules at old Control UI paths may re-export package components during migration. This keeps existing application imports and behavior stable while making `packages/ui` authoritative.

## Package boundaries

### `packages/ui` owns

- React presentation and local interaction state;
- package-owned view-model and callback types;
- keyboard, pointer, touch, focus, scroll, drag, resize, and modal-stack behavior that is independent of ToskLight runtime state;
- exact shared component styling, design tokens, geometry, density, and hardware/software display variants;
- Storybook stories and deterministic package-owned mock data;
- focused unit and component tests for package behavior; and
- public exports organized by component family.

### `apps/control-ui` owns

- `useApp`, `useServer`, feature contexts, stores, and reducers;
- `WindowRegistry` and the set of windows available in ToskLight;
- persisted desks, panes, grid positions, maximized state, and layout hydration;
- Tauri screen and native-window integration;
- parameter, programmer, playback, selection, patch, file, setup, and other feature controllers;
- REST, WebSocket, OSC, command, and server-callback subscriptions;
- conversion from live domain state to package view models;
- mutations invoked by package callbacks; and
- complete workflow composition for concrete ToskLight windows and dialogs.

The UI package must never import from `apps/control-ui`. It should use React and ReactDOM as peer dependencies and must not introduce a second React runtime.

## Extraction inventory

### 1. Foundations, buttons, and forms

Extract the existing controls exposed through `apps/control-ui/src/components/common/controls.tsx` and its `common/controls` modules:

- `Button`, including current variants, active and disabled states, icon-only use, loading state, and size and width options;
- `FormLayout`, `FormField`, side-label and top-label arrangements, grouped fields, help text, required state, and validation or error presentation;
- text, password, search, large-text, and multiline inputs;
- number and value inputs;
- select, multi-value choice, checkbox, switch, and segmented or toggle choices;
- icon, color, and other current pickers; and
- shared field wrappers used by setup and editor forms.

Also extract reusable common controls such as `SearchBar`, `TouchSelect`, and `FaderControls` where they have no application dependency.

Storybook must show every current button type and state before moving on to complete form examples. Form stories must cover both individual controls and representative grouped layouts.

### 2. Keyboard, numpad, and direct input

Extract the reusable behavior in `components/input/ModalInputControls.tsx` and related common input components:

- on-screen keyboard;
- numeric keypad;
- replace-on-first-input behavior;
- Enter and Escape handling;
- direct value entry;
- range or syntax entry where already supported; and
- correct interaction when opened inside an existing modal stack.

The input components receive values and callbacks. They must not know how a programmer value, patch field, or setup property is persisted.

### 3. Window primitives

Move the existing reusable primitives from `components/window-kit/WindowKit.tsx` rather than recreating them:

- `WindowHeader`;
- `WindowFrame`;
- `WindowSettings`;
- `WindowScrollArea`;
- navigation and information sidebars;
- title information and secondary information;
- grouped title actions;
- search and custom toolbar content;
- settings popovers or modal settings;
- bottom and footer regions; and
- empty states.

The public window API must support combinations of buttons, dropdowns or custom controls, search, title information, and settings without application-owned wrapper HTML.

The package should expose composition slots rather than encode ToskLight window names or feature commands.

### 4. Grid desktop and pane surfaces

The desktop manager must preserve the existing 24-column by 18-row grid. Overlapping, free-positioned mock windows are not an acceptable substitute.

Split the current `components/shell/DeskGrid.tsx` and `components/shell/Pane.tsx` responsibilities as follows.

Package-owned primitives:

- grid constants or configurable grid dimensions defaulting to 24 by 18;
- grid-coordinate to CSS geometry conversion;
- pointer-coordinate to grid-cell conversion;
- pane frame and pane chrome;
- drag and resize interaction constrained to grid cells;
- active, inactive, editing, and maximized visual states;
- empty-desktop interaction surface; and
- collision and overlap policy as an explicit API rather than accidental CSS positioning.

Application-owned adapter:

- loading the current desk and pane models;
- dispatching open, move, resize, maximize, close, and settings actions;
- selecting a window from `WindowRegistry`;
- rendering `WindowPicker` and pane settings workflows;
- persistence and restoration; and
- multi-screen or native-window behavior.

Storybook must include a deterministic desktop with several windows placed on the real grid, plus drag, resize, maximize, empty-grid, and constrained-placement cases.

### 5. Tables and multi-step lists

Move the generic `DataTable` from `components/window-kit/WindowKit.tsx` with its existing active-row, selected-row, keyboard, empty-row, row-class, and data-attribute behavior.

Do not treat the generic table alone as proof of Fixture Sheet support. Extract or define a specialized reusable table view based on the existing `windows/FixtureSheetTable.tsx` so Storybook can demonstrate:

- sticky headers;
- active and selected rows;
- fixture and logical-head indentation;
- row state classes and step-selection state;
- keyboard navigation and activation;
- configurable columns and widths;
- expandable or subordinate rows where required by current workflows; and
- a stable empty-table geometry.

Also provide composable multi-column or multi-step list primitives for workflows that move from a left list through one or more dependent lists to final content. Feature-specific data loading and selection state remain in app adapters.

### 6. Faders and individual input surfaces

Extract reusable input surfaces independently so applications can compose them without importing the complete programmer or playback feature:

- horizontal fader controls;
- `VerticalTouchFaderSurface` and its local pointer interaction;
- direct-entry affordance and direct-entry modal composition;
- fader labels, current values, ranges, accent colors, disabled state, and auxiliary actions;
- value buttons and related touch targets; and
- explicit touch-expanded and hardware-reduced display variants where currently present.

The existing app-aware `VerticalTouchFader` wrapper should remain in the Control UI initially and provide the hardware mode from `useServer` and `useApp`. The package surface receives that mode explicitly.

A fader is an absolute control. It must not become the primary implementation of a software encoder. Relative software-encoder semantics remain governed by `docs/plans/Later/54-software-encoder-relative-controls.md`.

### 7. Encoders

Start with one reusable encoder, then build encoder sections by composition.

Directly extract `HardwareEncoderDisplay` where possible. Split parameter-controller dependencies out of current encoder-section components so the package receives an encoder view model containing:

- stable identifier;
- primary and optional secondary target;
- label, current value, mixed or released state, and range feedback;
- enabled and editable state;
- touch-expanded or hardware-reduced mode;
- coarse and fine relative-change callbacks;
- press or activation callback;
- range or spread callback; and
- explicit absolute-entry callback.

Build an encoder section from individual encoders. Its family groups and labels, including examples such as Intensity, Color, Position, Beam, and custom application-specific groups, must be data-driven rather than hard-coded into the package.

The app adapter retains parameter controllers, programmer mutations, selection projections, feature-family decisions, and server feedback.

### 8. Playbacks

Extract the exact rendering and local interaction portions of the current touch and hardware playback cards rather than designing generic replacements.

Package-owned playback surfaces should accept a compact view model for labels, cue state, level, timing or progress, color/state indication, button availability, and mode. They should emit fader, GO, stop, release, flash, selection, and configuration callbacks without invoking playback services directly.

The Control UI retains playback bank controllers, assignment and page addressing, runtime actions, source projections, and configuration workflows.

### 9. Pool and button grids

Move `ButtonGrid` and `GridButton` with the existing `ResizeObserver`-based scaling and stable row sizing.

Split group, preset, cuelist, and other pool buttons into reusable visual cards plus application adapters. The reusable surface must support current numbering, labels, secondary information, colors, empty, selected, active, disabled, and store-target states, as well as explicit click and press-and-hold callbacks where used.

Pool object retrieval, selection dispatch, programmer actions, and store behavior remain application-owned.

### 10. Modals and dialogs

Move existing modal primitives such as `ModalTitleBar`, `ModalPortal`, and reusable dialog frames. Preserve current title buttons, tabs, close controls, geometry, backdrop behavior, and nested-modal presentation.

The existing modal implementation relies partly on DOM conventions and independent portals. Introduce one package-owned modal provider and stack manager that formalizes current behavior:

- deterministic stack order;
- only the top eligible modal handles Escape;
- configurable Escape, backdrop, and explicit-close policies;
- focus capture and restoration;
- nested modal operation;
- modal-level title buttons, tabs, dropdowns, search, and custom toolbar content;
- programmatic close by modal identifier; and
- close requests triggered by callbacks from application or server adapters.

The manager must not subscribe to the ToskLight server itself. The app receives server events and calls the manager's typed close or update API.

Distinguish reusable frames by role rather than by appearance alone:

- a modal blocks or layers over an existing workflow;
- a dialog presents a focused confirmation, edit, setup, or special workflow; and
- both use the same stack and close-policy infrastructure where appropriate.

Storybook must include a window opening a modal, that modal opening another modal, and a third nested modal. It must also show every close policy and title-bar configuration.

### 11. Complete windows and dialogs

Complete ToskLight windows remain application compositions, but package Storybook stories may compose deterministic mock windows from package primitives. Initial representative stories should include the current Stage, Fixture Sheet, Group Pool, Preset Pool, Cuelist Pool, Cues, Virtual Playbacks, File Manager, Text Editor, DMX, Help, and Patch or Setup-style layouts.

These stories are visual and interaction fixtures. They must not import live ToskLight server hooks or duplicate full feature controllers.

## Styling extraction

Component extraction includes the exact styles required to reproduce the current application. Moving TSX without its cascade is incomplete.

1. Inventory selectors used by each extracted component across the current global stylesheets.
2. Move shared tokens, controls, window-kit, modal, table, grid, encoder, fader, and playback-surface rules into package-owned style entry points.
3. Preserve selector behavior, geometry, and cascade order during the move.
4. Have the Control UI import the package styles before app-specific workflow overrides.
5. Keep feature-only and window-only overrides in the application until their owning view is extracted.
6. Add visual regression coverage before consolidating or renaming classes.

CSS cleanup and visual redesign are separate work. The extraction should first produce equivalent rendering.

## Storybook structure

Storybook should present components from bottom-up composition in this order:

1. Controls and buttons
2. Forms and form layouts
3. Keyboard, numpad, and direct input
4. Tables and Fixture Sheet table behavior
5. Individual fader
6. Individual encoder
7. Encoder section and configurable families
8. Individual playback and playback bank
9. Pool grid and pool buttons
10. Window configuration
11. Modal configuration
12. Dialog configuration
13. Multi-step and multi-column layouts
14. Nested modal stack
15. Grid desktop manager
16. Complete mock windows

Every public component story must expose useful controls for its configuration and show source code in Storybook Docs. Stories must use deterministic view models and callbacks and must not depend on a live server.

## Implementation phases

### Phase 1: Characterize the existing UI

- Inventory the current component, style, test, and application dependencies.
- Add or retain focused behavior tests around components that will move.
- Capture representative application screenshots for visual comparison.
- Record current keyboard, touch, pointer, modal, scroll, grid, and hardware/software behavior before changing ownership.

### Phase 2: Replace parallel package implementations

- Retain useful workspace and Storybook configuration.
- Remove or supersede package components that were independently recreated.
- Move directly reusable components and tests into `packages/ui`.
- Leave temporary re-export modules at old app paths where needed.
- Move the exact styles required by those components.
- Build Storybook stories around the extracted components.

### Phase 3: Split coupled surfaces

- Extract pure views from the desktop, pane, Fixture Sheet, encoder, playback, pool, and modal implementations.
- Keep app adapters responsible for contexts, controllers, domain projection, and mutation.
- Define narrow package-owned view models instead of importing app API or feature types.
- Add the centralized modal stack manager while preserving characterized close behavior.

### Phase 4: Storybook validation and feedback

- Run package type checking and unit or component tests.
- Build Storybook.
- Smoke-test every important story for nonblank content and interaction.
- Compare Storybook rendering against the captured application surfaces.
- Review component APIs, geometry, density, labels, actions, modal behavior, table behavior, grid behavior, and mode boundaries with operator feedback.
- Keep this phase package-focused apart from minimal re-export adapters required to keep the application compiling.

### Phase 5: Worktree for full application adoption

Only after the extracted package and Storybook are accepted, create a sibling Git worktree on an explicit `codex/` branch for the complete Control UI integration.

In that worktree:

- replace temporary re-exports and remaining app-local presentation markup with package imports;
- add app adapters for live view models and callbacks;
- keep `WindowRegistry` and complete workflow composition app-owned;
- remove superseded duplicate component and CSS implementations; and
- verify real desktop, server, touch, keyboard, OSC, and attached-hardware behavior according to the affected acceptance contracts.

## Verification gates

Before Storybook review:

1. Package type checking passes.
2. Package unit and component tests pass.
3. Storybook builds without importing live app state.
4. Every public component has a representative story and source preview.
5. Important stories render nonblank content at desktop and touch-oriented viewport sizes.
6. Extracted components match current application screenshots within intentional nondeterministic regions.
7. Grid desktop stories use the real grid geometry and do not overlap panes unless the configured collision policy explicitly permits it.
8. Nested modal, Escape, backdrop, explicit close, and programmatic close behavior is tested.

Before removing app-local implementations:

1. The Control UI builds and its focused component tests pass through package imports.
2. Existing operator labels, geometry, window placement, scrolling, and hardware/software mode boundaries are unchanged.
3. Fixture Sheet keyboard and row-state behavior remains intact.
4. Fader, encoder, playback, and pool interactions still reach the same app controllers and mutations.
5. Real desktop smoke and focused Playwright coverage pass for changed workflows.

## Acceptance coverage

1. Forms and form fields have one authoritative implementation in `packages/ui` and cover all current Control UI variants.
2. Keyboard, numpad, and direct-entry controls work standalone and in nested modals.
3. Window chrome, sidebars, title controls, settings, search, bottom regions, and scrolling are package-owned and compose without extra layout HTML.
4. The reusable desktop surface uses the same 24 by 18 grid geometry and constrained pane behavior as the application.
5. The package table primitives can reproduce the actual Fixture Sheet presentation and interaction behavior.
6. A fader and an individual encoder are separately composable controls with explicit touch-expanded and hardware-reduced presentation where applicable.
7. Encoder family groups are configured by data rather than hard-coded ToskLight family names.
8. Touch and hardware playback cards use the extracted application rendering and emit typed callbacks.
9. Pool grids preserve their existing scaling and state presentation.
10. Modal and dialog stacks handle nested layers, focus, Escape, backdrop, explicit close, and app-triggered programmatic close deterministically.
11. Storybook renders the extracted production components with deterministic mock data and source preview.
12. The package has no dependency on Control UI contexts, controllers, server APIs, Tauri integration, or `WindowRegistry`.
13. The Control UI retains its current behavior while consuming the package through direct imports or temporary re-export adapters.
14. Future repository applications can use forms, inputs, windows, grid panes, modals, tables, faders, and encoders without depending on ToskLight runtime state.

## Deferred work

This plan does not redesign the visual system, change operator terminology, alter persisted desk or show formats, change backend APIs, implement relative programmer semantics, modify playback runtime behavior, or define new Tauri multi-window behavior.

Help-file PNG generation from accepted Storybook stories may build on this package, but the screenshot-generation pipeline should be specified and implemented separately after the stories are stable.
