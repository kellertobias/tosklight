# DMX and Help Storybook Parity

## Status

Complete. Storybook now renders application-owned DMX and Help views extracted from the live
windows, with deterministic data boundaries and no duplicate complete-window presentation.

Use the application and its current deterministic help screenshots as the visual reference:

- `docs/help/assets/screenshots/panes/dmx.png`
- `docs/help/assets/screenshots/panes/help.png`

## Source-of-truth rule

1. The live `DmxWindow` and `HelpWindow` presentation are authoritative.
2. Storybook must load the same application-owned functional components and styles that the live
   application consumes.
3. Do not represent a live window with a generic table, `SelectionTree`, or fabricated article
   merely because those primitives are available.
4. Keep server polling, API loading, application state, output mutations, and URL policy in thin
   application adapters.
5. Use deterministic Storybook providers at the existing application boundaries.

## DMX parity

The current Storybook DMX story is wrong because it renders a short row table and universe
navigation. The application renders a diagnostic dot matrix and inspector.

The corrected application component story must reproduce:

- the **DMX Output** header;
- the **Values as dots** and **Sources** title actions;
- DMX Settings with Small/Large dot size;
- one section per visible logical universe;
- all channels 1–512 represented as the responsive dot matrix;
- row hexadecimal start addresses;
- live-value intensity classes and selected-channel state;
- the calculated values-per-row label;
- the fixed right-hand information pane;
- output summary when no channel is selected;
- selected channel address, hexadecimal address, and DIP-switch representation;
- fixture, patch-owner, patch-range, split, fixture-channel, attribute, and component information;
- the Raw value direct-entry surface; and
- **Release override**.

The Sources state must show active diagnostic overrides and their Release actions, including the
empty state.

Component/harness boundary:

- application component: dot-matrix geometry and rendering, universe sections, inspector
  presentation, DIP switches, summary, source list, selected state, and production callbacks;
- Storybook harness: deterministic DMX snapshots, output health, patched-fixture/profile data,
  output routes, desk settings, and observable override mutations.

Do not put a fixture-value `DataTable` or universe `SelectionTree` back into the DMX story.

## Help parity

The current Storybook Help story is wrong because it renders a generic `SelectionTree`, a
fabricated Command Line article, and a search control that the live window does not currently
contain.

The corrected application component story must reproduce:

- the simple **Help** header and optional **Live documentation** status;
- the fixed left navigation column;
- the actual hierarchical folder/topic rows;
- folder chevrons, indentation, expanded state, and selected-topic styling;
- the right scrollable documentation area;
- real Help typography for headings, paragraphs, links, lists, tables, code, desk keys, keyboard
  keys, placeholders, warnings, and screenshots;
- the application's touch scrollbar;
- loading, catalog-empty, topic-error, and catalog-warning states; and
- compact presentation without inventing different navigation semantics.

Use deterministic representative help content shaped like the real Quick Start/manual output,
including a screenshot and desk-key examples. Do not duplicate the full help API or Markdown
loading controller in the package.

Component/harness boundary:

- application component: Help layout, hierarchical navigation, selected/expanded presentation,
  content viewport, Markdown presentation, status/error states, and navigation behavior;
- Storybook harness: deterministic catalog/topic responses, live/error state, fixed asset URLs,
  and no network traffic.

Search may be added to Help only when the live Help window adopts the standard typed
window/modal-search feature. Storybook must not invent a Help-only search control ahead of the
application.

## Storybook states

Add deterministic stories for:

- DMX Values with no selected channel;
- DMX Values with a selected patched channel;
- DMX Sources with active overrides;
- DMX Sources empty;
- DMX Small and Large dot sizes;
- Help Quick Start with expanded navigation and representative screenshot;
- Help nested topic selection;
- Help loading;
- Help catalog/topic error and warning; and
- narrow/compact Help and DMX layouts where supported by the live view.

## Verification

- Component tests cover 512 DMX addresses per rendered universe and stable address selection.
- Component tests cover DIP switches, inspector fields, output summary, source release, and
  Small/Large geometry.
- Component tests cover Help tree expansion, selection, indentation, active row, loading, empty,
  error, warning, and content rendering slots.
- Storybook tests reject `.ui-data-table` and `SelectionTree` substitutes in the DMX story.
- Storybook tests reject the old fabricated Help `SelectionTree` composition.
- Storybook tests verify the expected DMX matrix, inspector, Help navigation, and Help content
  landmarks at desktop and touch-oriented viewports.
- Storybook stories make no REST or WebSocket requests.
- Focused live-app tests prove the adapters retain DMX polling/override behavior and Help
  catalog/topic loading.
- Compare the corrected stories visually with the two reference screenshots before requesting
  operator acceptance.

## Result

Completed on 2026-07-26.

- `DmxWindow` remains the live polling/provider adapter and now renders the exported
  `DmxWindowView`. The view owns the production dot matrix, responsive 512-address geometry,
  Values/Sources navigation, dot-size settings, selected-channel inspector, DIP switches, fixture
  metadata, raw-value entry, summary, and release callbacks.
- `HelpWindow` remains the live catalog/topic adapter and now renders the exported
  `HelpWindowView`. The view owns the production hierarchy, expansion and selection presentation,
  typed `WindowHeader` search, loading/empty/error/warning states, touch-scroll content area, and
  Markdown renderer. Its injectable URL transform lets Storybook bundle the representative
  Quick Start screenshot without making a help API request.
- Deterministic Storybook fixtures live under `apps/ui-library/storybook/fixtures`; application
  stories live beside the production windows under `apps/light-desktop/src/windows`.
- The fabricated DMX table/universe tree and fabricated Help tree/article/search stories were
  removed from `CompleteWindows.stories.tsx`.
- A shared application stylesheet entry now keeps the desktop and Storybook import order
  identical, including Help, window-kit, hardware, workflow, playback, fixture-address, and demo
  styles.
- Focused component coverage verifies both 512-address universes, responsive dot geometry,
  selection persistence, intensity classes, output summary, the complete patched inspector, DIP
  switches, source and selected override release, dot settings, Help navigation, nested filtering,
  typed search, empty results, Markdown key rendering, catalog/topic loading, and live refresh.
- Focused Storybook coverage verifies all 15 DMX/Help stories, production landmarks, absence of
  `DataTable`/`SelectionTree` substitutes, source mutation, bundled Help imagery, typed search, and
  loading/empty/error/warning states without REST or WebSocket traffic.

Verification:

- focused desktop Vitest: 5 files, 20 tests passed;
- desktop TypeScript typecheck: passed;
- UI-library TypeScript typecheck: passed;
- repository architecture checks: passed;
- contained Storybook production build: passed;
- focused DMX/Help Storybook Playwright: 17 tests passed.
