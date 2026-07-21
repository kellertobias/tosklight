---
slug: ui-library
title: UI Library
summary: "Presentation primitives and the visual system. Not yet an extracted package."
order: 20
---

# UI Library

There is no shared UI package yet. The primitives live inside `apps/control-ui`.
`docs/plans/Next/58-shared-frontend-libraries.md` specifies the intended split into a
component/window-system library plus an app-layout library, but it is specification only.

Knowing that saves searching for a `packages/ui` that does not exist.

## Where the primitives are

| Path | Contents |
| --- | --- |
| `apps/control-ui/src/components/window-kit/` | `WindowKit.tsx` (window chrome and pane primitives), `SelectionList.tsx`, `index.ts` |
| `apps/control-ui/src/components/common/` | `controls.tsx`, `FaderControls.tsx`, `ModalPortal.tsx`, `ModalTitleBar.tsx`, `SearchBar.tsx`, `TouchSelect.tsx`; barrel at `index.ts` |
| `apps/control-ui/src/components/common/controls/` | `foundation.tsx`, `formFields.tsx`, `textInputs.tsx`, `choices.tsx`, `pickers.tsx`, `InputModal.tsx` |
| `apps/control-ui/src/components/shell/` | App and desk layout — `AppShell`, `DeskGrid`, `Pane`, `PaneChromeContext`, `WorkspaceView` |
| `apps/control-ui/src/components/shared/` | Semi-generic domain widgets — `SourceValue`, `SourceLegend`, `GroupStrip`, `FixtureColorDot`, `RecordModeDialog` |

`apps/hardware-controls/src/components/` has its own unshared `ControlButton.tsx` and
`TouchFader.tsx`. That duplication is one thing the extraction would resolve.

## Live catalog

```
?ui-kit=1
```

`components/window-kit/UiKitCatalog.tsx` renders every primitive on one page. Check it before
writing a new control.

## Visual system

`apps/control-ui/src/styles/` holds eight CSS layers imported in cascade order by `src/styles.css`.
Order matters. Further global sheets sit at `apps/control-ui/src/*.css`: `window-kit.css`,
`hardware.css`, `chrome.css`, `help.css`, `workflow-themes.css`, `playback-colors.css`,
`hardware-dense.css`, `fixture-address.css`, `cuelist-settings-layout.css`, `product-demo.css`.

## Shared code

One file: `apps/shared/programmerKeypad.ts` (71 lines) — the `SoftwareKey` union, the
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

## Read first

1. `src/components/window-kit/UiKitCatalog.tsx`, then open `?ui-kit=1`
2. `src/components/window-kit/WindowKit.tsx`
3. `src/components/common/controls/foundation.tsx`
4. `src/components/common/index.ts` — the barrel shows what is public
5. `src/styles.css` — cascade order
6. `apps/shared/programmerKeypad.ts`
