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
