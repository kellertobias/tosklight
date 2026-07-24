# 11a — Command line and selection desk writes onto typed WebSocket actions

## Context

The authoritative `ProgrammingInteractionView` writers still use HTTP for command-line
replacement and semantic selection actions. Legacy WS commands exist, but their loose
payloads, direct registry mutation, missing exact revision checks, and compatibility
responses are not equivalent to the typed application-service routes.

## Work

1. Add correlated typed WS actions for command-line replacement and the complete
   selection action union, reusing the HTTP wire adapters and `ProgrammingService`.
2. Move the authoritative desk writers to those frames while retaining HTTP for
   integrators and retained v1 WS compatibility.
3. Remove duplicate desk-UI callers of the loose command-line/selection WS helpers where
   they are no longer production dependencies.
4. Never re-send. On any outcome-ambiguous command-line or selection WS failure, re-read
   the narrow programming-interaction snapshot, settle/rollback from authority, and
   surface the error.

## Definition of done

- Command-line replacement and replace/gesture/group/rule selection actions use one
  typed correlated WS path from the desk UI.
- Exact revisions, request identity, typed outcomes, replay behavior, and desk/session
  ownership match the retained HTTP routes.
- Transport failure causes one repair read and no second mutation frame.

## Verification

```sh
cargo test -p light-server
npm run test:unit
npm run test:e2e
```

## Decisions

None. Execute after parent chunk 11.

## Result

Completed on 2026-07-24.

- Added correlated `programmer.command_line.replace` and
  `programmer.selection.action` frames that reuse the typed programming service,
  HTTP validation, wire conversion, exact request identity, revision checks, and
  replay behavior.
- Moved the authoritative control-UI command-line and semantic selection writers
  to those WebSocket actions while retaining HTTP and v1 compatibility surfaces.
- Removed selection mutation retries and made ambiguous command-line or selection
  failures perform one authoritative programming-interaction repair without
  resending the mutation.
- Kept the loose compatibility helpers until the v1 facade retirement chunk; they
  are no longer used by the authoritative programming-interaction writers.
- Verified with focused Rust and Vitest coverage, generated-contract checks,
  TypeScript typechecking, the architecture ratchet, `cargo test -p light-server`
  (438 passed, 1 ignored), `npm run test:unit` (276 files and 1,997 Vitest tests),
  and `npm run test:e2e` (285 passed, 11 skipped).
