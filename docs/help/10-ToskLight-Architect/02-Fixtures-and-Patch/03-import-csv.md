# Import a CSV patch list

Open the top-right **⚙ Settings** in Show Patch and press **Import CSV** in the Settings title to add a whole fixture list from a spreadsheet, paperwork export, or another desk. Every CSV row becomes one fixture. The import only adds fixtures; it never changes or removes fixtures already in the show. From **Media Servers** or **Tracking**, the same action switches to **Fixtures** first. In a Show Patch pane, **Import CSV** sits in the title of that pane's **Pane Settings**.

1. **Columns.** Choose the file. Comma, semicolon, and tab separated files are detected automatically, and quoted cells may contain the separator. Choose whether the first row holds **Column names** or **Fixture data**, and whether X / Y / Z are in **Metres** or **Millimetres**. Above every column, assign **Patch**, **Fixture ID**, **Fixture Name**, **Fixture Type**, **Manufacturer**, **Mode**, **X**, **Y**, **Z**, **RotX**, **RotY**, **RotZ**, or **Ignore**. Recognised column names are suggested; each field can be assigned to only one column, so choosing it again moves it.
2. **Fixture types.** Rows are grouped by manufacturer, fixture type, and mode. A group uses a library fixture directly when exactly one library mode has the same manufacturer, fixture name, and mode name, ignoring only letter case and repeated spaces. Without a Manufacturer column, the Fixture Type may contain both, such as `Martin MAC Aura`. Without a Mode column, only a fixture with a single mode matches. Every other group stops in the wizard: choose the manufacturer, fixture, and mode it should use, or **Skip these rows**. The wizard moves to the next unmatched group after each choice and preselects a likely fixture, which you still confirm. Exact matches can be changed here too.
3. **Review.** Check every row's fixture ID, name, library fixture, patch, location, and rotation, then press **Import**. All importable rows are added together in one Patch change on the layer currently selected in Show Patch (**Default** when All fixtures is shown).

Cell rules:

- **Patch** accepts `universe.address`, `universe/address`, or one absolute address, where `513` means `2.1`. An empty cell or `-` imports the fixture unpatched. Only the first split of a multi-split mode is patched from the CSV.
- **Fixture ID** must be a whole number that is not used in the show or earlier in the file. Venue profiles use `0.x`. An empty cell takes the lowest free ID.
- **Fixture Name** defaults to the fixture name followed by its ID.
- **X / Y / Z** and **RotX / RotY / RotZ** default to 0; rotation is in degrees. A decimal comma is accepted.
- **Address conflicts** with the show or an earlier row are imported unpatched by default; choose **Skip row** to leave them out instead.

Rows with an unusable cell are listed as **Not imported** with the reason and never block the other rows. Closing after choosing a file asks for confirmation.
