# 10c4 — Files, lock, and operator-shell semantic migration

## Outcome

Migrate the remaining ordinary File Manager, Text Editor, desk-lock, and manual
operator-shell UI cases.

## Scope

- `08-file-manager-and-text-editor.spec.ts`
- `09-desk-lock.spec.ts`
- `15-text-editor.spec.ts`
- `16-file-manager.spec.ts`
- `19-manual-review-software-corrections.spec.ts`

## Done gate

- All 18 pending inventory rows in scope are migrated or narrowly justified.
- Root confinement, revision conflicts, hosted picker behavior, lock coverage,
  exact operator wording, and modal/pane placement remain unchanged.
- Focused UI/API/OSC cases, architecture, inventory, and parallel stress pass.

## Result

- Added nine semantic-world scenarios covering File Manager and Text Editor
  workflows, desk-lock behavior across screens, exact desk/desktop wording,
  fixture-browser alignment, confined operator file pickers, and the
  development-diagnostics boundary. Public browser-world adapters now own
  these workflows under the specific-feature and window-system bench areas.
- Retired all 18 ordinary UI inventory rows in scope. Root-confinement,
  revision-conflict, range/metadata, input-ownership, OSC lock, and raw
  API boundaries remain in their focused low-level suites.
- Kept three pre-existing skipped MANUAL-019 cases as narrowly supplemental
  boundaries because their Cues-pane, Help/DMX/Stage, and safe-blackout
  contracts are still explicitly unimplemented; they no longer count as
  ordinary migration rows.
- Verification: the focused semantic and retained-boundary run completed with
  16 passes and the three intentional skips under four-worker parallelism.
  The control UI build, architecture and source-size gates, 308-case migration
  inventory, semantic documentation checks (8/8), and diff check passed.
