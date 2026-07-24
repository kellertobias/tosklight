# 17a1 — Move screen configuration onto v2

## Context

Optional operator screens are persisted desk-store objects. Their snapshot, whole-object PUT/DELETE,
and independent-page assignment still use v1 routes.

## Work

1. Add a typed v2 screen snapshot and replay-safe screen create/update, delete, and independent-page
   intents with tolerant bodies.
2. Preserve sparse updates by merging typed patches into the stored screen object; creation remains
   an explicit full configuration intent.
3. Migrate control-ui screen setup, desktop window persistence, and independent page callers.
4. Delete the v1 screen routes and add typed behavior plus absence coverage.
5. Verify existing screen rows load unchanged and follow-main screens cannot acquire independent
   page state.

## Definition of done

- All screen configuration callers use typed v2 routes.
- The v1 screen snapshot, PUT/DELETE, and page routes are absent.
- Request replay, unknown-field tolerance, stored-row compatibility, and independent-page validation
  are covered.

## Verification

```sh
cargo test -p light-server screen_configuration
npm run test:unit
npm run test:e2e
```

## Decisions

Inherited from chunk 17. No open decisions.
