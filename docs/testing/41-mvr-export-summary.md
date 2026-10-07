# MVR Export Summary

## Purpose

**Export MVR** in Save Show As writes the archive in one press. Afterwards the operator must see
what the archive actually carries and every limitation of it, so nothing is lost silently when the
rig moves to another application.

## MVR-EXPORT-001 — Summary and warnings after a one-press export

Given an open show with two patched fixtures whose profile has no retained source GDTF, the
operator opens **Save As**, enters a unique name and presses **Export MVR** once.

- The archive is written to the location the dialog shows; no preview step or second press is
  needed.
- The dialog then shows the server's summary of that export: **Exported MVR to** the location and
  file, **2 fixtures · 0 scenery objects**, and **Not included:** cues, presets, playbacks, users,
  and desk layouts.
- Every export warning the server reported is listed in full, with a count. Here the archive
  carries a generated GDTF, so the warning that ToskLight generated GDTF files from the current
  fixture profiles is shown.
- **Copy warnings** and **Dismiss** are touch-sized (at least 44 px high). The summary stays until
  the operator presses **Dismiss** or starts another save or export.

## MVR-EXPORT-002 — A failed export keeps an actionable error

Given an archive of the same name already exists in the selected folder, pressing **Export MVR**
again does not overwrite it. The dialog shows the server's error in an alert with **Copy error**,
shows no export summary and no stale progress text, and the existing archive stays unchanged.
