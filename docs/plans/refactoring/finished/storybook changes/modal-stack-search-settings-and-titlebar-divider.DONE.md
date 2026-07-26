# Modal Stack, Search Settings, and Title-bar Divider

## Status

Completed on 2026-07-26 as the follow-up to
[`standard-window-modal-search.DONE.md`](standard-window-modal-search.DONE.md). The production
modal stack, search settings, title-bar divider, and Storybook coverage pass the integrated gates.

## Goal

Guarantee that every newly opened modal is above every modal already open, regardless of which
component created it. Correct the configured-title-bar story so:

- the search input's touch-keyboard modal opens above the configured modal;
- search settings retain correct top-layer behavior;
- search-settings fields use standard spacing;
- switch text is beside the switch;
- redundant Apply/Save actions are removed; and
- search is visually separated from every adjacent button group by the standard divider.

## Current evidence

- `ModalLayer` uses the package modal stack with z-index
  `calc(3000 + var(--modal-stack-index))`.
- `InputModal`, opened by the search `TextInput` keyboard button, still uses
  `.stacked-modal-layer.ui-input-modal-layer` at z-index `2300`; it therefore renders behind the
  configured parent modal.
- Search settings are independently hard-coded to z-index `4000`, so they appear above the parent
  without participating in the actual modal stack.
- Search settings attach a broad `.search-filter-modal label` rule to every nested label. That
  interferes with package form/switch geometry.
- Search settings call `onSettingChange` immediately but still show an **Apply** button that only
  closes the dialog.
- `TitleBarConfiguration` separately renders both **Apply** and **Save** for the same generic
  example.
- Title bars do not own one consistent divider between search and neighboring buttons; some
  application CSS uses border-image treatments on unrelated action/close boundaries.

## One authoritative modal stack

### Opening order

1. Every modal opened after another modal receives the next stack position and renders above all
   existing modal layers.
2. Opening order, not modal type or hard-coded component z-index, determines visual order.
3. Apply the rule to:
   - `ModalFrame`/`ModalLayer`;
   - text, multiline, and number input modals;
   - search settings;
   - picker dialogs;
   - grouped selection dialogs;
   - nested confirmation/error dialogs; and
   - application-owned modals launched from an existing modal.
4. A child modal must not render inside or underneath its parent's card, overflow clipping, or
   backdrop.
5. Portals remain allowed, but every modal portal must register with the same active
   `ModalProvider` stack.
6. Remove fixed modal-type z-index tiers such as `2300` and `4000` once their components
   participate in the stack. Keep only one application-level base and the registered stack index.

### Top-layer behavior

Only the top modal may:

- receive backdrop dismissal;
- handle Escape when its policy allows it;
- receive pointer interaction outside explicit non-modal popovers;
- claim initial modal focus; and
- expose itself as the active `aria-modal` surface.

Closing the top modal reveals the previous modal without changing its draft state and restores
focus to the exact control that opened the child. A lower modal must not close, submit, or process
Escape while a child is open.

### Dynamic and mixed modal sources

1. Registration must work when a child is opened after the parent has already rendered.
2. Closing and reopening a modal moves the new instance to the top.
3. Stable IDs support programmatic close but do not preserve stale stack order across unmount.
4. Two independently portaled children cannot share the same stack index.
5. Legacy application modals used inside a package `ModalProvider` must be adapted rather than
   placed in a competing z-index namespace.
6. If a modal is opened without a provider at an application root, fail clearly in development or
   route it through the root provider; do not silently fall back to a lower fixed z-index.

## Configured title-bar story

In `Modals/Modal stack / Title bar configuration`:

1. Clicking the keyboard action in the title-bar search opens the search input modal above
   **Patch fixtures**.
2. The parent title bar, tabs, search, actions, and body remain visible beneath the new backdrop
   but cannot receive interaction.
3. Closing the input modal returns focus to the search field/keyboard trigger.
4. Opening Search settings also creates the next stack layer rather than relying on z-index 4000.
5. Opening an additional child from Search settings, such as a select/input modal where
   applicable, places that child above Search settings.
