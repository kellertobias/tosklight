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

## Result

- Added generated screen configuration DTOs, an authenticated v2 snapshot, and one tolerant,
  replay-safe action endpoint for explicit create, sparse update, delete, and independent-page
  intents.
- Migrated the control client to cache v2 snapshots, distinguish create from update safely, send only
  changed fields, and use generated request contracts; removed all public v1 screen routes and
  handlers.
- Covered request replay/collision, future fields, sparse extension preservation, follow-main
  rejection, independent pages, existing desk-store rows, client create/update payloads, and v1
  route absence.
- Verified wire contracts (83 tests plus generated-artifact check), server unit coverage (466 passed,
  one ignored), frontend unit coverage (2,004 passed), architecture/formatting, and the full E2E
  suite (285 passed, 11 intentional skips). The first sandboxed unit attempt hit the known CITP UDP
  permission boundary; the complete escalated run passed.
