# 10c3 — Network, OSC, and restart semantic migration

## Outcome

Migrate the remaining ordinary UI halves of network output, OSC/cross-surface,
virtual-time, persistence, and recovery contracts.

## Scope

- `03-network-output-protocols.spec.ts`
- `04-osc-api-and-cross-surface.spec.ts`
- `05-virtual-time-persistence-and-recovery.spec.ts`

Wire packets, raw OSC subscription envelopes, restart process control, and
persisted-file corruption fixtures remain reviewed low-level boundaries.

## Done gate

- All 26 pending inventory rows in scope are migrated or narrowly justified.
- API/UI parity, OSC feedback, desk isolation, exact clock boundaries, show
  durability, and recovery behavior remain unchanged.
- Focused UI/API/OSC/wire/restart cases, architecture, inventory, and parallel
  stress pass.
