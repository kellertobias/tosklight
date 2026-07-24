# 12a — Shared context headers and desk-context route de-scoping

## Context

The parent chunk `done/12-show-scope-optional-header.md` defines the complete route
de-scoping contract. This first child establishes the shared server boundary and
migrates routes where a desk id is context rather than an operand.

## Work

1. Define shared Axum extractors for optional `X-Tosk-Show` and `X-Tosk-Desk`
   headers. A supplied show must match the active Show; a supplied desk must exist.
   Without a desk header, resolve the single desk or the designated main desk
   deterministically. Document both headers in `docs/engineering/api-rules.md` §6.
2. Remove show/desk context segments from playback actions, cue deletion, command-line,
   and programming-selection HTTP routes. Keep request operands and typed outcomes
   unchanged.
3. Migrate their control UI transports, bench helpers, and root/server tests. Desk UI
   edits send both context headers; integrators may omit them.

## Definition of done

- Both extractors cover match, mismatch/unknown, and absent/default behavior.
- The four route families carry no show or desk context path segment.
- UI calls send the relevant guards and all retained HTTP behavior remains replay-safe.

## Verification

```sh
cargo test -p light-server
npm run test:unit
npm run test:e2e-api
npm run test:e2e
```

## Decisions

Inherited from parent chunk 12. No open decisions.
