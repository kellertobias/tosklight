# Semantic Color controls

## Purpose and status

These scenarios are the acceptance contract for TL-550. They cover the production Color Special
Dialog, its compact and full layouts, Shift ranges, the desk's Easy/Advanced Color setting, Media
layers and the quiet Fixture Sheet Color status under the fixture-independent programming
contract. Production reports programming contract 1 (TL-552), so the semantic Color dialog and
encoder pages are the desk's Color controls.

Executable coverage:

- Vitest: `apps/light-desktop/src/components/modals/specialDialogs/intention/color/`,
  `…/parameterControls/ParameterControlView.inline.test.tsx`,
  `apps/light-desktop/src/features/colorReport/`,
  `apps/light-desktop/src/windows/fixtureSheetColorNotice.test.tsx` and
  `…/windows/setupWindow/ColorPresentationSettings.test.tsx`.
- The Storybook specs `fixture-abstraction-mockup.spec.ts` and
  `fixture-abstraction-software-shift.spec.ts` cover the shared `ColorDialogLayout`, pickers and
  touch faders at every established viewport.
- The root Playwright spec `tests/65-semantic-special-dialogs-and-hardware-selection.spec.ts`
  carries COLOR-RANGE-001, the SEMANTIC-COLOR-003 White Blend range from the keyboard Shift (one
  drag) and the attached hardware Shift (two touches), its Undo and a cancelled first endpoint.
- The root Playwright spec `tests/112-color-intent.spec.ts` carries SEMANTIC-COLOR-001 and 004.
- The root Playwright spec `tests/119-semantic-color-controls.spec.ts` carries SEMANTIC-COLOR-002,
  005 and 006 at 1496×761 in the software-only layout: the full modal's frame and its **Color** and
  **Details** tabs, the hue ring and faders without scrolling, and the scrolling approximation and
  Direct color of a JBLED A7 / ROOT PAR 6 / wheel-only rig (the wheel profile carries measured
  slots, because an uncalibrated head never shows a Δu′v′); the Media color dialog with White Blend
  read from the layers' DMX greyscale and tint and from the preview card, with layer and Master
  Intensity unchanged; and the Fixture Sheet triangle, its single batched report request, the
  absence of any message or focus move, and tap and Enter opening the **Details** tab.
- The same spec carries SEMANTIC-COLOR-007 at 1600×1000 in the software-only layout: the existing
  `tests/fixtures/default-stage.show`, the packaged Default (`assets/demo.show`) and the Clean
  Built-in Default loaded through **New Show**; the page-1 readouts, one Red detent and a programmed
  blue. Vitest: `…/familyEncoders/colorSlotDisplay.test.ts`.
- The same spec carries SEMANTIC-COLOR-008 at 1600×1000 in the software-only layout: two Generic RGB
  LEDs with a red, a red/blue and a black Cue on Playbacks 1–3, the page-1 readouts and the full
  modal's Hue, Saturation and White Blend with an empty Programmer, and one Red detent. Vitest:
  `…/familyEncoders/colorSlotDisplay.test.ts`; Cargo: `position_readout::color_seed_tests`.
- All of these run under `npm run test:e2e`, because production reports programming contract 1;
  `npm run test:e2e-semantic` runs them on the contract-1 E2E test server, where a missing semantic
  publication fails instead of skipping.
- Manual: the 1024×768 and 760×900 viewports and the hardware-connected layout for 002, the physical
  Media picture for 005 (TL-523) and the absence of sound for 006.

Physical colour, UV and Media picture evidence is manual (TL-523). Record the build, the desk mode,
the viewport, the fixtures and the Media personality with every manual run.

Run each visual check at 1496×761, 1024×768 and 760×900, in both the software-only and the
hardware-connected layout.

## SEMANTIC-COLOR-001 — Compact dialog in the measured encoder area

1. On a desk reporting the semantic programming contract, select three RGB fixtures and open the
   **Color** tab, then press **Special Dialog**.
2. Where the lower encoder area itself measures at least 680 × 210, verify the compact dialog
   replaces the encoders inside that area:
   - page 1 holds the 2D hue/saturation picker on the left and the coloured horizontal
     **White Blend** touch fader on the right, with **White balance** and **Expand** directly below
     the fader;
   - there is no header, footer, status line, fixture count, approximation, preset or details tab
     and no **Encoders** button.
3. Press **White balance**. Verify page 2 shows exactly **Temperature** and **Duv**, warm → white →
   cool and magenta → white → green, each white at its centre. Press **Special Dialog**: page 1
   returns.
4. Tap the active **Color** tab. Verify the encoders return on the encoder page they were on,
   without paging; the next tap pages as usual. The upper workspace is unchanged throughout.
5. Where the area is smaller than 680 × 210 — including at 1024×768, 760×900 and every
   hardware-connected layout whose area is smaller — verify **Special Dialog** opens the full modal
   directly. Resizing the window so the area crosses the budget switches between the two.

## SEMANTIC-COLOR-002 — Full modal

1. Press **Expand**, or open the dialog where the compact budget does not fit.
2. Verify the standard modal frame titled **Color** with its close button and two tabs in the
   title bar, **Color** and **Details**, opening on **Color**.
