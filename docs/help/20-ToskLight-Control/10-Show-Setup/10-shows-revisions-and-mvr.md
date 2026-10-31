# Create, open, and save shows

Keep the native `.show` file as the production source. It contains the patch, installed positions, embedded fixture profiles, Groups, Presets, Cuelists, Playbacks, and other show objects. Desk configuration stays with the installation.

## Create a show

Open the Show menu and choose **New Show**. Start with an empty show, a clean Demo Show, a template, or [an MVR import](18-mvr-exchange.md).

**Create Empty Show** immediately creates and autosaves a working show named **New Empty Show**, adding a number when needed. You can program before naming it. Its first internal **Save As** renames that same show and keeps its identity and programming.

On a new installation, **Default Stage Show** is the working copy of the bundled Demo Show. Your changes autosave to that copy. **Load Clean Built-in Default** creates a separate fresh copy from the untouched bundled source.

Use [template shows](19-template-shows.md) for your own reusable starting rigs and programming.

## Load a show

1. Choose **Load Show** from the Show menu.
2. Open **Source: Internal** in the title bar and choose **Internal**, a listed **USB** drive, or **Network**.
3. Browse folders. The location bar above the table names the folder you are in; choose any step in it, or **Up one folder**, to go back.
4. Choose **Load Latest** beside the intended show.

Folders appear before `.show` files. Each show row includes its last-saved time and an ellipsis button. The ellipsis opens the revision table, with **Latest Autosave** first and named revisions below it. See [Named revisions and copies](17-revisions-and-copies.md) before loading an earlier version.

**Partial Load** opens the selected source's dependency and conflict preview without activating it. It is available for the latest autosave and named revisions.

## Save a copy

Open **Save Show As** to choose a name and destination. Its title-bar Source dropdown offers Internal, connected USB drives, and Network. USB drives appear directly in that list.

Click **Location** to expand the folder table. It lists the folders and the `.show` files already in the current folder, with the same location bar and **Up one folder** as Load Show. Open the destination folder, then collapse the table to keep its path visible above the name. Folder-read failures show an error in place of the folder list.

Choose **New Folder** to create a folder in the current location. ToskLight opens it and confirms **Created folder** below the table, so the next save lands inside it. New Folder is not offered on another desk's folders.

Saving to another folder creates an independent portable copy. The current show and named history remain intact.

To overwrite a show in the folder, choose its row or type its name. The row is highlighted, the name field warns that the show already exists, and the save action becomes **Replace Show**. ToskLight always asks before replacing: **Cancel** keeps the existing show, the folder, and the name you entered. A replaced show this desk already knows keeps its library name and identity, and a backup copy is stored with the desk's show backups first. The open show cannot be replaced this way, because it already autosaves.

For established internal shows, selecting an existing show as the destination opens a separate overwrite confirmation. Read [Named revisions and copies](17-revisions-and-copies.md) before confirming it. A failed save leaves an actionable status with the destination or reason.

**Save Named Revision** creates an immutable restore point; ordinary edits continue to autosave. Create a revision before a large repatch or programming change.

## Program offline and merge back

Two programmers can work on one show at the same time on different desks and bring the work
together afterwards.

1. In the Show menu, press **Take Offline Copy**, name the copy and choose **Internal** or a USB
   drive. The desk saves a named revision **Offline copy: <name>** of the open show and writes
   the copy next to your other shows.
2. Open and program the copy on another desk. Keep programming the original here if you like.
3. Back on this desk, with the original open, press **Merge Offline Copy** and choose the copy.

The merge compares the copy with the show as it was when the copy was taken:

- Changes made on only one side are taken over without questions. A Cue changed on both sides for
  different fixtures merges fixture by fixture.
- Where both sides changed the same value differently, the list shows both values. Choose
  **Keep Mine** or **Take Theirs** for each, or decide all at once.
- When both sides stored the same Group or Preset number, both are kept: the copy's moves to the
  next free number and everything in the copy that used it follows. Macros whose command text
  names a moved number are listed so you can check them.

**Merge** first saves a named revision **Before merging <copy>** and then applies every change in
one step, so **[UND]** takes the whole merge back. If the show changes after you opened the
preview, preview again. A show that is not an offline copy of the open show cannot be merged; use
**Partial Load** to copy objects from it instead.

## Load a rig from Architect

Choose **Network** in Load Show to find documents offered by Architect and other desks. Load the intended document to make it the active local show. An independently opened or saved `.show` copy stays independent.

Architect can bind a document to the active show on a Control desk. Rig edits then synchronize in both directions, while Cue programming, Programmer values, and playback operation stay on Control. Check Architect's sync status and resolve pending changes or conflicts before handing over the rig. See [Work with Control](../../10-ToskLight-Architect/07-control-and-transfer.md).

If discovery finds no document, open the saved portable `.show` file instead. Check that Architect has the intended show open and that both applications can communicate on the production network.

![Show menu and show-management actions](../../assets/screenshots/workflows/show-menu.png)

![Load the latest autosave or a named revision](../../assets/screenshots/workflows/show-load-revisions.png)
