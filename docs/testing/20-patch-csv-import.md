# Patch CSV Import

## Purpose

Prove that Show Patch adds a fixture list from a CSV file after the operator assigns its columns,
uses exact library matches directly, and resolves every other fixture type through the fixture-type
wizard before one atomic Patch change.

## Column assignment

1. Open **Show > Show Patch** and press **Import CSV** in the title bar, directly after
   **+ Add fixture**.
2. Choose a CSV whose header row names Patch, Fixture ID, Fixture Name, Manufacturer, Fixture Type,
   Mode, X, Y, Z, RotX, RotY, RotZ, plus one unrelated column. Confirm every named column is
   suggested and the unrelated column shows **Ignore**.
3. Assign **Patch** to a different column. Confirm the previous Patch column returns to **Ignore**
   and the assignment summary names the new column.
4. Repeat with a semicolon-separated file and a file without a header row. Confirm the delimiter is
   detected and **Fixture data** keeps the first row as a fixture.

## Exact matches and the fixture-type wizard

1. Include rows whose manufacturer, fixture type, and mode exactly match library modes (with
   different letter case), and rows whose type is unknown to the library.
2. Press **Next: fixture types**. Confirm exact matches are labelled **Exact match** and only the
   unknown groups are marked as needing a library fixture.
3. Choose a manufacturer, fixture, and mode for the first unmatched group and press
   **Use this fixture**. Confirm the wizard moves to the next unmatched group.
4. Press **Skip these rows** for another group. Confirm **Next: review** stays disabled until every
   group has a fixture or is skipped.
5. If every group matches exactly, confirm **Next: fixture types** goes straight to Review and
   **Back** still offers the fixture-type list for changes.

## Review and import

1. Include an invalid Fixture ID, a Fixture ID already in the show, a duplicated Fixture ID, an
   invalid Patch, a Patch that overlaps an existing fixture, and an empty Patch.
2. Confirm Review marks each invalid row **Not imported** with its reason, the overlap as
   **Unpatched**, and the empty Patch as an ordinary unpatched fixture.
3. Switch **Address conflicts** to **Skip row** and confirm the overlap is no longer imported.
4. Press **Import**. Confirm every importable row appears in Show Patch with its ID, name, library
   fixture and mode, patch, location in metres (or millimetres when selected), and rotation, on the
   selected layer, and that the existing fixtures are unchanged.
5. Confirm a server rejection leaves the dialog open with a visible error and adds no fixtures.
6. Choose a file, press Close, and confirm **Stay in Import CSV** keeps every choice.

## Persistence

1. Save, close, and reopen the show. Confirm the imported fixtures, positions, rotations, and
   patches are unchanged and unpatched imported fixtures remain selectable and programmable.
