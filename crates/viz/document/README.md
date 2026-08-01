# viz-document

The planning document behind the Viz editor: one `.show` file, patched with the lighting desk's
own semantics, with none of the desk's runtime.

## Why it exists

The visualizer takes its live values from Art-Net or sACN sent by whatever desk is driving the
rig — frequently not ToskLight at all. So a standalone visualizer session does not need a desk; it
needs the rig: what fixtures exist, what they are, where they are, and how they are addressed.
Running a full `light-headless` to obtain that would pull in an output engine, playback,
programmer, OSC and sessions that nothing in this application uses.

## What it reuses

Everything that decides what a patch means:

- `ShowPatchService` for patching, repatching and removal, including validation, placement,
  splits and profile-revision resolution;
- `MvrImportService` for MVR import, and the shared `mvr_export` builder for export;
- `light-show` for the portable show file itself.

`PlanningPorts` implements `ShowPatchPorts` with exactly the persistence half: it opens the show,
resolves immutable profile revisions, commits the transaction, and installs nothing.
`prepare_runtime` still receives the compiled engine snapshot and drops it, so a patch that could
not compile is rejected here exactly as it is on a desk.

## The file

An ordinary portable show. The desk opens it directly through its show library — no conversion, no
import step, no second format. A saved document also retains the profile revisions it uses, so it
reopens on a machine without the fixture library that originally supplied them.

## Verified behavior

`cargo test -p viz-document` proves patching without a desk runtime, revision advance, the
save/reopen round trip, and MVR export.

Two behaviors are worth knowing because they are easy to assume otherwise:

- **Overlapping DMX addresses are accepted.** Double-patching is legitimate rigging, so this
  boundary stores it; surfacing the clash to the operator is the patch sheet's job, as on the desk.
- **A profile with no retained source GDTF is referenced in an export, not embedded**, and is
  reported in `MvrExportSummary::missing_profiles`.