6. Repeated open/close sequences must not invert the stack or reuse a stale layer index.

## Search-settings form

1. Compose search settings with the shared `FormLayout`, `FormField`, `SelectField`, and
   `SwitchField` geometry.
2. Remove broad descendant-label rules from `.search-filter-modal`. Component-owned form controls
   retain their own internal label layout.
3. Use one consistent vertical field gap and modal-body padding matching other standard settings
   modals.
4. Place the switch's setting text, such as **Favorites only**, beside the actual switch control.
5. Keep the switch and its On/Off feedback aligned on the same row as the label.
6. Do not place the setting label underneath the switch at supported desktop or touch widths.
7. Select labels, select controls, switch labels, and switches share one deliberate label column
   and one control column.
8. Descriptions and validation messages align with the control column and do not collapse the
   row spacing.
9. Search settings remain usable with one setting, mixed select/toggle settings, long labels, and
   the narrow supported modal viewport.

## One commit action

Do not present both Apply and Save for one draft or one immediate-settings surface.

### Search settings

The current settings callbacks apply immediately. Therefore:

- remove **Apply**;
- keep the title-bar close action;
- keep **Clear settings** only when `onClearSettings` exists;
- closing does not perform a second commit; and
- focus returns to Search settings after nested controls and to the settings trigger after the
  dialog closes.

If a future caller requires staged settings, it may supply one explicit commit action—either
**Apply** or **Save**, named for its actual persistence behavior—but never both for the same
draft.

### Title-bar configuration story

Replace the example's adjacent **Apply** and **Save** actions with one meaningful primary action.
The story may use **Save** for a persisted draft or **Apply** for a non-persisted operation, but it
must not imply two successive commits are required.

This does not remove legitimate Apply and Save actions from unrelated workflows where they act on
different scopes; the invariant is one commit action per draft/scope.

## Search divider

Whenever a search control is immediately adjacent to any button, tab group, action group, or
title-bar close button, insert the standard search divider between them.

### Visual contract

- The divider is a slightly wider neutral gray bar, not a hairline border.
- A centered one-device-pixel vertical line uses a cyan-tinted near-white color at partial
  opacity.
- The gray bar occupies the title-bar height and visually separates the complete interaction
  groups.
- The cyan-white line is centered inside the gray bar and does not touch button content.
- The divider has no hover, pressed, or focus state and is not clickable.
- At high device-pixel ratios, preserve a crisp one-device-pixel center line.

A target structure is a 7–9 CSS-pixel gray separator containing a centered 1-device-pixel
semi-opaque cyan-white line. Final width/opacity remain subject to operator visual review.

### Structural rules

1. Add the divider through a shared title-bar primitive or explicit separator element, not a
   caller-specific margin or border-image.
2. Insert it on the left of search when the preceding adjacent element is a tab/button group.
3. Insert it on the right of search when the following adjacent element is an action or close
   button.
4. Do not render duplicate dividers when actions are grouped.
5. Do not render an empty divider when search is alone beside passive title/spacer content.
6. Apply the same rule to modal title bars and standard window title bars.
7. At narrow responsive layouts where search moves to its own row, omit dividers for controls that
   are no longer physically adjacent.
8. Mark the separator `role="separator"` only if it communicates useful grouping to assistive
   technology; otherwise keep it decorative and `aria-hidden`.

Remove competing title-bar border-image or ad hoc search/action divider rules along migrated
surfaces.

## Storybook states

Add focused stories for:

- configured modal with search and one Save action;
- search keyboard modal above the configured modal;
- Search settings above the configured modal;
- a third child modal above Search settings;
- search settings with select and switch fields;
- immediate settings with Clear settings and no Apply;
- search adjacent to tabs on the left and an action/close button on the right;
- search with no adjacent buttons and therefore no divider;
- window title-bar search using the same divider;
- narrow layout where search moves to its own row; and
- three sequential open/close/reopen cycles.

