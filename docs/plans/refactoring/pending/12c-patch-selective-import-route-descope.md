# 12c — De-scope Show Patch and selective-import target routes

## Context

Depends on 12a's shared optional context extractors. Show Patch still scopes the active
Show in its path, while selective import legitimately addresses a source Show but also
incorrectly scopes its active target Show in the path.

## Work

1. Remove the active `{show_id}` scope from Show Patch snapshot/mutation routes and
   migrate UI, bench, and tests to the optional Show guard.
2. Keep selective import's source Show id as an operand, remove only the target Show
   scope, and guard the loaded target through `X-Tosk-Show`.
3. Run a final v2 route/caller audit against the parent chunk's complete definition of
   done and update stale route documentation or tests found by that audit.

## Definition of done

- Show Patch carries no Show path scope.
- Selective import retains only its source-Show operand.
- No v2 route carries `{show_id}` as scope, and no v2 route carries a desk segment where
  the desk is context.

## Verification

```sh
cargo test -p light-server
npm run test:unit
npm run test:e2e-api
npm run test:e2e
```

## Decisions

Inherited from parent chunk 12. No open decisions.
