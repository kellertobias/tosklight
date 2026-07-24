# 17a — Move screen and control-desk configuration onto v2

## Context

Screens and control desks are persisted desk-store operands, but the control UI still reads and
writes them through whole-object v1 routes. Page assignment must preserve independent-screen,
follow-main, explicit-desk, and current-session semantics.

## Work

1. Add typed v2 screen snapshot and screen intent routes for create/update, delete, and independent
   page assignment. Use tolerant bodies and request identity for edits.
2. Add typed partial v2 control-desk update and page-assignment routes with request identity.
3. Migrate control-ui screen setup, desktop window persistence, and page selection callers.
4. Migrate feature-local tests and delete the corresponding v1 screen and control-desk routes.
5. Cover explicit desk/page selection and the current-session/default-desk behavior without changing
   current-page playback action addressing.

## Definition of done

- Screen and control-desk configuration callers use typed v2 routes exclusively.
- The v1 screen routes and v1 control-desk PUT/page routes are absent.
- Existing desk-store data loads unchanged and current-page versus explicit-page behavior is covered.

## Verification

```sh
cargo test -p light-server screens
cargo test -p light-server playback_topology_page
npm run test:unit
npm run test:e2e -- tests/04-osc-api-and-cross-surface.spec.ts
npm run test:e2e
```

## Decisions

Inherited from chunk 17. No open decisions.

## Result

Execution inventory separated screen objects from control desks: they use different persisted
schemas, authorization rules, event families, and page-authority semantics. The work was split into
17a1 for screen configuration and 17a2 for control-desk settings/page selection. No production
behavior changed in this planning split.
