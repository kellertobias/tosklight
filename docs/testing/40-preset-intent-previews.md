# Preset intent previews

## Purpose and status

These scenarios are the acceptance contract for TL-656: Color and Position Preset tiles preview
the programming intention they store. The operator contract is
[Groups and Presets › Automatic Color and Position previews](../help/10-Desk/20-Programmer-and-Cues/03-groups-and-presets.md#automatic-color-and-position-previews).

Executable coverage:

- Vitest: `apps/light-desktop/src/features/presetPreview/presetPreview.test.ts` (display colour,
  multi-colour swatch, spreads, Group families, dot sampling with kept extremes and at most ten
  deterministic dots, precedence) and
  `apps/light-desktop/src/windows/presetsWindow/PresetCardGrid.preview.test.tsx` (tile rendering,
  live update, explicit override, legacy tile, **Automatic icon**).
- Storybook: story `ToskLight/Windows/Pools › Intent Preset Previews`, checked by
  `apps/ui-library/storybook/tests/preset-previews.spec.ts`.
- Root Playwright: `tests/129-preset-intent-previews.spec.ts` covers PRESET-PREVIEW-001 to 003.

The previews read only the stored Preset; none of these steps recalls a Preset or changes the
Programmer. Rig: four fixtures patched in a Color Intent show; the scenarios store Presets directly
as show objects, the way Record and Update leave them.

## PRESET-PREVIEW-001 — Color presets show their distinct colours

1. Store Color 1 as one universal red Color Intent, and Color 2 with fixtures 1 and 3 red and
   fixtures 2 and 4 blue.
2. Open the Preset pool on the Color family. Tile 1 shows a single red swatch. Tile 2 shows two
   segments, red then blue, and is announced as *Color preview, 2 colours*. It is never one purple
   average.
3. Store Color 3 as a universal Color Intent with a hue spread through 0°, 120° and 240°. Tile 3
   shows red, yellow, green, cyan and blue segments.
4. Store Color 4 holding only a colour-wheel slot (`color.wheel.1` Deep Red), a value without a
   Color Intent. Tile 4 keeps the plain tile without a preview.
5. Check the Programmer: it holds no values, and no Preset was recalled.

## PRESET-PREVIEW-002 — Position presets show up to ten representative aims

1. Store Position 1 with five fixtures fanned from Pan −60° to +60° at Tilt 45°. Open the Position
   family. Tile 1 shows a square with five dots on one horizontal line.
2. Store Position 2 with thirty aims in a six-by-five Pan/Tilt grid. Tile 2 shows exactly ten dots,
   including the four corners of the grid and the highest and lowest aim on each axis, and is
   announced as *Pan/Tilt position preview, 10 of 30 aims*.
3. Store Position 3 as one universal Target at Stage X 0 m, Y 2 m. Tile 3 shows one dot in the
   centre of its square, marked as a Target preview.

## PRESET-PREVIEW-003 — Live updates, explicit icons and both layouts

1. Update Color 1 to a universal green. Without touching the pool, tile 1 turns green.
2. Open Color 1's button settings with `[SET]` and tap the tile; choose the icon **★** and save.
   Tile 1 shows the star and no automatic preview. Open the settings again, tap **Automatic icon**
   and save: the green preview returns.
3. Connect the simulated hardware controller so the desk switches to the hardware-connected layout.
   The Color and Position tiles show the same previews, and the position square stays square and
   inside the tile's picture box.
