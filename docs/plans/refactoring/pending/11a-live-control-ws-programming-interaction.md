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
