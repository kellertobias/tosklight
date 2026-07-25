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
