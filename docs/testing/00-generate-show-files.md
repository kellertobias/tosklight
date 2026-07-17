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

The maintained `Default Stage Show` file contains the complete built-in standard patch, fixture numbers, multipatches, logical heads, virtual dimmers, and stage layout. It has exactly 49 patched fixture records across four universes: front Fresnels 1–6 at `1.1`–`1.6`, the middle and outside ACL sets at `1.11` and `1.12`, the stage hazer at `1.13`, back profiles, LED washes, and Trackspots on universe 2, floor RGBW PARs, back RGB Sunstrips, and front RGB strobes on universe 3, and the overhead RGB multipatch at `4.1`. It also contains the same two enabled test routes as `compact-rig.show` and starts with an empty command line, selection, programmer, preload, and playback state.

## SHOW-000 — Copy a show with Save As

**Priority:** P0
**Primary layer:** Manual UI and show-file persistence

**Starting show:** Load the canonical `compact-rig.show`, immediately use Save As to create `show-000-compact-copy.show`, and continue with the copy as the active show.

**Actions:**

1. Open the show menu, click **Load**, find `compact-rig.show`, and click **Load Latest Autosave** for that show.
2. Reopen the show menu, click **Save As**, enter `show-000-compact-copy.show`, and confirm the Save As dialog.
3. Reopen the show menu and verify its active-show label is `show-000-compact-copy.show` before making a mutation.
4. Open Groups, click fixture 1 in Stage or Fixture Sheet, press `[REC]`, click stored empty Group 4, and store the selection. Confirm Group 4 now contains fixture 1 in the copy.
5. Press `[SET] [GRP] [4] [ENTER]`. Confirm this exact shortcut opens the Group properties modal for Group 4. In the modal, replace the name with `Copy Center Spot`, choose a non-default color with the color button, choose an icon, and confirm the modal. Verify the Group 4 tile displays the new name, color, and icon.
6. Press `[SET]`, then click the visible Group 4 tile. Confirm this alternate gesture opens the same Group properties modal with `Copy Center Spot` and the chosen color and icon already populated. Close the modal without changing them.
7. Reopen the show menu, click **Save Named Revision**, enter a descriptive revision name such as `SHOW-000 compact mutation`, and confirm it. Choose **Load Revision as Copy** and verify a separately named revision copy becomes active while Group 4 still contains fixture 1 and retains its name, color, and icon. Confirm the Show menu identifies the source show and revision and says new changes autosave only to the copy. Then use **Load** to reopen canonical `compact-rig.show`.
8. Open Groups and verify canonical Group 4 is still named `Center Spot`, is still stored empty, and did not acquire the copy's color or icon.
9. Use **Load** to open canonical `default-stage.show`. Immediately click **Save As**, enter `show-000-default-copy.show`, and confirm it.
10. Click fixture 1, then press `[REC] [GRP] [9] [0] [0] [ENTER]` on the Lightning Desk. Confirm Group 900 exists in the copy through the Groups object API; the visible pool contains only its configured slot range and does not expose slot 900.
11. Save a named revision of the copy, then use **Load** to reopen canonical `default-stage.show`.
12. Query the canonical show's Groups objects and verify Group 900 does not exist.

**Group properties UI contract:** both `[SET] [GRP] <group-number> [ENTER]` and `[SET]` followed by clicking that Group's tile open the same Group properties modal. The modal contains an editable name field, a color button, and an icon control. Confirming it persists all three properties; cancelling or closing it without confirmation makes no changes. The configured visible slot range does not need to expose Group 900, so that Group's existence is verified through the object API.

**Assertions:**

- Save As creates a distinct show file and makes that copy active.
- Each copy initially preserves the canonical show's patch layers, patched fixtures, groups, routes, stage layout, and clean programmer/playback state exactly.
- Before its deliberate mutation, `show-000-compact-copy.show` contains 16 fixtures, distinct `Dimmers` and `LEDs` layers, groups 1–3 with the listed ordered membership, and stored empty Group 4 named `Center Spot`.
- Both Group-property gestures open the same modal for Group 4, and its edited name, color, and icon appear on the Group tile and survive loading that named revision as an independent copy.
- Loading the named revision creates a new show identity with stable source provenance; it does not replace the working copy's newer Latest Autosave or the canonical source.
- `show-000-default-copy.show` contains the complete 49-record built-in patch and stage layout.
- Editing and saving either copy does not alter its canonical source file.
- Both canonical files remain reusable starting points after the test.

**Pass condition:** Save As produces independent working copies while leaving both canonical show fixtures unchanged.
