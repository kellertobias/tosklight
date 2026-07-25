---
slug: ui-library
title: UI Library
summary: "App-local presentation primitives and their executable verification contract."
order: 20
---

# UI Library

There is no tracked shared UI package or Storybook configuration. The primitives live inside
`apps/light-desktop`.
`docs/plans/Next/58-shared-frontend-libraries.md` specifies the intended split into a
component/window-system library plus an app-layout library, but it is specification only.

Knowing that saves searching for `@tosklight/ui` source that does not exist. Ignored `dist/` and
`storybook-static/` files are generated remnants, not an ownership boundary or recoverable source.

## Where the primitives are

| Path | Contents |
| --- | --- |
| `apps/light-desktop/src/components/window-kit/` | `WindowKit.tsx` (window chrome and pane primitives), `SelectionList.tsx`, `index.ts` |
| `apps/light-desktop/src/components/common/` | `controls.tsx`, `FaderControls.tsx`, `ModalPortal.tsx`, `ModalTitleBar.tsx`, `SearchBar.tsx`, `TouchSelect.tsx`; barrel at `index.ts` |
| `apps/light-desktop/src/components/common/controls/` | `foundation.tsx`, `formFields.tsx`, `textInputs.tsx`, `choices.tsx`, `pickers.tsx`, `InputModal.tsx` |
| `apps/light-desktop/src/components/shell/` | App and desk layout — `AppShell`, `DeskGrid`, `Pane`, `PaneChromeContext`, `WorkspaceView` |
| `apps/light-desktop/src/components/shared/` | Semi-generic domain widgets — `SourceValue`, `SourceLegend`, `GroupStrip`, `FixtureColorDot`, `RecordModeDialog` |

`apps/light-hardware-controls/src/components/` has its own unshared `ControlButton.tsx` and
`TouchFader.tsx`. That duplication is one thing the extraction would resolve.

## Live catalog

```
?ui-kit=1
```

`components/window-kit/UiKitCatalog.tsx` renders every primitive on one page. Check it before
writing a new control.

## Visual system

`apps/light-desktop/src/styles/` holds eight CSS layers imported in cascade order by `src/styles.css`.
Order matters. Further global sheets sit at `apps/light-desktop/src/*.css`: `window-kit.css`,
`hardware.css`, `chrome.css`, `help.css`, `workflow-themes.css`, `playback-colors.css`,
`hardware-dense.css`, `fixture-address.css`, `cuelist-settings-layout.css`, `product-demo.css`.

## Shared code

One file: `packages/light-controls/src/programmerKeypad.ts` (71 lines) — the `SoftwareKey` union, the
`numericPadLayout` physical key layout, and `oscProgrammerActionForKey`.

Consumed via relative paths, with no package or alias, by the control UI keypad, the hardware
surface, and the Playwright bench. One keypad model, three consumers, which is what keeps the keypad
contract consistent across surfaces.

## Guidance

- Presentation primitives only. A component that knows about cues, revisions, or subscriptions is a
  feature.
- Extend an existing primitive rather than adding a near-duplicate.
- Keep touch targets desk-appropriate. Hover may reveal detail but must not be required.
- Preserve both software-only and hardware-connected layouts when changing a primitive.

## Executable contract

There is no standalone package gate. `npm run test:unit` runs the app-local component tests,
Control UI typecheck, and production build. `npm run test:e2e-ui` exercises the real browser and
operator layouts. Run both when changing shared presentation behavior.

A future extraction is one coherent change: add tracked package source and its consumers together,
then add package-specific tests or Storybook gates. Do not revive root scripts before that contract
exists.

## Read first

1. `src/components/window-kit/UiKitCatalog.tsx`, then open `?ui-kit=1`
2. `src/components/window-kit/WindowKit.tsx`
3. `src/components/common/controls/foundation.tsx`
4. `src/components/common/index.ts` — the barrel shows what is public
5. `src/styles.css` — cascade order
6. `packages/light-controls/src/programmerKeypad.ts`
