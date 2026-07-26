# Forms Controls and Selection Modals

## Status

Completed on 2026-07-26. The `Controls/Production controls / Forms` review corrections are
implemented in production components, covered in Storybook, and verified in the integrated gates.

## Ownership and source-of-truth boundary

- Improve the production components rendered by Storybook, not Storybook-only lookalikes.
- Keep reusable form presentation and interaction in `apps/ui-library`.
- Keep server access, Tauri integration, and the hosted ToskLight File Manager in
  `apps/light-desktop` adapters. The UI-library file control must expose typed callbacks and must
  not import application contexts, `FileManagerPickerHost`, REST, or WebSocket code.
- Use the existing `HorizontalFaderField`/`HorizontalFader` form presentation rather than exposing
  a raw native range input as a default control. The fader may retain its native input internally
  for accessible interaction.
- Generalize the grouped choice presentation currently demonstrated by
  `LayoutChoiceField` in
  `apps/light-desktop/src/components/control/PlaybackConfigurationModal.tsx`; do not duplicate
  that complete application modal in the package.
- Load icon metadata from the repository-owned catalog under `assets/icons`. Do not create a
  second hand-maintained icon inventory.

## Current evidence

- `FormsExample` in `apps/ui-library/src/common/Controls.stories.tsx` fixes Stage view to `"2d"`
  with a no-op change callback, uses one Notes line, and exposes raw file and range inputs.
- `LargeTextInput` in `apps/ui-library/src/common/controls/textInputs.tsx` focuses the textarea
  before every explicit scroll.
- Storybook imports the application's page-level overflow rules while
  `apps/ui-library/storybook/config/preview.css` does not establish a scrollable Forms canvas.
- The same preview stylesheet globally hides native carets for deterministic captures.
  `InputModal` currently depends on read-only native input/textarea selection, although the
  package already has a reusable `ModalCaretValue` presentation.
- Number-pad and text-keyboard geometry is defined in
  `apps/ui-library/src/input/ModalInputControls.tsx` and `apps/ui-library/src/styles/input.css`.
- `ColorPickerField` and `IconPickerField` are package-owned in
  `apps/ui-library/src/common/controls/pickers.tsx`, but some of their current presentation rules
  still live in desktop application CSS. Keep component-owned styles with the package when
  correcting them.

## Forms story corrections

### Stateful controls and scrollable demo

1. Make the **Stage view** `2D` / `3D` control stateful. Selecting either value changes the
   selected value and visible active state; it must not remain permanently on `2D`.
2. Replace the short **Notes** sample with stable representative text containing at least ten
   visible lines. The initial viewport must overflow so the scroll controls and native scrolling
   can be reviewed without first typing more content.
3. The Forms story itself must scroll with wheel, trackpad, touch drag, and the Storybook canvas
   scrollbar at the supported review viewports. Opening and closing a nested form modal must not
   permanently lock the Forms page's scrolling.
4. Pressing the Notes up/down controls must scroll the existing textarea without remounting it,
   clearing its selection, changing its value, or producing a focus/outline/background flash.
   Repeated presses during smooth scrolling must not make the field blink. Preserve the caret and
   selection range while scrolling.

### Fader instead of a range input

1. Remove **Range input** from the Forms story. A native `input[type="range"]` is not an accepted
   default ToskLight form control.
2. Add a labeled, controlled fader example using the shared `HorizontalFaderField` presentation and
   callback contract. It must show its value and be operable by the same pointer/touch/keyboard
   paths supported by the production fader.
3. If forms need a field wrapper around the fader, add a composable wrapper rather than another
   fader implementation.

## Number input modal

1. Increase every number-pad touch button, its row height, and the corresponding gaps by a
   consistent 20–30 percent. A target around 64 px from the current 52 px key size is acceptable.
   Widen the modal as needed; do not squeeze the enlarged keys back into the current width.
2. Keep the five-column keypad geometry and the tall **ENTER** key relationship intact.
3. Show a clearly visible insertion caret in the displayed number value. It must be visible when
   the modal opens, including at the start and end of the value.
