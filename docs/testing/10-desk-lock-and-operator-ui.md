# Desk Lock and Operator UI Review

These scenarios verify desk-scoped input suppression and the concrete operator-interface corrections retained by the completed Desk Lock and manual-review features. They use fresh sessions and unique temporary objects/files in the isolated Playwright worker. They do not claim API equivalents for terminology, geometry, CSS alignment, or pane composition that exist only in the production UI.

## LOCK-001 — Desk Lock covers every screen and input surface

**Priority:** P0

**Primary layers:** One integrated Playwright `@ui @api @osc` case plus one `@ui` fallback case

**Starting fixture:** Open two browser screens attached to the same physical desk and retain a separate session for another desk. Subscribe one OSC client to the locked desk alias and capture the desk's current DMX state.

### PIN mode

1. Configure the current desk with message `Call the operator`, a valid data-image wallpaper, PIN mode, and PIN `1234`; then lock it.
2. Confirm the lock dialog, message, and wallpaper appear on both existing screens. Open a third screen after locking and confirm it immediately presents the same lock state.
3. Attempt a Grand Master API write and a command WebSocket write through the locked desk.
   - **Expect:** Both are rejected because the desk is locked.
4. Send an OSC keypad digit to the locked desk alias.
   - **Expect:** The command line and captured DMX remain unchanged.
5. Use the separate desk session.
   - **Expect:** That desk is not locked and can still change its own Grand Master.
6. Enter an incorrect PIN on the first desk.
   - **Expect:** The dialog reports **Incorrect PIN** and remains visible on every screen.
7. Enter `1234` and click **Unlock Desk**.
   - **Expect:** The lock disappears from all three screens and the authoritative desk state reports unlocked.
8. Send OSC digit 1 again.
   - **Expect:** The command line becomes `F1` and the browser command-line display agrees.

### Button mode and fallback presentation

1. Configure button mode with no message, no PIN, and an unavailable wallpaper URL; then lock the desk.
2. Confirm the readable fallback text **This desk is locked.**, no PIN field, and a visible **Unlock Desk** button.
3. Click the button and confirm the lock closes.

**Assertions:** Lock state is physical-desk scoped, synchronizes every current and late-attached screen, rejects API and command writes, drops OSC input, preserves output, leaves other desks independent, validates PINs, and retains a readable no-PIN fallback.

**Pass condition:** A locked desk cannot be operated through any attached screen, API session, or OSC controller, yet output remains stable and unrelated desks continue operating.

## MANUAL-019 — Operator-visible software corrections remain coherent

**Priority:** P1

**Primary layer:** Eight production Playwright `@ui` cases, with API/OSC observers only where the visible workflow needs authoritative state verification

**Starting fixture:** Use the worker's active show and authenticated session. Each subcase creates only the temporary Cuelist, route, show copy, or confined files it needs and removes temporary files where applicable.

### Desktop terminology does not rename the physical desk

1. Confirm the saved-workspace dock says **DESKTOPS**, not **DESKS**, and offers **New desktop**.
2. Long-press the current Desktop, confirm **Desktop settings**, rename it, clone its complete pane layout, and delete only the clone.
3. Confirm the session retains the same physical desk ID and OSC alias. Subscribe OSC to that alias and prove a keypad digit still routes into the same desk.
4. Open the show controls and confirm **Desk Status**, **Shut Down Desk**, and **Desk Setup** retain desk terminology for the physical/logical control surface.

### Fixture browsers share the title-bar search layout

1. Open **+ Add fixture** from Patch. Confirm search is inside the fixture-browser header, names align left, and neighboring metadata aligns right.
2. Open **Setup → Fixture library**. Confirm search, **Import GDTF**, and **Create fixture** occupy the title action area; list names align left and metadata/detail values align right.

### Every operator file field uses the confined picker contract

1. Disable system-picker fallback and create one decoy `.txt` plus valid `.show`, `.mvr`, `.gdtf`, image, and `.glb` files under `Shows`.
2. Open each production field: Load from flash drive, MVR import, GDTF import, fixture stage icon, fixture 3D model, Desk Lock wallpaper, and Stage scene import.
3. For every picker, prove the decoy cannot be confirmed and the field's valid extension can be selected.

### File Manager and Text Editor actions live in pane headers

1. Add File Manager. Confirm **Edit**, **Create**, **View**, **Back**, and **Forward** are inside its pane header, not duplicated in the body toolbar, and the root-relative path updates beside the title.
2. Use Pane Settings to show a hidden file and the View menu to hide Properties.
3. Add Text Editor. Confirm **Open File**, **Refresh**, **Save**, and **Save As** are inside its pane header, not duplicated in the body toolbar.
4. Open a Markdown file, edit it, and save. Confirm the header state moves from **Saved** to **Unsaved** and back to **Saved**, and the file API contains the edit.

### Cues pane keeps the editor and omits direct deletion

1. Create a Cuelist with one named Cue and add **Cues · Cuelist** to a fresh Desktop.
2. Confirm the right-side Cue editor is visible with **Title**, **Fade**, **Delay**, and **Trigger**, and the title contains `House Open`.
3. Confirm the pane has no **Delete Cue** button; Cue deletion remains the documented command workflow.

### Help, Outputs, DMX, and Stage retain their distinct responsibilities

1. Add Help and compare its two column boxes. Navigation remains left of the topic content and both start at the same vertical position.
2. In **Desk Setup → Outputs**, edit an existing route, add another route, and remove the new route through explicit editor confirmation.
3. Open DMX. Confirm there is no Routes button; select a patched channel and confirm the monitor shows its fixture identity.
4. Open Stage setup. Confirm Stage Settings does not preselect a built-in element. Click **Add element**, choose **Stage deck · 2 × 1 m**, and confirm the asset inspector shows that choice.

### Development is diagnostic, not an operator pane

1. Open the new-Desktop pane chooser and confirm **Development** is absent. Trigger the former Shift-0 desk action and confirm it does not open Development.
2. Open **Desk Status → Debug → Open Development** and confirm the Development window opens there for diagnostics.

### Shows & recovery loads through safe blackout

1. Create an indexed show copy and a non-show decoy file. Open **Desk Setup → Shows & recovery**.
2. Confirm the embedded file manager starts at `Shows`. Select the decoy and confirm **Load selected show safely** is disabled.
3. Select the `.show` copy and click the enabled load action.
4. Confirm the request opens that indexed show with `{ transition: "safe_blackout" }`, the status says the file is now open, and bootstrap reports the copy as active.

**Assertions:** Each subsection is implemented directly by a named `MANUAL-019 @ui` case in `tests/19-manual-review-software-corrections.spec.ts`. The suite checks literal text, accessible controls, placement/alignment, persisted or server-observed state where relevant, OSC desk identity, confined extension filters, exact route mutation, selected DMX fixture identity, Stage choice, diagnostic-only Development access, and safe-blackout show activation.

**Pass condition:** Terminology, pane composition, file selection, Cue editing, Help, output setup, DMX monitoring, Stage assets, diagnostics, and recovery behave as one coherent operator application without conflating saved Desktops with physical desks.
