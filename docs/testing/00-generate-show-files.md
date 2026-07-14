# Reusable Show Files and Save As

**Automated coverage:** Implemented by [`tests/00-generate-show-files.spec.ts`](../../tests/00-generate-show-files.spec.ts). Run both independent surfaces with `./test e2e`, or select one with `./test e2e-api` and `./test e2e-ui`.

The test suite keeps two canonical show files as stable fixtures. Prepare them once, retain them between runs, and update them deliberately when the fixture contract changes. Do not regenerate them as routine test setup. Every test opens one canonical file, immediately saves it under a test-specific name, and performs all mutations against that working copy.

## Canonical file contracts

### `compact-rig.show`

The maintained `Compact Rig` file contains:

- Patch layer `Dimmers`, containing fixtures 1–12 as `Generic / Dimmer / 8-bit` on universe 1 at addresses 1–12.
- Patch layer `LEDs`, containing fixtures 21–24 as `Generic / RGB LED / RGB virtual dimmer` on universe 1 at addresses 13, 16, 19, and 22. Each fixture occupies three physical RGB channels; its intensity is a virtual dimmer and occupies no DMX channel.
- RGB fixture names `RGB LED 1` through `RGB LED 4`.
- These ordered groups:

  | Group | Name | Ordered members |
  | --- | --- | --- |
  | 1 | All Dimmers | 1–12 |
  | 2 | Odd Dimmers | 1, 3, 5, 7, 9, 11 |
  | 3 | Front Dimmers | 1, 2, 3, 4 |
  | 4 | Center Spot | Empty |

- Two enabled routes: logical universe 1 to Art-Net universe 1, and logical universe 1 to unicast sACN universe 101.
- An empty command line, selection, programmer, preload, and playback state.

### `default-stage.show`

The maintained `Default Stage Show` file contains the complete built-in standard patch, fixture numbers, multipatches, logical heads, virtual dimmers, and stage layout. It has exactly 49 patched fixture records, including the front Fresnels, back profiles and LED washes, RGB Sunstrips, RGB strobes, RGBW PARs, ACL sets, overhead RGB multipatch, Trackspots, and stage hazer. It also contains the same two enabled test routes as `compact-rig.show` and starts with an empty command line, selection, programmer, preload, and playback state.

## SHOW-000 — Copy a show with Save As

**Priority:** P0
**Primary layer:** Manual UI and show-file persistence

**Starting show:** Load the canonical `compact-rig.show`, immediately use Save As to create `show-000-compact-copy.show`, and continue with the copy as the active show.

**Actions:**

1. Verify the active show is `show-000-compact-copy.show`.
2. Rename empty group 4 from `Center Spot` to `Center Spot Copy`, save the copy, and close it.
3. Reopen canonical `compact-rig.show` and verify group 4 is still named `Center Spot`.
4. Load canonical `default-stage.show`, immediately use Save As to create `show-000-default-copy.show`, and continue with that copy as the active show.
5. Create empty group 900 named `Copy Marker`, save the copy, and close it.
6. Reopen canonical `default-stage.show` and verify group 900 does not exist.

**Assertions:**

- Save As creates a distinct show file and makes that copy active.
- Each copy initially preserves the canonical show's patch layers, patched fixtures, groups, routes, stage layout, and clean programmer/playback state exactly.
- `show-000-compact-copy.show` contains 16 fixtures, distinct `Dimmers` and `LEDs` layers, groups 1–3 with the listed ordered membership, and empty group 4 named `Center Spot`.
- `show-000-default-copy.show` contains the complete 49-record built-in patch and stage layout.
- Editing and saving either copy does not alter its canonical source file.
- Both canonical files remain reusable starting points after the test.

**Pass condition:** Save As produces independent working copies while leaving both canonical show fixtures unchanged.