4. The on-screen left/right buttons and physical Left/Right Arrow keys move that same caret by one
   insertion position. The next digit, decimal point, sign operation, or Backspace acts at the
   displayed position.
5. Pointer/touch placement in the value preview updates the same caret state. The native selection
   and the rendered caret must never disagree.
6. Cover an empty value with a visible placeholder rather than substituting a fake numeric value.
   The placeholder is not committed as input.

## Text and multiline input modals

### Caret, layout, and placeholder

1. Display the insertion caret in regular and multiline input modals. Left/Right Arrow controls,
   physical arrow keys, pointer/touch placement, insertion, and Backspace all operate at that
   visible position.
2. Do not rely on a read-only native field's platform-dependent caret rendering. Use one
   authoritative, accessible caret/value presentation and avoid showing a duplicate caret.
3. Make the regular text modal's **ENTER** button fill the complete height of its right-hand action
   rail. Multiline retains its distinct **New line** and title-bar **Done** actions.
4. Leave one normal key-width gap between the cursor-navigation pair and the **SPACE** key. The gap
   must remain visible at the supported touch viewport rather than being simulated with a tiny
   margin.
5. Render **ESC** with a visibly yellow/amber button surface, not merely yellow text or border.
   Preserve a contrast-safe label and clear active, focused, and pressed states.
6. When the draft is empty, show the calling input's placeholder in both the underlying form field
   and modal value preview. Clearing must reveal the placeholder immediately. Placeholder text is
   never part of the draft or submitted value.

### Shift behavior

Add a visible **SHIFT** key and support the same state from touch/click and a physical Shift key.
Use an explicit state machine so pointer and keyboard paths cannot drift:

- inactive: character keys enter their normal value;
- one-shot: tapping/clicking Shift arms uppercase for the next character, and entering that
  character returns Shift to inactive;
- cancel one-shot: tapping/clicking Shift again before entering a character returns it to
  inactive;
- locked: holding Shift latches uppercase and keeps it latched across multiple characters;
- unlock: activating Shift again while locked returns it to inactive.

The key needs distinct inactive, one-shot, and locked visuals. Shift changes character casing or
the keyboard-layout map's shifted symbol; it must not uppercase already-entered text. Escape,
Enter, Backspace, cursor movement, and Space do not consume a one-shot Shift state.

## Color field and palette

1. Restyle the closed color field so it visually matches one palette choice: the outer button
   retains its border and interaction chrome, while a smaller inset color surface has visible
   margin on every side.
2. Preserve a readable value/indicator without painting the entire outer button edge-to-edge in
   the chosen color.
3. Make every palette choice square. The square geometry must survive responsive wrapping and
   active/focus borders; do not derive height from a text line.
4. Retain keyboard navigation, selected state, custom hexadecimal entry, and contrast-safe focus
   indication.

## Icon selector

### Catalog and grouping

1. Remove the custom-icon text field and **Use custom icon** action. Arbitrary custom icons are not
   part of this selector.
2. Put a dropdown in the modal title bar for choosing the visible icon group.
3. Populate it from every current group in `assets/icons`:
   **Beam size**, **Fixture base**, **Fixture type**, **Flash**, **Functionality**, **Gobo**,
   **Laser shape**, **Misc**, **Position**, **Position beam**, and **Prism**.
4. Include all editable catalog icons in their group. Do not show generated `.expanded.svg`
   siblings as duplicate choices.
5. Preserve the icons already visible in the current selector. If an existing glyph has no
   repository-catalog equivalent, place it in an explicitly named built-in/general group rather
   than keeping a hidden parallel array.
6. Render actual icons with accessible names; filenames and internal paths are not the only
   operator-visible labels.

### Default group

Expose a default-group input on the reusable icon selector. A caller such as a gobo editor can
open directly on **Gobo**, while another caller can choose a different group. The default selects
the initially visible library group; it does not silently replace the current icon value. Define
and test a deterministic fallback when the requested group is absent.

