---
slug: ui-library
title: UI Library
summary: "The @tosklight/ui presentation library, contained Storybook, and application-owned compositions."
order: 20
---

# UI Library

The tracked presentation library lives at `apps/ui-library` and keeps the workspace package
identity `@tosklight/ui`. Low-level reusable views, styles, stories, and component tests live there.
Functional ToskLight compositions remain in `apps/light-desktop`.

The contained Storybook application lives at `apps/ui-library/storybook`. Its configuration
discovers colocated stories from both the library and desktop application; it may provide
deterministic fixtures and provider harnesses, but it does not own parallel visual implementations.

## Where the primitives are

| Path | Contents |
| --- | --- |
| `apps/ui-library/src/` | `@tosklight/ui` source, public family entry points, styles, component tests, and low-level stories |
| `apps/ui-library/storybook/config/` | Storybook discovery, preview styling, manager theme, and global configuration |
| `apps/ui-library/storybook/tests/` | Deterministic real-browser story verification |
| `apps/light-desktop/src/components/` | Product shell, Dock, command controls, adapters, and functional compositions |
| `apps/light-desktop/src/windows/` | Complete ToskLight windows; application stories are colocated here as they are added |

Compatibility modules at former desktop component paths may re-export `@tosklight/ui`, but
production consumers should import the package identity rather than relative paths into the
library.

## Live catalog

Run `npm run storybook` from the repository root. The contained catalog serves on
`http://127.0.0.1:6006`; its discovery roots cover both focused package contracts and colocated
application-owned functional stories.

## Visual system

`apps/ui-library/src/styles.css` is the public shared style entry point.
`apps/light-desktop/src/styles.css` imports it before the application-owned layers. Storybook loads
the desktop entry point so the catalog uses the same cascade, background, fonts, density, and mode
styling as the live application.

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

`npm run test:ui-package` typechecks `@tosklight/ui` and runs its component suite.
`npm run storybook:build` creates the deterministic static artifact, and
`npm run test:storybook` exercises it in serial Chrome. `npm run test:unit` keeps the package and
application integration gates together; use `npm run test:e2e-ui` for real operator workflows that
Storybook cannot prove.

## Read first

1. `apps/ui-library/src/index.ts` and its family entry points
2. `apps/ui-library/src/window-kit/WindowKit.tsx`
3. `apps/ui-library/src/common/controls/foundation.tsx`
4. `apps/ui-library/src/styles.css`
5. `apps/ui-library/storybook/config/main.ts`
6. `apps/light-desktop/src/styles.css`
7. `packages/light-controls/src/programmerKeypad.ts`
