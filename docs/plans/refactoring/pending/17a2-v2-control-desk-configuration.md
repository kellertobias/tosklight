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