## Verification

- Package stack test: each newly registered modal has a strictly higher effective layer than all
  currently open modals.
- Package stack test: the search input modal opened from Title bar configuration renders above its
  owning modal.
- Package stack test: Search settings and a child opened from it follow registration order without
  fixed z-index overrides.
- Escape test: each Escape closes only the top eligible layer in order.
- Backdrop test: pointer events on a lower backdrop do nothing while a child is open.
- Focus test: closing each child restores focus through the complete opener chain.
- Form geometry test: search-setting labels and controls use aligned columns and standard gaps.
- Switch geometry test: **Favorites only**, switch track, and On/Off feedback share one row and
  have non-overlapping bounding boxes.
- Action test: immediate Search settings have no Apply/Save button and Clear settings remains
  conditional.
- Story test: Title bar configuration contains exactly one primary commit action.
- Divider geometry test: every physically adjacent search/button boundary has exactly one wide
  gray divider with a centered crisp cyan-white line.
- Responsive test: no orphan/duplicate divider remains after search moves to another row.
- Application integration tests cover input modals, search settings, nested pickers, and
  confirmation dialogs launched from existing modals.
- The contained Storybook build and focused browser checks pass without network traffic.

## Acceptance

- Every new modal opens above all existing modals.
- Search input no longer opens behind Title bar configuration.
- Search settings remain above the parent through the real stack, not z-index 4000.
- Search-settings spacing matches standard forms.
- Toggle text is beside the toggle.
- Search settings do not show redundant Apply.
- Title bar configuration does not show both Apply and Save.
- Every adjacent search/button group is separated by the wide gray divider with its centered
  translucent cyan-white pixel line.

## Non-goals

- Do not change search filtering semantics or persisted filter ownership.
- Do not remove Clear settings.
- Do not add Apply and Save as aliases for the same mutation.
- Do not redesign unrelated modal bodies.
- Do not encode application services or persistence into the package modal stack.

## Result

Implemented the shared modal, search-settings, and title-bar corrections:

- `ModalProvider` now assigns monotonic registration order and makes only the top layer interactive,
  Escape/backdrop eligible, initially focusable, and `aria-modal`. Closing a child restores focus
  after the parent layer becomes active again; reopening receives a fresh top position.
- Input modals, search settings, icon and grouped-selection dialogs, encoder/fader direct-value
  dialogs, and modal window settings all use `ModalLayer`. The desktop and Storybook roots own the
  provider. Anchored select/color popovers remain non-modal but portal into the active modal
  stacking context.
- Search settings use `FormLayout` with aligned side labels and shared select, switch, and text
  fields. Legacy fixed z-index and broad descendant-label rules were removed. Immediate settings
  have no Apply/Save action, while conditional Clear settings remains.
- The configured title-bar story now exposes one Save action. Dedicated configured keyboard,
  settings, third-child, search-only, window-search, and reopen interaction states cover the stack.
- Modal and standard window title bars use the shared 8-pixel gray search divider with a centered
  translucent cyan-white device-pixel line. Responsive layouts hide the divider when search moves
  to its own row, and migrated application chrome no longer adds competing border-image dividers.
- Desktop tests that render package modal sources now use the required root provider instead of
  weakening the production provider contract.

Verification completed:

- `npm test --workspace @tosklight/ui` passed: 18 files and 105 tests.
- The affected desktop modal integration set passed: 10 files and 146 tests across the combined
  targeted runs.
- `npm run typecheck --workspace @tosklight/ui` passed.
- `npm run typecheck --workspace @tosklight/light-desktop` passed.
- `npm run storybook:build --workspace @tosklight/ui` passed.
- Focused Storybook modal/search/title-bar browser verification passed: 4 tests.
- `git diff --check` passed for the affected package, desktop, and plan paths.

The root-owned contained Storybook gate passed all 217 Playwright checks. Browser review confirmed
the nested stack, search settings, responsive title-bar divider, focus-safe modal transitions, and
the multiline input's title-bar Done action.
