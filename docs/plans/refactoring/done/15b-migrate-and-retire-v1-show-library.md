# 15b — Migrate the production show-library client

## Context

15a provides the typed, replay-safe v2 service while v1 coexists. The original 15b combined the
production migration with a broad acceptance/tool migration and route deletion. This landing
migrates the production client first; 15c owns exhaustive caller migration and retirement.

## Work

1. Migrate `ShowApiClient`, and therefore `features/showLifecycle`, Quick Setup, and recovery,
   to the typed v2 snapshot, actions, downloads, and MVR routes.
2. Adapt the generated v2 result union back to the existing UI domain types at one boundary.
3. Keep v1 routes temporarily registered for test/tool migration in 15c.

## Definition of done

- Every production show-browser, recovery, overwrite, revision, download, and MVR operation
  uses the v2 client contract.
- Focused client contract tests prove semantic action bodies and result decoding.
- Existing UI domain behavior remains unchanged.

## Verification

```sh
npm run test:unit
```

## Decisions

The exhaustive caller migration and v1 deletion are split into 15c. No open decisions.

## Result

- Migrated the production `ShowApiClient` to the v2 library snapshot, semantic action,
  download, MVR preview, MVR apply, and MVR export routes.
- Centralized request identity generation and decoding of the typed action-result union,
  preserving the UI's existing `ShowEntry`, revision, and MVR result interfaces.
- Converted the UI's keyed MVR conflict choices into the typed fixture-resolution action
  contract at the client boundary.
- Updated focused transport tests to assert v2 action discriminants, stable show identities,
  request ids, and typed outcomes.
- Verified the production tree contains no v1 show-lifecycle or MVR callers; the remaining v1
  show paths are the show-object/preload families intentionally owned by later chunks.
- Verified with `npm run test:unit` (including 277 frontend files and 2,001 frontend tests).
