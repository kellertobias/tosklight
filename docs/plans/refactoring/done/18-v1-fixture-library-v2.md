# 18 — Fixture library/profiles onto v2; retire the v1 fixture routes

## Context (verified 2026-07-23)

`apps/control-ui/src/api/client/fixtures.ts` (consumed via `features/fixtureLibrary`)
drives all v1 fixture routes (`http_router.rs:42-80`): fixture-library GET/PUT + revision
delete, fixture-profiles GET/PUT, warnings, revisions, profile revision delete, package
download, package import, source-GDTF upload. `desktop-smoke.mjs` also reads profiles.

Cautions:

- The fixture-package contract is owned by `.agents/skills/build-light-fixtures/SKILL.md` —
  the `.toskfixture` format and validation semantics must not drift while re-homing routes.
- Library/profile saves are edits → request identity per api-rules §3.
- Package import/GDTF upload are long-running: keep visible progress + actionable error
  state (AGENTS.md UI rules) — don't silently change the upload flow.

## Work

1. v2 routes: snapshot reads (library, profiles, warnings, revisions), intent writes
   (save profile, delete revision, import package, attach GDTF). Blob GETs (package
   download) stay GET under v2 paths. Wire types typed+tolerant.
2. Migrate `features/fixtureLibrary` transports and the editor/revisions/transfers/warnings
   panes; migrate `desktop-smoke.mjs` and any bench/test callers.
3. Delete v1 fixture routes route-by-route with caller greps.

## Definition of done

- Fixture library UI (browse, edit, revisions, import/export, GDTF) fully on v2; v1
  fixture routes deleted; desktop smoke green.
- `.toskfixture` import/export byte-compatible (round-trip an existing library asset,
  e.g. `assets/fixture-library/generic--dimmer-fresnel.toskfixture`).

## Verification

```sh
cargo test -p fixture -p server
npm run test:unit
npm run test:e2e -- tests/<fixture library specs>
npm run test:e2e            # full suite gate — FIXTURE-002 @restart is known flaky; isolate
npm run test:desktop-smoke
```

## Decisions

None.

## Result

- Added typed v2 definition, profile, warning, and revision snapshots plus one
  replay-safe fixture-library action endpoint for profile/definition saves and
  deletes, package import, and GDTF attachment. Action outcomes retain only
  bounded server authority; package and GDTF bytes remain outside the replay
  cache.
- Migrated the control UI, fixture editor/revisions/transfers, desktop smoke,
  bench helpers, planned-demo setup, and root acceptance callers. Independent
  warning/profile snapshots keep malformed retained legacy definitions from
  suppressing recovery warnings or migrated profiles.
- Removed every v1 fixture-library/profile/package route and its dead adapter.
  The only remaining v1 route strings are deliberate server assertions that
  each retired route returns 404.
- Preserved the `.toskfixture` contract. All 47 shipped archives validate; the
  shipped Generic Dimmer Fresnel round-trips with identical serialized profile
  data, stable mode identities, icon, and GLB model. Server coverage also proves
  package import/export, exact GDTF bytes, stale-revision rejection, and replay.
- Verification passed: fixture suite (81), server suite (472 passed / 1 ignored
  plus 14 benchmark tests), `npm run test:unit` (277 Vitest files / 2,005 tests
  plus the Rust workspace), generated contracts, production UI build,
  architecture/source-size gates, focused fixture/recovery E2E, and the full
  E2E gate (285 passed / 11 skipped).
- One parallel focused rerun dropped the fixture icon in FIXTURE-002; that same
  scenario passed in the full gate and immediately in isolation. The packaged
  desktop bundle built, but `npm run test:desktop-smoke` could not complete in
  this GUI session: both process scenarios timed out at the shared
  frontend-ready marker while an existing user-owned ToskLight bundle remained
  running. No user process was terminated.