3. Verify the **Color** tab is the colour selection alone: a large hue ring (about 340 px) with
   **Saturation**, **White Blend**, **Temperature** and **Duv** together beside it, without vertical
   overflow at 1496×761.
4. Verify the **Details** tab holds the per-fixture approximation, then **Direct color**, and
   scrolls inside the modal when it is taller than the window.
5. With a mixed JBLED A7 / ROOT PAR 6 / wheel-only selection programmed magenta and output once,
   verify the **Details** approximation shows the requested swatch once, one row per head from the output that
   was sent, the visible match with Δu′v′, and UV in its own column: **UV applied**, **UV limited by
   the emitter** or **UV unavailable on this fixture**. A fixture without a programmed colour is
   not listed.

## SEMANTIC-COLOR-003 — Shift ranges and edits

1. Arm the on-screen **SHIFT**, touch White Blend at 80%, then at 20%. Verify marker **1** and the
   hint appear after the first touch with nothing written, then markers **1** and **2**, the readout
   **80% → 20%**, and fixture values stepping 80 → 50 → 20 in selection order.
2. Repeat with the keyboard Shift held for both touches and with the attached hardware Shift.
   Verify identical results.
3. Spread Hue 350° → 10°. Verify the middle fixture is red (0°), never cyan. Spread 0° → 180°:
   the middle fixture is 90°, clockwise.
4. Touch Saturation without Shift. Verify only Saturation returns to one value; the Hue and
   White Blend ranges stay.
5. Verify every touch or drag is one Undo step, follows Blind, Preview and Preload, and that the
   same gestures with nothing selected change nothing and show no message.

## SEMANTIC-COLOR-004 — Easy and Advanced belong to the desk

1. Program a colour with a Temperature and a UV request.
2. In **Setup → Attributes & encoders → Color model → Color controls on this desk**, switch between
   **Easy**, **Easy with Amber and UV** and **Advanced**.
3. Verify the Color encoder pages change (Advanced adds Temperature, Duv and the wheels), the
   programmed colour, Temperature and UV are unchanged, and no Undo entry is added.
4. Save and reopen the show on another desk. Verify the show carries no Color presentation and the
   other desk keeps its own setting.

## SEMANTIC-COLOR-005 — Media layers

1. Select two Media Server layers and open **Color → Special Dialog**. Verify the dialog is titled
   **Media color** and page 2 is **Preview** instead of Temperature and Duv. In the full modal the
   second tab is **Preview** instead of **Details**.
2. Set White Blend to 0%, 50% and 100%. Verify the picture desaturates progressively while the
   tint remains; a 100% red tint gives a red picture; a neutral picture needs the tint at full
   red, green and blue.
3. Verify layer and Master Intensity are unchanged by every Color edit.
4. Add a lamp to the selection. Verify the lamp dialog opens instead.

## SEMANTIC-COLOR-006 — Quiet Fixture Sheet status

1. Request UV on a fixture without UV and output once. Verify one small, steady, subdued triangle
   beside its Color value, from one report request for the rows on screen.
2. Verify no toast, banner, sound, alert or live region appears and focus does not move while the
   output keeps changing.
3. Tap the triangle (or press Enter on it). Verify the full Color modal opens on its **Details**
   tab with that fixture's row marked and the selection is unchanged.

## SEMANTIC-COLOR-007 — Readouts in existing and shipped shows

1. Open an existing Default Stage Show saved by an earlier build, then the Default show, then
   **New Show → Load Clean Built-in Default**. In each, select colour fixtures that hold no colour
   (for example an RGBW PAR, a JBLED A7 and a Robin DLS) and open the **Color** encoders.
2. Verify page 1 reads **Red**, **Green**, **Blue** and **White Blend** with the open-white start
   every first edit uses: `100%`, `100%`, `100%`, `0%`. No control reads a dash.
3. Turn **Red** one detent down. Verify the encoder reads the requested value and the other
   controls keep theirs.
4. Program blue. Verify Red and Green read `0%` (black and zero are values) and Blue `100%`. A
   selection whose fixtures differ reads **Mixed**; a fixture holding a Direct colour reads
   **Direct** until a page 1 or 2 edit adopts it.

## SEMANTIC-COLOR-008 — Readouts of a running Cue or Playback colour

1. Program two RGB fixtures red and record the Cue; record a second Cue with one fixture red and
   the other blue, and a third with both black. Clear the Programmer.
2. Run the red Cue, select both fixtures and open the **Color** encoders. Verify page 1 reads
   **Red** `100%`, **Green** `0%`, **Blue** `0%`, **White Blend** `0%` — the Cue's colour, not the
   open-white start — while the Programmer stays empty.
3. Open the Color special dialog. Verify it shows the same colour: Hue 0°, Saturation 100%, White
   Blend 0%.
4. Turn **Red** one detent down. Verify the Programmer holds the Cue's red slightly reduced, with
   Green and Blue still 0: the first edit started from the Cue's colour.
5. Clear the Programmer and run the red/blue Cue instead. Verify Red and Blue read **Mixed** and
   Green `0%`. Run the black Cue instead: every control reads `0%`, never a dash or open white.
6. Release every Cue. Verify the readouts return to the open-white start (`100%`, `100%`, `100%`,
   `0%`).
