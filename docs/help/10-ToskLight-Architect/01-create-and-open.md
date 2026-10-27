# Create and open a rig

Start ToskLight Architect to plan a rig in a portable `.show` document. On first launch, Architect opens a writable Demo Show copy. Later launches reopen the recent document.

Use **New Show** on the Show screen for a new rig, or choose **Open > Load Show from Disk** for an existing file. A show brought from Control can stay synchronized with that desk; see [Work with Control](07-control-and-transfer.md).

The sidebar opens **Show**, **CAD**, **Patch**, and **Media**. **Settings** holds Visualizer, Library, and DMX configuration. **Open Window** opens another Architect window on the same document, useful for keeping CAD and Patch on separate screens. The renderer opens separately through **Open Viz**.

## Open the Demo Show

Choose **New Show > Open Demo Show** for a writable copy of the bundled rig. The copy contains a patched lighting rig, scenery, effects, crowds, and media surfaces, ready to inspect in the Visualizer.

Each use creates a separate show in this installation's shows folder: **Demo Show**, then **Demo Show 2**, and so on. The file bar reports the copy's name and location. You can edit, rename, or delete it; the bundled template remains unchanged.

The Desk's **Default Stage Show**, Architect's Demo Show copies, and Visualizer demo mode use the same portable template and embedded fixture revisions.

## The Show screen

**Show**, at the top of the dock, is the open show as a document. It is available with no show open,
because it is where a show is created or opened. Its file actions run across the top: **New Show**
and **Open Demo Show** under **New Show**; **Load Show from Disk**, **Load Recent Shows**, and **Load from ToskLight Control** under **Open**; then **Save As**,
**Publish to ToskLight Control**, **Import MVR** and **Export MVR**.

**Load Recent Shows** opens a modal with a scrollable table of files opened on this computer,
newest first. Names omit the `.show` extension; Location shows an Internal badge for internal files, and Last saved shows the file timestamp. Files moved or deleted are omitted. Use the modal close control to dismiss it.
**Load from ToskLight Control** lists only active shows announced by discovered desks.
The table includes the desk IP address and operating system when announced. There is no manual host or IP input.

A show loaded from a desk stays synchronized with that desk. Use **Save As** for an independent copy, or **Publish to ToskLight Control** to add a locally planned show to a desk. See [Work with Control](07-control-and-transfer.md).

Below them the show is seen from both sides at once. The left half draws the rig from above, with the
show's name over it. The right half is **Show information**, which titles every printed CAD page, in
two columns:

* **Show**, on the left: **Project**, **Venue**, **Show date** and **Version**.
* **Lighting designer**, on the right: **Name**, **Phone**, **Email** and the **Company logo**.

Below both columns are the show name, last-saved time, and fixture and universe counts. Project information saves automatically to the open show and appears in printed title blocks. Wait for **Saving project information…** to finish before closing. A failed save keeps your draft and offers **Retry saving project information**.

**Upload logo** accepts PNG, JPEG, or WebP up to 10 MB. **Replace logo** and **Remove logo** save automatically too. The show carries the image, and each printed title block fits it without changing its proportions.

**Make Default** keeps the lighting designer's name, phone, email and logo on this computer, not in
the show. Every show created afterwards with **New Show** starts with them; shows that are opened,
copied from a desk or opened from the demo keep their own. Press it again whenever the details change.

## Rename the show

A show has a name of its own, separate from its file. The **Show** screen shows it over the rig
overview, with a pencil beside it. Press the pencil and type the new name. Press Enter or click away to keep it; Escape cancels, and an empty name changes nothing. The name is
what a desk's **Load Show** menu offers, so a renamed show is offered under its new name at once.
The file keeps its name; **Save As** writes the show to a new one. Renaming a show that follows a
desk renames it on this computer only: the desk's show keeps the name its own library gave it.
