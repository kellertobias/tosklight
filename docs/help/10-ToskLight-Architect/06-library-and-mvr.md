# Fixture library and MVR

Open **Settings > Library** to choose profiles or create a fixture revision. This library belongs to the computer; a show keeps the profile revisions embedded when its fixtures were patched.

The profile editor is shared with Control. See [Fixtures and patch](02-Fixtures-and-Patch/index.md) for library import, channel editing, geometry, and calibration. Architect's asset pickers read this computer's filesystem, so photographs, icons, and GLB models can be selected directly.

## Review an MVR before importing

**Import MVR** previews the exact embedded fixture files and modes. Native ToskLight profiles take
precedence; a broken embedded GDTF cannot silently use an installed fixture with the same name.
Read **Import limitations** before applying. Fixtures without usable profiles can be kept unresolved
for recovery or skipped; choosing a different DMX address cannot resolve missing profile data.

Conflicting immutable native profile revisions are listed with affected fixture counts. Import is
blocked by default. Choose **Import conflicting profiles as new identities** to copy the exact
archive profiles without overwriting existing profiles or changing unrelated fixtures. Imported
fixtures are bound to the copied identities. Mode,
channel, geometry and profile-authored calibration identities inside each copied profile remain consistent.
Per-fixture identity-bound corrections keep their original source identity and become inactive until
revalidated for the new profile identity; they are never silently rebound. Retained
GDTF bytes keep their original association evidence; export generates GDTF if that association no
longer matches the new identity. The source is never silently revalidated.

Apply uses the data captured by the preview, even if the picked file changes. Opening another
show or preparing another archive replaces that preview. If the destination patch changes, preview
again. An invalid address leaves the preview available so you can correct it. Reimporting the same
fixture does not conflict with its own existing address. The planning patch sheet continues to
show intentional address overlaps; resolve them before running the show on a control desk.

Source, additional DMX-break and unsupported-scenery warnings remain visible after a successful
import until **Done**. The portable show retains its imported profiles and source evidence without
requiring them to be installed in this machine's fixture library.

See [Shows, revisions, and MVR](../20-ToskLight-Control/10-Show-Setup/10-shows-revisions-and-mvr.md) for the portable-show and interchange reference.
