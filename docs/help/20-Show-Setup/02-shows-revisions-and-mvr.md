# Shows, Revisions, and MVR

A `.show` file is the portable source for patch, stage layout, groups, presets, Cuelists, playbacks, and other show objects. Desk configuration and operator identity do not travel with it.

## Create, load, and revise

Use the Show menu to create a show, upload/open a show from the library, or save a copy under a new name. Creating an empty show immediately creates, activates, and autosaves a real show named **New Empty Show** (with a number when needed); programming is protected before you choose a final name. The first **Save As** on that provisional show renames the same show, preserving its identity and current programming. **Save Named Revision** creates an immutable numbered restore point. **Load Latest Autosave** always resumes that show's newest work. **Load Revision as Copy** creates and activates a separate show from the selected restore point; it never rewinds or replaces the original show's Latest Autosave.

The generated copy name includes the source show, revision number, and copy date. The Show menu identifies it as a separate revision copy and keeps the source show name, revision number and name, and creation time visible. The left dock also labels the active show as a revision copy. All subsequent changes autosave to the copy, not the original. The copy remains in the show library after switching shows or restarting the desk, and it can have its own named revisions.

## Save a revision copy

Because autosave already protects the active copy, **Save** asks where the copy should remain. Choose **Keep as Separate Show** to leave it independent, or **Overwrite Original Show** to replace the original show's mutable Latest Autosave. If the original was deleted, the overwrite-original choice is unavailable and the copy remains fully usable.

For an established, named show, **Save As** can create another named show or select the original or another existing show as a destination. Choosing an existing destination opens a separate destructive confirmation; cancel is the safe default. A confirmed overwrite first creates an internal recovery backup, replaces only the destination's Latest Autosave, and preserves the destination identity and all of its named revisions. The revision copy and immutable source revision are retained until explicitly deleted.

![Show menu and its primary show-management actions](../assets/screenshots/workflows/show-menu.png)

![Load the latest autosave or a named revision](../assets/screenshots/workflows/show-load-revisions.png)

## Import MVR

Choose **New Show > Load from MVR** and review the preview before creating the show. The preview reports matched profiles, missing GDTF modes, address conflicts, and scenery. Resolve each conflict by choosing a safe address, importing unpatched, or skipping it. Apply only after checking the result. Merge-into-existing-show support exists internally but currently has no operator control and must not be relied on as an available workflow.

![Start a new show from an MVR archive](../assets/screenshots/workflows/mvr-new-show.png)

Embedded GDTF files are imported into the desk fixture library. Fixtures without a matching definition remain visible as unresolved import records instead of being silently discarded.

## Export MVR

Export preview reports fixture and scenery counts, embedded profiles, missing retained source profiles, omissions, and warnings. The export includes fixture UUIDs, patch, transforms, supported scenery, and retained GDTF sources where available. Resolve warnings before relying on the archive as an interchange master.

![MVR export preview](../assets/screenshots/workflows/mvr-export.png)

MVR is an exchange format, not a replacement for the native `.show` history. Keep the native show and named revisions as the operational source.
