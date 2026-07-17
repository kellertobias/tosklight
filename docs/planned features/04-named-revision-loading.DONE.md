# Named Revision Loading

## Status

**Implementation status: Complete.** Provenance-linked revision copies, collision-safe identities, isolated autosave, visible source communication, explicit Save/Save As overwrite choices, recovery backup, immutable source/destination revision handling, help, and paired `SHOW-005` acceptance coverage are implemented.

## History model

A show has one mutable **Latest Autosave** and zero or more immutable **Named Revisions**. Autosave continuously updates Latest Autosave; intermediate autosaves are not presented to the operator as revision history. Internal recovery backups may still be retained, but they are recovery data rather than selectable show revisions.

**Load Show** always opens the selected show's Latest Autosave, regardless of which named revision preceded the latest changes. It does not ask the operator to choose a revision.

Each named revision has a separate **Load Revision as Copy** action. Loading a named revision must never rewind, replace, hide, or otherwise modify the original show's Latest Autosave.

## Revision copies

**Load Revision as Copy** creates and activates a separate show file from the immutable named-revision snapshot. The copy has its own show identity and its own Latest Autosave. All subsequent changes are autosaved only to the copy until the operator explicitly chooses another destination.

The generated name uses the original base name, revision number, and the date on which the copy was created:

`<base-name>-rev-<revision-number>-<copy-date>`

If that name already exists, append a disambiguating suffix without overwriting either copy. The copy also retains stable provenance metadata identifying the original show and named revision; its relationship to the original must not depend only on parsing the generated name.

The original show remains intact throughout this workflow:

- its Latest Autosave is not changed;
- the loaded named revision remains immutable;
- its other named revisions remain available; and
- later autosaves that were made after the loaded revision remain the original show's Latest Autosave.

The revision copy is an ordinary independent show and remains available after switching shows or restarting the desk. It can receive its own named revisions without adding those revisions to the original show.

## Show menu communication

Immediately after loading a revision copy, the Show menu must identify both the active copy and its source. It must communicate at least:

- that the active show is a separate revision copy;
- the original show's name;
- the source revision number and name;
- when the copy was created; and
- that current changes are being autosaved to the copy, not to the original.

The normal show identity outside the menu must also make the revision-copy state visible so the operator cannot mistake it for the original show while programming.

## Manual Save and Save As

Autosave already protects current work in the revision copy. A manual **Save** from a revision copy therefore resolves where that copy should live; it is not required merely to persist the latest edits. The Save workflow asks whether to:

1. keep the active copy as a separate show file; or
2. overwrite the original show's Latest Autosave.

**Save As** supports entering a new show name or selecting an existing show file as the overwrite destination. The original show must be available as an explicit destination while its provenance association remains valid.

Selecting the original or any other existing show does not overwrite it immediately. The desk must show a separate confirmation that names the destination and explains that its Latest Autosave will be replaced. Overwriting the original requires an explicit destructive confirmation; cancel remains the safe default.

Overwriting an existing show replaces only that show's mutable Latest Autosave. It must not:

- delete or rewrite any of the destination show's named revisions;
- change the destination show's identity or name;
- mutate the source named revision;
- transfer the revision copy's named revisions into the destination; or
- silently delete the revision copy.

Before replacing an existing Latest Autosave, create an internal recovery backup of the previous state. After a successful overwrite, loading the destination show opens the copied state, while its existing named revisions remain unchanged. The separate revision copy continues to exist until the operator explicitly deletes it.

If the original show no longer exists, the copy remains usable as an independent show, but **Overwrite Original Show** is unavailable. The operator may still keep it separately or use Save As with another valid destination.

## Acceptance contract

- Loading a show opens its Latest Autosave.
- Loading a named revision creates and activates a separately autosaved show copy.
- Creating or autosaving that copy does not modify the original show or obscure its newer work.
- Generated copy names identify the original base name, source revision number, and copy date without overwriting an existing copy.
- The desk visibly identifies the active show as a revision copy and shows its source revision.
- Manual Save asks whether to keep the copy separate or overwrite the original.
- Save As can target the original or another existing show, but every overwrite requires a separate explicit confirmation.
- Overwriting replaces only the destination's Latest Autosave and preserves all named revisions and show identity.
- The source revision remains immutable, and the revision copy is retained until explicitly deleted.
