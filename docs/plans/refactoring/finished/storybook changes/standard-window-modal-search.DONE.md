# Standard Window and Modal Search

## Status

Completed on 2026-07-26. Search is now a first-class typed feature of package windows and modals
instead of an arbitrary caller-rendered title-bar slot.

## Operator contract

1. Windows and modals expose one standard search feature.
2. Search is visible when an `onSearch` callback is provided and absent when it is not provided.
3. Every window and modal search uses the same input, clear action, icon, touch keyboard behavior,
   dimensions, title-bar placement, focus behavior, and accessible labels.
4. When search settings are configured, the same settings affordance and settings dialog are used
   everywhere.
5. Search settings use typed configuration. Individual features must not recreate their own search
   icon, options button, filter-dialog chrome, spacing, or title-bar wrapper.
6. Search remains controlled by the feature owner. The package owns presentation and local
   open/close/focus behavior, not filtering, server queries, or persisted filter state.

## Public API

Replace the arbitrary `search?: ReactNode` title-bar contract with a typed feature contract. The
exact names may follow package conventions, but the behavior should be equivalent to:

```ts
type SearchSetting =
  | {
      kind: "select";
      id: string;
      label: string;
      value: string;
      options: readonly { value: string; label: string }[];
    }
  | {
      kind: "toggle";
      id: string;
      label: string;
      value: boolean;
    };

interface SearchFeatureConfiguration {
  value: string;
  placeholder?: string;
  ariaLabel?: string;
  settingsTitle?: string;
  settings?: readonly SearchSetting[];
  onSettingChange?(id: string, value: string | boolean): void;
  onClearSettings?(): void;
}

type SearchFeatureProps =
  | {
      onSearch?: undefined;
      search?: never;
    }
  | {
      onSearch(value: string): void;
      search: SearchFeatureConfiguration;
    };
```

Apply this contract to:

- `WindowHeader`;
- `WindowFrame`;
- `ModalTitleBar`; and
- `ModalFrame`.

The title-bar primitive renders the package `SearchBar` automatically when `onSearch` exists.
Callers provide data and callbacks, not a rendered `SearchBar`.

Default labels should derive consistently from the visible title when it is plain text:

- input: `Search <title>`;
- clear action: `Clear search`;
- settings action: `Search settings`; and
- settings dialog: `<title> search settings`.

Callers may override these strings only where the visible title cannot provide an accurate
accessible name.

## Standard search settings

- Search settings appear only when the typed `settings` list is non-empty.
- The settings trigger always occupies the same position inside the standard search control.
- The settings dialog uses the shared modal stack, `ModalTitleBar`, close policy, spacing, form
  controls, and focus restoration.
- Select settings use the package select control and always include explicit values and labels.
- Toggle settings use the package switch control.
- **Clear settings** resets configured settings through `onClearSettings`; it does not clear the
  search query.
- Closing or applying settings returns focus to the settings trigger.
- Escape closes only the top search-settings dialog and must not close its owning modal.
- Do not accept arbitrary caller-supplied search-settings chrome or raw title-bar children.

## Migration

- Replace `search={<SearchBar ... />}` and placeholder search nodes in package stories.
- Migrate the Cuelist Pool search, Add Fixture search, manufacturer lookup, Fixture Library search,
  Help search, File Manager search, and other existing window/modal searches to the typed contract.
- Preserve each feature's current query, placeholder, filters, empty-result messaging, and
  application-owned filtering behavior.
- Keep exceptional non-window/non-modal search surfaces explicit, but make them use the same
  `SearchBar` and typed settings model rather than duplicating presentation.
- Remove compatibility `search?: ReactNode` only after all application call sites have migrated.

## Storybook

Add dedicated stories showing:

- a window with no search callback and therefore no search control;
- a window with controlled search;
- a modal with controlled search;
- search with select and toggle settings;
- clear query versus clear settings;
- search settings opened from a modal to prove nested stack and Escape behavior; and
- narrow touch geometry.

All complete window and modal stories that support search must configure it through `onSearch` and
the typed search configuration rather than rendering `SearchBar` themselves.

## Verification

- Package test: omitting `onSearch` renders no search textbox.
- Package test: providing `onSearch` renders the standard search control and typing calls it with
  the complete current query.
- Package test: clear search calls `onSearch("")`.
- Package test: settings render from typed select and toggle configurations and call
  `onSettingChange` with the correct ID and value.
- Package test: Clear settings is distinct from clearing the query.
- Package test: settings close restores trigger focus.
- Package test: Escape closes nested search settings without closing the owning modal.
- Storybook test: window and modal searches use the same component classes and computed geometry.
- Storybook touch-viewport checks cover the standard search and settings dialog.
- Application tests preserve Cuelist, fixture, manufacturer, Help, and File Manager filtering and
  empty-result behavior.
- Architecture check rejects new `search={<SearchBar ... />}` window/modal call sites after
  migration.

## Result

The typed standard-search contract is implemented across `WindowHeader`, `WindowFrame`,
`ModalTitleBar`, and `ModalFrame`. Plain-text titles derive consistent search and settings names;
typed select, toggle, and text settings use shared package controls; clearing the query remains
distinct from clearing settings; and application-owned query/filter state remains outside the UI
package.

The follow-up modal-stack plan,
`modal-stack-search-settings-and-titlebar-divider.DONE.md`, supersedes the original settings
layer details and completes the shared stack, top-only Escape/backdrop behavior, focus restoration,
immediate-settings action removal, aligned settings form, and title-bar divider contract.

Application window and modal call sites use the typed `search` plus `onSearch` API. Two intentionally
exceptional chrome portals still render the same typed package `SearchBar` directly: the pane-hosted
File Manager toolbar and the Fixture Library setup action target. Neither duplicates search
presentation or filtering semantics, and no `search={<SearchBar ... />}` compatibility call site
remains.

Verification shared with the follow-up plan:

- the complete UI package unit suite passed: 18 files and 105 tests;
- UI and desktop package typechecks passed;
- the contained Storybook production build passed; and
- focused window/modal search, nested settings, clear-query/clear-settings, focus, stack, geometry,
  and responsive browser checks passed.

The root-owned full contained Storybook gate passed all 217 Playwright checks. Follow-up browser
review confirmed the shared window/modal search geometry, settings presentation, responsive
divider, and nested-modal behavior.
