# 21 — Files + help routes: move to v2

## Context (verified 2026-07-23)

Two self-contained v1 groups remain, both fully caller-mapped:

- **Files** (`crates/server/src/file_manager.rs:35-67`): the former v1 roots,
  input-context (GET/POST/DELETE), entries, metadata, content, stream-ticket, thumbnail, notes (GET/PUT),
  text (GET/PUT), operations. Sole client: `api/client/files.ts` (via `features/files`,
  FileManagerWindow).
- **Help** (`crates/server/src/help.rs:51-53`): the former v1 catalog, topic, and asset
  routes. Clients: `api/client/help.ts` and
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
3. Delete the v1 registrations; grep apps/, tests/, and docs/ for the legacy file and
   help prefixes — note the FileManagerWindow unit
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

## Result

Claimed 2026-07-24. The recorded decision is resolved; no maintainer decision is open.

Completed 2026-07-24:

- Re-registered every File Manager and Help route under `/api/v2` and removed the
  production v1 registrations and callers, including desk-boundary exemptions for
  ticketed content and public Help assets.
- Added generated typed wire contracts and tolerant request extraction for File Manager
  mutations. Notes, text, operations, and input-context claims/releases now carry
  request identity and return replay-safe outcomes; request-ID reuse with a different
  payload conflicts before mutation.
- Migrated the production clients, help Markdown asset rewriting, E2E driver/raw callers,
  server tests, window tests, and the Help generator tour.

Verification:

- `cargo check -p light-server -p light-wire`
- `cargo test -p light-server file_input -- --nocapture`
- `cargo test -p light-server file_manager -- --nocapture`
- `cargo test -p light-wire`
- `npm run test:unit` — 277 frontend files / 2,008 tests plus the Rust workspace passed
- focused File Manager/Text Editor E2E — 10 passed
- `npm run test:e2e` — 286 passed, 10 intentionally skipped
- `npm run manual` — 140-page PDF and offline HTML verified
- whole-repository runtime/caller grep found no legacy file or help prefixes
