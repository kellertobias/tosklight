# File Manager and Text Editor

These scenarios cover the confined file service and the two operator panes built on it. They use a fresh per-test server and data directory plus unique temporary names, then delete every created file and folder. They do not mutate show programming, so they do not create a canonical-show working copy merely to satisfy the show-scenario template.

The foundation cases are deliberately surface-specific: `FILE-001` verifies the authenticated service boundary, while `FILE-002` and `TEXT-001` verify the corresponding production panes. `FILE-016` and `TEXT-015` retain the completed feature-level regression suites and state their real API/UI coverage below.

## FILE-001 — Default file root is confined and revision safe

**Priority:** P1

**Primary layer:** Playwright `@api`

**Starting fixture:** Use the test's isolated data directory and authenticated session. Create one uniquely named `.txt` file under the built-in `Shows` root.

1. Read the root list and confirm a root with ID `shows` and label `Shows` exists.
2. Create the text file through the confined operation endpoint and read its initial revision.
3. Save the UTF-8 text `Preset check\nStandby cue 12\n` with that revision and read it back.
4. Repeat the save with the now-stale original revision.
   - **Expect:** The service rejects it with HTTP 409 and reports that the file changed since it was opened.
5. Request a directory containing `../` through the confined entries endpoint.
   - **Expect:** The service rejects it with HTTP 400 and reports that parent traversal is not allowed.
6. Delete the temporary file.

**Assertions:** The built-in root is addressable by opaque root ID, valid text round-trips exactly, optimistic revisions prevent stale overwrite, and a relative path cannot escape the root.

**Pass condition:** The default file service supports revision-safe operator text without exposing unrestricted filesystem traversal.

## FILE-002 — File Manager opens and edits a text file

**Priority:** P1

**Primary layer:** Playwright `@ui`

**Starting fixture:** Create one uniquely named `.md` file under `Shows` with the text `House open`, then open a fresh Desktop and add the production **File Manager** pane.

1. Confirm the pane presents its three-column workflow, including **Locations**, directory contents, and **Properties**.
2. Find the named file and double-click it.
   - **Expect:** The pane opens its text editor for that file.
3. Replace the contents with `House open\nBeginners` and click **Save**.
4. Read the file through the authenticated file API and confirm it contains `Beginners`.
5. Delete the temporary file.

**Assertions:** The real pane discovers the confined file, opens it through the visible row interaction, saves through its editor, and changes the same authoritative file observed by the API.

**Pass condition:** The three-column File Manager provides a complete visible browse-open-edit-save path for supported text files.

## TEXT-001 — Text Editor persists its file association and dirty state

**Priority:** P1

**Primary layer:** Playwright `@ui`

**Starting fixture:** Create one uniquely named `.txt` file under `Shows` containing `Cue 1`, then open a fresh Desktop and add the production **Text Editor** pane.

1. Click **Choose File…** and choose the named file from the confined file list.
   - **Expect:** The editor displays `Cue 1`.
2. Change the text to `Cue 1\nCheck follow spot`.
   - **Expect:** The editor reports **Unsaved** without discarding the selected file association.
3. Click **Save**.
   - **Expect:** The editor reports **Saved**, and the file API returns text containing `follow spot`.
4. Delete the temporary file.

**Assertions:** Choosing a file associates it with the pane, editing produces a visible dirty state, and saving writes the associated confined file.

**Pass condition:** A dedicated Text Editor makes its file identity and save state explicit and persists the operator's text.

## FILE-016 — Complete confined File Manager contract

**Priority:** P1

**Primary layers:** One Playwright `@api` contract case and four production `@ui` cases

**Starting fixture:** Use unique temporary folders/files under the test's `Shows` root. Cases that validate a configured root create it inside the test data directory. Remove all temporary roots and contents after the case.

### Authenticated service and input ownership

1. Request roots without authentication, then with a valid bearer token.
   - **Expect:** The first request is 401. The authenticated result includes the non-removable `Shows` root, exposes capabilities, and omits the server path.
2. Create visible and hidden files. Confirm ordinary listing hides the dotfile and `hidden=true` reveals it; parent traversal remains rejected.
3. Write `0123456789`, request `bytes=2-5` and `bytes=-3`, and confirm HTTP 206, correct range headers, and exact `2345`/`789` bodies.
4. Read metadata. When native notes are supported, save/read one and confirm no sidecar note file appears in the listing.
5. Copy a file twice into one destination. Confirm an unresolved conflict returns 409 and **Keep Both** creates `range copy.txt`.
6. Put `COPY` on the desk command line, let one File Manager instance claim the input context, and confirm that claim clears the command line and rejects a competing pane claim.
7. Send OSC Enter and Escape to the subscribed desk. Confirm Enter leaves the File Manager claim authoritative and Escape releases it without leaking the key into the programmer command line.

