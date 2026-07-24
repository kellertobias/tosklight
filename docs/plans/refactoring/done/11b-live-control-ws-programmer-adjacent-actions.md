# 11b — Priority, Preset, and Preload desk writes onto typed WebSocket actions

## Context

Programmer priority and Preset recall have loose compatibility WS commands, while their
authoritative desk writers still POST typed requests over HTTP. Preload lifecycle has
partial typed WS handling, but its desk boundary and Preload value mutations still use
HTTP or incomplete compatibility frames.

## Work

1. Give priority and Preset recall full correlated typed request/outcome WS frames with
   exact revision and replay parity.
2. Inventory and migrate all desk-owned Preload lifecycle and value mutations to typed
   correlated WS actions; keep snapshots/events and integrator HTTP routes.
3. Remove every client-side mutation re-send. Any ambiguous failure repairs all narrow
   authorities needed by the action before FIFO processing continues.

## Definition of done

- Priority, Preset recall, Preload lifecycle, and Preload values use typed WS mutations
  from the desk UI.
- HTTP remains available to integrators, and retained compatibility frames keep their
  established behavior.
- No writer retries a lost/failed mutation frame.

## Verification

```sh
cargo test -p light-server
npm run test:unit
npm run test:e2e
```

## Decisions

None. Execute after 11a.

## Result

- Added correlated typed WebSocket actions for Programmer priority, Preset recall,
  Preload lifecycle, and Preload values, including exact revision authority,
  idempotent replay outcomes, tolerant request decoding, and retained compatibility
  frames and HTTP routes.
- Kept priority and non-GO Preload lifecycle writes independent of the active-Show
  activation lock while Preset recall, Preload GO, and Preload value writes retain
  their required activation boundary.
- Routed all four desk-owned mutation families through the live client and removed
  client mutation resends. Ambiguous failures now repair the narrow authoritative
  snapshots before each writer continues.
- Verified with `cargo test -p light-server` (440 passed, 1 ignored),
  `npm run test:unit` (276 Vitest files / 1997 tests plus all Rust and contract
  gates), and `npm run test:e2e` (284 passed, 11 skipped; two unrelated,
  non-reproducing timing/UDP cases each passed immediately in isolation).
