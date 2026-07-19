# Selective Show Import

Selective Show Import is the application-owned path for copying portable objects from another
show into the active show. It is intentionally separate from generic show-object PUT operations:
the service plans dependencies and reference rewrites across the whole selection, validates the
compiled candidate, and commits the result as one active-show transaction.

## Operator workflow

Open **Show → Load → Partial Show Load**. Choose a source show, select portable objects, and request
a preview. The preview explicitly lists:

- objects that will be imported, replaced, duplicated, kept, or skipped as identical;
- selected, automatically included, destination-bound, and missing dependencies;
- object conflicts and their operator-selected resolution;
- immutable fixture-profile revisions and profile conflicts;
- managed assets and their copy, identical, missing, or conflicting state; and
- every problem that blocks confirmation.

Changing the selection or a conflict resolution makes the previous preview stale. The operator
must update it before **Apply as One Show Revision** becomes available. Source reading and preview
requests can be cancelled. Once the atomic apply starts, the modal remains open until it succeeds
or fails because cancelling only the HTTP client would not cancel a blocking persistence commit.
A revision conflict requires a new preview; it is never papered over with a blind retry.

## Version 2 HTTP contract

All routes require an authenticated desk session:

```text
GET  /api/v2/shows/{target}/selective-imports/{source}/catalog
POST /api/v2/shows/{target}/selective-imports/{source}/preview
POST /api/v2/shows/{target}/selective-imports/{source}/apply
```

The catalog returns the exact portable source revision and lightweight object labels. Preview is
side-effect free for the active show and returns both source and target portable-show revisions.
Apply must repeat both revisions and send the target revision in `If-Match`. The server rejects the
operation if either show changed after preview. A successful mutation creates one safety backup,
one portable-show revision, one compiled runtime installation, and one typed
`selective_import_applied` event. Identical selections are valid no-ops and emit no event.

The checked-in Rust DTOs, TypeScript declarations, and JSON Schemas live under `light-wire`. The
frontend transport does not accept raw bodies for mutation: it can only send the typed selection,
explicit resolutions, preview revisions, and request correlation identity.

## Compatibility and extension rules

- Raw object bodies and unknown fields remain lossless while known identities and references are
  rewritten by typed descriptors.
- Legacy inline fixture snapshots are canonicalized into show-level profile revisions by the
  existing portable-show migration and travel with the selected objects.
- Existing whole-show Load, Save As, named revisions, MVR import, and old show files are unchanged.
- Unknown object kinds remain visible in the source catalog, but preview blocks them until their
  capability supplies a deterministic descriptor. It must not guess from field names.
- The current server has no external managed-asset store. Any descriptor that names a managed
  asset therefore produces an explicit missing-asset blocker. Adding a real store requires exact
  revision inspection, reversible preparation, compensation before show commit, and an infallible
  publication receipt after commit.
- A new portable capability joins this workflow by registering its object descriptor and, when
  needed, its profile-asset descriptor. It must not add a feature-specific copy endpoint.

The frontend currently performs one compatibility refresh after a changed import for older
capabilities that do not yet own focused event stores. Group and Preset changes already reconcile
through the scoped Selective Import event. Remove the compatibility refresh capability by
capability as Patch, Playback configuration, layouts, and future objects gain narrow stores.
Selective Import commands themselves are exposed only through the feature-owned
`SelectiveImportProvider`; they are not added to the transitional global server context.
