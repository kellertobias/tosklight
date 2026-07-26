# Playback Bank Touch and Hardware Layout

## Status

Completed on 2026-07-26. This plan covers `Playbacks/Playback bank`; the separate
[`virtual-playbacks-pool-grid-layout.DONE.md`](virtual-playbacks-pool-grid-layout.DONE.md) covers
the Virtual Playbacks pool grid.

## Goal

Make the touch and hardware playback-bank cards explain themselves without unexplained amber
marks, invalid demo states, clipped titles, or overlapping fader text. Restore the accepted
color-gradient fader treatment and allocate the limited hardware width to the information and
actions the operator actually reads.

Keep the distinction between:

- the playback name;
- the page/slot address;
- the active cue name;
- the fader level and mode;
- the configured playback-button actions; and
- real runtime states such as running, loaded, pickup required, Flash held, and Swap held.

Do not communicate those different meanings with anonymous yellow shapes.

## Current evidence

- `PlaybackCards.stories.tsx` assigns slot 2 the `loaded` class, which desktop CSS renders as an
  amber dashed border without a textual explanation.
- The same story assigns `pickup-required` to slot 3, **Bump**, even though that playback has no
  fader. Pickup is impossible and meaningless on that card.
- A faderless playback with one action is collapsed into one large button containing both
  `3 · Bump` and `FLASH`, so the playback identity and configured action read as one unexplained
  phrase.
- `VerticalTouchFaderSurface` owns the accepted level fill in
  `apps/ui-library/src/styles/operator-surfaces.css`, but playback/application style layering can
  flatten or obscure that gradient.
- The touch story uses `Cue 4 · Solo` as fader-mode feedback while the percentage is placed in the
  same shrinking fader surface. At constrained card heights these texts can collide.
- Hardware headers place the full playback title and `2.1`, `2.2`, or `2.3` in one grid row.
- Hardware cue rows reserve separate columns for a marker, cue number, cue name, and trailing
  status. The name therefore receives too little of the available width.
- Hardware controls reserve a fixed 66 px fader column after the action-button area.
- `HardwareCueRowsView` renders an empty `<i>` marker, and legacy application playback selectors
  also target broad descendant `<i>` elements. These can create decorative colored marks without
  a defined operator meaning.

## Source-of-truth boundary

- Correct `TouchPlaybackCardView`, `HardwarePlaybackCardView`, `HardwareCueRowsView`,
  `VerticalTouchFaderSurface`, and their package-owned styles in `apps/ui-library`.
- Keep playback runtime projection, held-action behavior, pickup rules, and application callbacks
  in `apps/light-desktop`.
- The Storybook bank must use valid deterministic view models that could be produced by the live
  application. Do not fabricate impossible state combinations to make the story look busy.
- Bring any broad legacy selectors touched by this work under component-specific classes. A
  package-owned playback component must not depend on an unscoped application selector to paint
  its internal markers or fader.

## Status indicators

### Remove unexplained yellow marks

1. Remove the invalid `pickup-required` state from the faderless **Bump / FLASH** example.
2. Pickup-required styling may appear only on an assigned playback with a physical or touch fader
   that genuinely needs pickup.
3. Remove empty decorative marker elements that have no defined semantic state.
4. Do not use a yellow/amber dash, bar, dot, wedge, border fragment, or glow without an adjacent
   or otherwise unambiguous operator-readable state.
5. Scope cue-row, loaded, pickup, selected, held, and fader-fill styles so one state's marker
   cannot accidentally paint another element.

### Explain real states

If the bank story demonstrates a loaded cue, pickup requirement, held Flash, or held Swap:

- show **LOADED** in the summary detail position normally used for fade time;
- show pickup direction, physical level, and target directly on the hardware fader;
- show held Flash or Swap by changing the configured button itself, without a separate
  **FLASH HELD** or **SWAP HELD** label;
- use color as supporting feedback rather than the only explanation;
- keep the playback's configured color distinct from amber warning/state color;
- ensure the state remains readable in grayscale and at the supported touch viewport; and
- remove the indicator immediately when the mock state is cleared.

The two bank stories keep ordinary playbacks readable while placing representative loaded, held,
and hardware-pickup states directly in the 8-by-2 examples. The configurable story exposes those
states as controls, so separate one-off state stories are unnecessary.

## Touch bank

### Playback identity and action

1. Keep the playback identity separate from its configured action.
2. A faderless single-action playback named **Bump** must clearly read as:
   - playback/title: **3 · Bump**; and
   - action: **FLASH**.