The Forms story must demonstrate changing groups, selecting an icon from at least two groups, and
opening the selector with a non-general default group.

## Reusable grouped selection modal

Add a Forms example for the selection pattern currently used when configuring the top, middle, or
bottom playback button:

- options are divided into named groups;
- every option has a label and may have a description;
- the current option is visibly selected;
- selecting an option returns its typed value and closes the modal;
- the trigger shows the selected option;
- the modal accepts optional clear text;
- when clear text is supplied, a title-bar clear action uses that exact text, such as
  **Empty Button**;
- when clear text is absent, no clear action or empty title-bar space is rendered; and
- the clear callback/value is explicit rather than inferred from an empty string.

Create this as a package-owned selection-modal primitive with caller-supplied groups, options,
descriptions, selected value, and optional clear action. Adapt the existing playback-layout
selector to the primitive where practical so the Storybook example and production workflow prove
the same component. Playback action grouping and application-owned value rules remain in
`apps/light-desktop`.

The Forms story needs examples both with **Empty Button** and with no clear action.

## File drop field

Replace the native **File input** row with a production file-drop field:

1. The complete field/drop surface is a usable touch target and clearly invites either dropping a
   file or opening the ToskLight File Manager.
2. Clicking/tapping its browse action opens the built-in root-confined File Manager, not the
   browser or operating-system picker by default.
3. The reusable control accepts constraints such as allowed extensions, MIME types, and single or
   multiple selection, and exposes callbacks for dropped files and opening the application picker.
4. Dragging an accepted file over the surface produces a clear accepted/ready state.
5. Dragging a rejected file, mixed accepted/rejected selection, too many files, or an unsupported
   item produces a distinct rejected state and does not permit the drop.
6. Dropping an accepted file calls the consumer exactly once and shows selected/loading/success or
   actionable error feedback as appropriate.
7. Dropping a rejected file changes no selected value and reports why it was rejected.
8. `dragenter`/`dragleave` handling must tolerate child elements so the hover state does not flicker
   while the pointer crosses text or icons inside the drop surface.
9. Keyboard activation and screen-reader labels must expose both drop constraints and the browse
   action.

The package component owns presentation, drag validation, and callbacks. The desktop adapter uses
the existing root-confined picker flow (`RootConfinedFilePickerButton` /
`openFileManagerPicker`). The Storybook harness must provide deterministic accepted, rejected,
loading, success, error, and hosted-picker states without contacting a live server.

## Storybook states and interaction coverage

Extend the focused Storybook checks under `apps/ui-library/storybook/tests` and package unit tests
for at least:

- switching the Forms `2D` / `3D` value;
- page wheel/touch scrolling and Notes overflow;
- Notes scroll buttons preserving focus, value, selection, and a stable rendered field;
- enlarged number-pad computed geometry at desktop and touch viewports;
- visible number/text/multiline carets at start, middle, end, and empty values;
- insertion and deletion after both on-screen and physical cursor navigation;
- placeholder restoration after clearing;
- regular Enter full-height geometry, cursor-to-Space gap, and amber Escape states;
- Shift one-shot, cancellation, hold-to-lock, locked typing, and unlock behavior;
- inset color trigger and square palette choices;
- icon group discovery, every catalog icon exactly once, default group, and absence of custom-icon
  entry;
- grouped selection with descriptions, selection, configurable **Empty Button**, and no-clear
  configuration;
- fader interaction with no native range control in the Forms story;
- accepted and rejected file drag hover/drop, nested drag targets, single callback delivery, and
  deterministic built-in File Manager opening; and
- absence of REST and WebSocket traffic from UI-library-only stories.

Use role/state assertions for behavior and focused computed-geometry assertions for the literal
sizing, square, gap, and full-height requirements. Add screenshots only for states where visual
review is materially useful; interaction tests remain the behavior contract.

## Acceptance and verification

- The Forms story scrolls and every example is interactive at the supported desktop and touch
  review viewports.
- No standalone native range or visible native file input remains as the Forms example; a shared
  fader may use a native range internally as its accessible interaction element.
