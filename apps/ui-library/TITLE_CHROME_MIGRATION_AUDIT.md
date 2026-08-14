# Title chrome migration audit

This inventory is the TL-224 acceptance record. `tools/check-title-chrome-audit.mjs` scans every production TSX source under `apps/light-desktop/src` and `apps/ui-library/src`; tests and stories are covered separately by the component and Storybook suites.

## Shared contract

- `TitleChrome` is the only action/group/tab/dropdown/search renderer used by `WindowHeader` and `ModalTitleBar`.
- Window and modal callers use `groups` and nested `search.onSearch`; the former `actions`, modal `tabs`, split `onSearch`, `WindowAction`, `ModalTitleTab`, `TitleBarSearchDivider`, and `legacySearch` title APIs are absent.
- Every window chrome renders Settings as its far-right terminal control. It is disabled when the surface has no settings action.
- Every modal title renders Close as its far-right terminal control. Accept, when present, is rendered immediately to its left.
- Application-owned portal targets and non-action selectors may use `toolbar`; normal title buttons, tabs, dropdowns, and search do not.

## Window inventory

The machine audit accounts for every production `WindowHeader`, `WindowFrame`, `PaneView`, and `PoolWindow` occurrence. The application owners are:

- shell and setup: `components/shell/Pane.tsx`, `components/setup/fixturePatch/PatchChrome.tsx`, `windows/setupWindow/SetupChrome.tsx`;
- core windows: Channels, DMX, Dynamics, Fixture Sheet, Grid Dynamics, Groups, Help, Macros, Patch, Scheduler, Timecode, and Visualization;
- specialized windows: Cuelist Detail/Pool, Dynamic Editor, File Manager, Media Pane, Presets, Stage, Text Editor, and Running;
- shared owners: `desktop/PaneView.tsx`, `pools/PoolWindow.tsx`, and `window-kit/WindowKit.tsx`.

All are migrated through the shared renderer. Pane drag props remain on the outer `WindowHeader`; action controls stop pointer propagation through the existing Button path, so pane dragging and title actions retain separate interaction ownership.

## Modal and dialog inventory

The machine audit accounts for every production `ModalTitleBar` and `ModalFrame`, plus every source occurrence of `role="dialog"`, `aria-modal`, `nested-modal`, `modal-card`, and `file-confirmation`.

- control and programmer dialogs: Playback Configuration/Page, Sound to Light, Record/Preload, Command Choice, Debug, Desk Settings, Quick Setup, Selective Import, Show Recovery, Special Dialogs, Store Settings, System Controls, Window Picker, and Update workflow;
- setup dialogs: Desk Lock Settings, Output Routes, Playback Layout, fixture library revision/transfer, fixture address/browser/placement/edit/conflict surfaces, fixture-profile editor dialogs, screen picker/settings, and Record Mode;
- window-owned dialogs: Channel pages, File Manager picker/confirmations, Grid Dynamics, Scheduler, Virtual Playback exclusion zones, Cuelist properties/settings, Dynamics Editor, Preset configuration, and Timecode editor;
- shared UI dialogs: command history, SearchBar settings, InputModal/pickers, ModalNumberEditor, unsaved-input confirmation, and ModalStack/WindowSettings primitives.

All title-bearing entries are migrated to `ModalTitleBar` or `ModalFrame`. There are no approved compatibility exceptions.

## Intentionally titleless

- `apps/light-desktop/src/components/modals/DeskLockOverlay.tsx`: the full-screen desk lock is an application state surface, not a dismissible modal. Its “Desk locked” heading is page content; Escape, backdrop, and explicit close are deliberately forbidden. Unlock is the only terminal operation.

## Reproduction

```sh
node tools/check-title-chrome-audit.mjs
npm run typecheck --workspace @tosklight/ui
npm run typecheck --workspace @tosklight/light-desktop
npm test --workspace @tosklight/ui
npm run storybook:build --workspace @tosklight/ui
npm run test:storybook --workspace @tosklight/ui
```
