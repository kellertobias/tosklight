# 15a — Add the typed, replay-safe v2 show-library service

## Context

Chunk 15 combines a new persistence-sensitive service contract with a broad caller migration.
This first landing adds and proves the v2 boundary while the existing v1 routes coexist.

## Work

1. Add tolerant typed wire contracts in `crates/wire` for the show-library snapshot and
   create/open/default/rollback/rename/overwrite/revision-save/revision-open/MVR-apply intents.
2. Add v2 show-library routes, including unchanged GET blob routes for show and MVR downloads.
3. Add session-scoped request-id replay protection for every mutating intent. Reusing an id
   with a different request must conflict; exact retries return the original outcome.
4. Keep v1 routes intact for 15b and add focused route, tolerance, replay, and recovery tests.

## Definition of done

- Every show-library operation has a typed v2 route.
- Mutating intents are replay-safe and tolerant of unknown fields.
- Existing v1 behavior and persisted show compatibility remain unchanged.

## Verification

```sh
cargo test -p light-wire
cargo test -p light-server --no-default-features show_library_v2
cargo test -p light-server --no-default-features
npm run test:unit
```

## Decisions

Route names follow intent-shaped `/api/v2/shows/...` resources; no unresolved decision.

## Result

- Added tolerant generated v2 contracts for show-library snapshots, semantic lifecycle
  actions, revisions, MVR preview/apply, and binary exports.
- Added authenticated `/api/v2/shows` and `/api/v2/mvr` routes with session-scoped,
  signature-checked replay protection for every mutating action while retaining v1 for the
  caller migration in 15b.
- Preserved the existing show store and persistence behavior by adapting the established
  lifecycle operations behind the new typed boundary.
- Added focused coverage for tolerant create requests, exact replay, conflicting request-id
  reuse, snapshot projection, and revision replay.
- Verified with `cargo test -p light-wire` (82 passed plus the generated-contract check),
  `cargo test -p light-server --no-default-features show_library_v2` (2 passed),
  `cargo test -p light-server --no-default-features` (456 passed, 1 ignored), and
  `npm run test:unit` (including 277 frontend files and 2,001 frontend tests).