3. Do not merge those strings into an unexplained `Bump / FLASH` composition or make the title
   itself appear to be the held action.
4. The FLASH surface remains a press/hold/release control and must preserve pointer capture,
   cancellation, and held feedback.
5. If a status label says **FLASH**, distinguish current held state from the static configured
   action label. Static **FLASH** means what the button does; active feedback means it is currently
   held.

### Restore the fader gradient

1. Restore the clearly visible level-dependent gradient used by the accepted vertical touch
   fader.
2. The filled portion should transition from the darker mixed playback color to the brighter
   playback color and stop at the current level.
3. Preserve a darker unfilled region above the level so the fader position is immediately
   readable.
4. The gradient must use the playback's configured accent color, not a hard-coded green or amber
   fill.
5. Playback-specific CSS must not replace the gradient with a flat block, hide it behind another
   background, or stretch it over the complete fader regardless of level.
6. Verify zero, low, mid, high, and full values and at least three playback colors.

### Prevent fader-text overlap

Give the fader label/mode and numeric value separate reserved areas:

- the label and mode, such as **Master** and **Cue 4 · Solo**, stay together at the top;
- the percentage remains independently readable and never paints over `Cue 4 · Solo`;
- long mode text truncates or wraps according to an explicit one/two-line rule before it reaches
  the percentage;
- the percentage remains inside the fader but has a stable anchored position and contrast-safe
  text shadow/background treatment; and
- at the minimum supported card height, both mode and percentage remain readable without overlap.

Do not solve the collision by removing the mode, current cue, or percentage.

## Hardware bank

### Header allocation

1. Place the compact page/slot address (`2.1`, `2.2`, `2.3`) at the top right of the card header.
2. Give the playback title the remaining header width instead of reserving an oversized address
   column.
3. Keep the address on one line with tabular numerals and a subdued visual weight.
4. Let the title use the maximum safe width up to the address, with deterministic ellipsis only
   after that space has been used.
5. The address must not push the title into a narrow column or appear detached from the card it
   identifies.

### Cue-name space

1. Give **Look**, **Solo**, **Blackout**, and longer production cue names substantially more
   horizontal space.
2. Compress or remove decorative marker width before truncating the cue name.
3. Keep the cue number compact and place trailing fade/status text only when it communicates a
   real state.
4. Prefer the cue name over an empty status column. An absent fade time or status must consume no
   width.
5. Preserve clear previous/current/next hierarchy without relying on anonymous yellow markers.
6. The current cue may retain a progress treatment, but the fill must sit behind the row content
   and never resemble a separate control or obscure the text.

### Playback-action space

1. Give configured actions such as **FLASH**, **OFF**, **GO +**, **GO −**, **SWAP**, and longer
   action names more horizontal space.
2. Reduce fixed, unused, or decorative space before reducing action-label width.
3. Rebalance the hardware action area and compact fader column so the action buttons remain
   touch-readable and their text does not collide or clip.
4. Buttons must continue to map to their configured top/middle/bottom hardware positions and
   preserve press-versus-held behavior.
5. Do not silently abbreviate action names unless that exact abbreviation is already part of the
   operator contract.

## Storybook states

Keep `Playbacks/Playback bank` intentionally limited to three stories:

- **Configurable Playback**, whose Storybook Controls select touch or hardware mode, family,
  title, page and slot, summary text and progress, one/two/three buttons and their labels,
  active-button feedback, fader presence and level, empty and selected state, and hardware
  pickup physical/target positions;
- **Eight By Two Touch Bank**, with one-button faderless playbacks above matching
  three-button-plus-fader playbacks; and
- **Eight By Two Hardware Bank**, with the same objects and control shapes plus one pickup from
  above and one pickup from below.

The two banks carry representative loaded, selected, active-button, family, and empty examples.
Touch never renders pickup feedback.

## Verification

- Package test: a faderless playback can never render pickup-required feedback.
- Package test: the Bump title and FLASH action are separate accessible elements, and held Flash
  remains a press/release action.
- Package style test: touch fader computed styles retain a level-clipped color gradient for zero,
  mid, and full values.
- Storybook geometry test: `Cue 4 · Solo` and the percentage have disjoint bounding boxes at every
  supported touch-bank height.
- Storybook geometry test: the hardware page/slot address is at the top right and the title uses
  all remaining header width.
- Storybook geometry test: cue-name cells receive more width than marker, cue-number, and optional
  status cells, and **Look**, **Solo**, and **Blackout** render without collision.