### Visible File Manager state machine

1. Add and enlarge the File Manager pane. Confirm header actions **Edit**, **Create**, and **View**, the three columns, ordered directory rows, breadcrumb, and readable header path.
2. Select a WAV file and confirm its audio preview uses an authenticated stream URL. Fetch its first four bytes with a Range request and confirm the returned RIFF header.
3. Enable **Show Hidden** through Pane Settings and confirm a hidden file becomes visible. When native notes are available, save a note from Properties; otherwise confirm the control is disabled.
4. Copy a file into a destination, repeat the copy, and choose **Keep Both** in the visible conflict dialog.
5. Rename the copy. Attempt Delete, cancel once, then confirm the second deletion through the platform-specific Trash/permanent-deletion wording.
6. Switch to Grid view, then use Back and Forward. Confirm the pane returns to the same destination and breadcrumb state.

### Configured roots and hosted picker

1. In **Setup → File Manager**, confirm the built-in `Shows` root, add an **Operator Files** configured root, change its label, save, and open the File Manager workspace.
   - **Expect:** The configured root is visible by label, its absolute server path is not visible in the workspace, and no legacy left-dock File Manager shortcut appears.
2. Remove the configured root and save. Confirm `Shows` remains available.
3. Open the hosted picker for a single `.txt` file in a specified initial directory. A `.png` selection cannot be confirmed; a `.txt` selection can be submitted with Enter.
4. Open a folder-only picker and cancel with Escape. Open a multi-select either picker and return one folder plus one file with **Select**.
5. Confirm operator file fields open the confined picker first. With system fallback disabled there is no fallback action. Enable it in Setup, reopen the wallpaper field, and confirm the fallback input accepts only `.png,.jpg,.jpeg,.gif,.webp`, is single-file, and is not a directory picker.

**Assertions:** Authentication, opaque roots, confinement, range streaming, native capability reporting, conflicts, desk input ownership, pane state, configured locations, picker target/cardinality/filter behavior, keyboard confirmation/cancel, and constrained fallback agree with the executable cases in `tests/16-file-manager.spec.ts`.

**Pass condition:** File Manager presents one confined, authenticated, conflict-aware file model across API, pane, Setup, OSC-owned command input, streaming preview, and hosted picker surfaces without exposing unrestricted server paths.

## TEXT-015 — Complete dedicated Text Editor contract

**Priority:** P1

**Primary layer:** Two production Playwright `@ui` cases with the authenticated API as persistence/event oracle

**Starting fixture:** Create a unique Markdown file under `Shows`, open a fresh Desktop, and add the Text Editor pane or panes named by the subcase.

### Concurrent editors, conflict handling, and recovery

1. Open the same file in two Text Editor panes. Save a clean edit in the first and confirm the second updates to the saved contents and remains **Saved**.
2. Make independent drafts in both panes and save the first.
   - **Expect:** The second reports **Conflict**, retains its draft, and **Compare versions** shows the unsaved and newer file versions separately.
3. Confirm **Reload Newer Version** replaces the draft only after confirmation and returns to the authoritative file text.
4. Perform an external API write. Clean panes update automatically; a pane with an unsaved draft retains it and reports **Conflict**.
5. Rename the file through the file operation API. Confirm both panes update their associated filename while an unsaved draft remains intact, then save that draft under the renamed path.
6. Read the persisted user layout and confirm both Text Editor panes store root `shows` and the renamed file path.
7. Delete the file. Confirm the pane reports **Missing** while retaining text, then use **Recreate File** and prove the authoritative file returns with that retained content.

### Confined Open File and pane modes

1. Open **Open File** and confirm it uses the confined picker with no system-fallback action; select the prepared Markdown file.
2. In Pane Settings, enable **Read-only pane** and choose **Rendered Markdown**.
   - **Expect:** The textarea disappears, the Markdown heading/list render visibly, Save and Save As are disabled, and the read-only notice appears.
3. Confirm the persisted pane configuration records read-only mode and Markdown view.
4. Disable read-only and choose **Edit + Markdown**. Edit the heading and confirm the preview changes before saving.
5. Choose **Plain Text** and confirm the editor remains while the rendered article disappears.

**Assertions:** Clean synchronization, dirty conflict preservation, version comparison/reload, external updates, rename association, layout persistence, missing-file recovery, confined selection, read-only enforcement, and all three view modes match `tests/15-text-editor.spec.ts`.

**Pass condition:** Multiple dedicated editors remain consistent without losing unsaved work, preserve their portable pane association, recover deleted files deliberately, and render only the configured editable/read-only view.
