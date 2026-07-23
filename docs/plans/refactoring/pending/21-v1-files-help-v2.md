# 21 — Files + help routes: move to v2

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

## DECIDED (maintainer, 2026-07-23)

**Move everything to v2.** No exemptions — after this chunk (with 14 also decided as
v2), no served route outside the test-gated `with_test_routes` block carries `/api/v1`.

## Work

1. Re-register both groups under `/api/v2/…`; migrate `files.ts`, `help.ts`, and
   `windows/helpMarkdown.ts` (asset URLs) in the same chunk.
2. File-write endpoints (`notes`/`text` PUT, `operations` POST, `input-context`
   POST/DELETE) gain request identity per api-rules §3 while being touched.
3. Delete the v1 registrations; grep for stragglers (`rg -F '/api/v1/files'`,
   `rg -F '/api/v1/help'` across apps/, tests/, docs/) — note the FileManagerWindow unit
   test references the content route, and help asset paths feed the manual pipeline.

## Definition of done

- Files + help served only under v2; v1 forms deleted; FileManagerWindow and the help
  window (topics + inline assets/screenshots) verified working; `npm run manual` builds.

## Verification

```sh
npm run test:unit           # FileManagerWindow tests reference content route
npm run test:e2e   # full suite gate
npm run manual     # help asset paths feed the manual pipeline — confirm it still builds
```

## Decisions

Decided (2026-07-23): everything moves to v2. No open decisions remain in this chunk.