- Storybook geometry test: configured hardware action labels fit in their buttons at the minimum
  supported width.
- Storybook state test: every amber status treatment has an explicit accessible state label.
- Storybook state test: no empty marker or broad legacy selector creates stray yellow bars,
  wedges, dots, or border fragments.
- Interaction test: touch and hardware faders, GO/OFF actions, and Flash/Swap press/release
  callbacks retain their current behavior.
- Focused desktop adapter tests confirm loaded, pickup, held, selected, and running state mapping
  without changing playback runtime semantics.
- The contained Storybook build and focused playback-bank interaction suite pass without REST or
  WebSocket traffic.

## Acceptance

- No unexplained yellow object remains on either playback bank.
- The default Bump example clearly separates the playback name from its FLASH action and does not
  claim pickup is required.
- The touch fader again shows its colored level gradient.
- The touch percentage never overlaps `Cue 4 · Solo` or other mode feedback.
- Hardware page/slot addresses sit compactly at the top right while titles receive more width.
- Cue names and hardware action labels receive visibly more usable space.
- Every exceptional state is both semantically valid and textually understandable.
- Touch-only layout corrections do not change hardware behavior, and hardware layout corrections
  do not change touch interaction.

## Non-goals

- Do not change playback runtime, cue transition, fader, pickup, Flash, Swap, or Off semantics.
- Do not remove useful loaded/pickup/held feedback; make it valid and understandable.
- Do not replace playback colors with amber status color.
- Do not redesign Virtual Playbacks in this plan.
- Do not change the configured playback action order or page/slot addressing.

## Result

Implemented and accepted on 2026-07-26.

- The package and live touch bank now keep playback identity and a single faderless configured
  action in separate controls. **3 · Bump** remains the playback title while **FLASH** remains its
  press/release action.
- Pickup styling and text are suppressed when a playback has no fader. Loaded state occupies the
  summary's trailing fade-time position, pickup state is explained on the hardware fader, and
  held Flash or Swap feedback appears on the configured button rather than as a floating label.
- Removed empty cue-row marker elements. Hardware rows allocate width to cue number, cue name, and
  an optional real status; absent status consumes no column. Hardware addresses remain compact at
  the top right, action space was widened, and the fader column was reduced.
- Playback faders explicitly retain their level-clipped configured-color gradient. Mode/label and
  percentage use separate reserved areas with a contrast-backed numeric value.
- Consolidated the playback section to one freely configurable playback plus representative
  eight-by-two touch and hardware banks, with focused geometry/state assertions.

Focused verification completed:

- UI-library playback tests: 5 passed;
- desktop Playback Fader Bank tests: 42 passed; and
- repository architecture/source-size gate: passed.

The complete UI and desktop unit suites, both TypeScript gates, production Storybook build, and
all 209 integrated Storybook Playwright checks subsequently passed. Browser review exposed and
confirmed the fix for the inherited label/value overlap on the production fader surface.

A final live-adoption audit found that the desktop bank still duplicated the shared card markup
while reusing only its fader and action primitives. The touch and hardware desktop adapters now
render `TouchPlaybackCardView`, `HardwarePlaybackCardView`, and `HardwareCueRowsView` directly,
supplying only live controller callbacks, cue timing/projection data, and application-owned
assignment/configuration overlays. The package owns the bank grid, card color hierarchy, cue-row
markup, and touch/hardware geometry. Component markers are asserted in both the live adapter test
and the application Storybook stories so this split cannot silently return.

The design-reference refinement adds the complete playback family and hardware-shape contract:

- cue lists default to warm green, group masters to orange, dynamics to magenta, speed groups to
  cyan, special masters to white, and empty slots to opaque gray; custom playback colors still
  override those defaults;
- one button, one button plus fader, two buttons, three buttons, and three buttons plus fader are
  demonstrated in both touch and hardware modes;
- the touch identity, `page.playback` address, cue/status progress, speed beat indicators,
  selected outline, and family-colored fader are package-owned;
- hardware cards show button assignments on the left and the bottom-up, full-width fader value
  indication on the right, reducing previous/current/next cue context to the current cue when
  shallow;
- empty slots never expose a disabled or decorative fader; and
- dedicated 8 by 2 stories show eight one-button playbacks above eight three-button-plus-fader
  playbacks, with the lower row receiving the additional height.

The live desktop adapter now derives the same family model and runtime summaries as Storybook.
The former generic `#20c997` playback default is treated as a legacy fallback and resolves to the
family default at presentation time; explicitly configured playback colors remain intact.
