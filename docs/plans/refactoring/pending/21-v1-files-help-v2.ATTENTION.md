# 21 — Files + help routes: decide their home, then migrate or exempt

## Context (verified 2026-07-23)

Two self-contained v1 groups remain, both fully caller-mapped:

- **Files** (`crates/server/src/file_manager.rs:35-67`): roots, input-context (GET/POST/
  DELETE), entries, metadata, content, stream-ticket, thumbnail, notes (GET/PUT),
  text (GET/PUT), operations. Sole client: `api/client/files.ts` (via `features/files`,
  FileManagerWindow).
- **Help** (`crates/server/src/help.rs:51-53`): `/api/v1/help`, `/help/topics/{*id}`,
  `/help/assets/{*path}`. Clients: `api/client/help.ts` and
  `apps/control-ui/src/windows/helpMarkdown.ts` (assets).

Neither touches show state; both are read-heavy content/file surfaces. Mechanical
re-versioning to `/api/v2` is possible but low-value; the api-rules mostly bite on the
write paths (files notes/text PUT, operations POST → request identity; input-context).

## DECISION NEEDED (maintainer)

1. **Move both to v2** for a uniform surface (then delete the v1 registrations), or
2. **Exempt them** as internal content/file transports: record a dated exemption in
   `docs/engineering/api-rules.md` (they keep `/api/v1` until touched for other reasons),
   which lets chunk 22 declare "v1 retired" with a named exception list.

## Work (after decision)

- Option 1: re-register under v2; migrate `files.ts`, `help.ts`, `helpMarkdown.ts`;
  file-write endpoints (notes/text/operations) gain request ids per §3; delete v1.
- Option 2: add the exemption paragraph; bring only the write endpoints into §3 compliance
  in place (request identity on operations/notes/text) since "touching" them here anyway
  is cheap — or explicitly defer that too, noted in the exemption.

## Definition of done

- The decision is recorded; the chosen option implemented; FileManagerWindow and the help
  window (topics + inline assets/screenshots) verified working.

## Verification

```sh
npm run test:unit           # FileManagerWindow tests reference content route
npm run test:e2e   # full suite gate
npm run manual     # help asset paths feed the manual pipeline — confirm it still builds
```

## Decisions

**DECISION NEEDED** — option 1 or 2 above before starting.