- Input modal behavior is identical for touch/click and regular keyboard input, with one visible
  authoritative caret.
- Icon groups come from the repository catalog and callers can choose the initial group.
- Grouped selection is reusable and the existing playback-button workflow retains its labels,
  grouping, descriptions, and **Empty Button** behavior.
- The file field opens the real built-in File Manager through an application adapter and accepts
  valid drops without granting the UI package application access.
- Focused package tests, Storybook interaction tests, package typecheck, and the contained
  Storybook production build pass.
- A final operator review confirms the number-pad size, keyboard geometry, color presentation,
  square palette choices, icon grouping, grouped selector, fader, and file-drop feedback before
  live-application adoption.

## Non-goals

- Do not redesign unrelated buttons; the reviewed button examples are accepted.
- Do not add an operating-system picker as the primary file workflow.
- Do not add arbitrary uploaded/custom icons.
- Do not change playback action semantics or regroup them in application code except as required
  to feed the shared presentation primitive.
- Do not replace the existing production fader, File Manager, or icon assets with Storybook mocks.

## Result

Implemented the Forms, modal-input, color, icon-catalog, grouped-selection, fader, and file-drop
work described above:

- The Forms story is stateful and scrollable, uses the production fader, preserves the 12-line
  Notes field and its selection while scrolling, and replaces the visible native file input with
  the shared file-drop field.
- Number, regular-keyboard, and multiline modal inputs now render one authoritative caret and
  support pointer placement, placeholders, physical/on-screen cursor editing, one-shot and locked
  Shift, the enlarged number pad, the full-height regular Enter key, the cursor-to-Space gap, and
  amber Escape states.
- Color and icon pickers use the reviewed inset/square presentation and repository icon catalog.
  Expanded SVG variants are excluded from the displayed catalog, callers can choose a starting
  group, and no custom-icon entry is exposed.
- A reusable grouped-selection field now backs the playback layout chooser while preserving its
  groups, descriptions, labels, and explicit Empty Button behavior.
- A reusable file-drop field owns validation, nested drag state, single callback delivery,
  accessibility, and deterministic feedback. A desktop adapter composes it with the existing
  root-confined File Manager picker.

Verification completed:

- `npm run typecheck --workspace @tosklight/ui` passed.
- `npm run typecheck --workspace @tosklight/light-desktop` passed.
- `npm test --workspace @tosklight/ui -- --run src/input/ModalInputControls.test.tsx src/common/controls.test.tsx`
  passed: 34 tests.
- `npm test --workspace @tosklight/light-desktop -- --run src/components/control/PlaybackConfigurationModal.test.tsx src/components/files/RootConfinedFilePickerButton.test.tsx`
  passed: 22 tests.
- The focused Storybook interaction run for the Forms and input-modal scenarios passed: 2 tests.
- The contained Storybook production build passed.

The integrated Storybook gate subsequently passed all 217 Playwright checks, including the
reviewed screenshot contract and playback-bank geometry. Browser review confirmed the regular,
number, and multiline keyboards, cursor buttons, conditional number-pad keys, title-bar Done
action, multiline scroll controls, and playback-style color field at the supplied desk scale.

The final O2/UI-refactoring audit consolidated every direct number editor onto
`ModalNumberEditor`. Number values now keep both cursor arrows in the value row on the right,
while the keypad retains fixed empty grid cells when decimal or THRU is disabled. The same shared
surface is used by touch and hardware encoders, vertical faders, dual encoders, and the hardware
fade controls. The regular select exposes its listbox and active option through ARIA, and stale
desktop form, input, fader, modal, table, pool, and window-kit base rules were removed so the
package remains the presentation owner.

Post-audit verification passed 108 UI-package tests, 1,993 desktop tests, both TypeScript gates,
the architecture/CSS-ownership ratchet, 219 Storybook behavior/catalog checks, the separately
reviewed screenshot check, the 47-entry non-mutating screenshot manifest, the 27-image live-app
capture, manual generation, and the packaged `npm run open` readiness path.
