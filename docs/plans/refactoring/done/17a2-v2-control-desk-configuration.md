# 17a2 — Move control-desk settings and page assignment onto v2

## Context

Control-desk settings and explicit desk-page assignment are persisted desk-store operands, but their
whole-object/page writes still use v1 routes and lack request identity.

## Work

1. Add typed partial v2 control-desk settings and page-assignment intents with tolerant bodies and
   request replay.
2. Preserve session authorization, session projection refresh, page auto-creation, existing-only
   selection, OSC feedback, and event ordering.
3. Migrate control-ui desk settings/page callers and root playback/OSC scenarios.
4. Delete the v1 control-desk PUT/page routes and add absence coverage.
5. Cover explicit desk/page selection and current-session/default-desk behavior without changing
   current-page playback action addressing.

## Definition of done

- Control-desk settings and page selection callers use typed v2 intents exclusively.
- The v1 control-desk PUT/page routes are absent.
- Existing desk rows load unchanged and explicit-page versus current-page behavior remains covered.

## Verification

```sh
cargo test -p light-server playback_topology_page
npm run test:unit
npm run test:e2e -- tests/04-osc-api-and-cross-surface.spec.ts
npm run test:e2e
```

## Decisions

Inherited from chunk 17. No open decisions.

## Result

- Added a typed, replay-safe v2 control-desk action endpoint with sparse settings updates and
  explicit existing-only or auto-creating page assignment.
- Preserved desk authorization, all matching session projections, persisted desk rows, page
  creation ordering, OSC feedback, and the distinction between explicit-page selection and
  current-page playback actions.
- Migrated the control UI and root OSC/virtual-playback scenarios, then removed the v1
  control-desk settings and page-assignment routes. The v1 page-playback live-action route remains
  intentionally for chunk 17c.
- Verified `cargo test -p light-wire` (83 unit plus generated-contract coverage), focused server
  tests (1 control-desk and 5 page-topology tests), focused client tests (9), `npm run test:unit`
  (2,004 frontend tests and all Rust workspace tests), focused E2E (53 passed / 2 skipped),
  `npm run test:e2e` (285 passed / 11 skipped), and `npm run test:architecture`.
- During final review, removed an unsupported layout-clear flag so the partial v2 update contract
  continues to match the legacy store's preserve-on-omission behavior.
