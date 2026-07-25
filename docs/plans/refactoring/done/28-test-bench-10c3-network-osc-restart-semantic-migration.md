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

## Result

- Added 24 semantic-world scenarios for DMX output, OSC and cross-surface
  behavior, virtual-time boundaries, restart durability, recovery, and revision
  copies. The public world now owns focused network-output and cross-surface
  adapters; current-page playback UI addressing also resolves the visible slot
  on the selected page.
- Retired all 26 ordinary paired `@ui` inventory rows. The corresponding API,
  raw OSC, wire-packet, restart-process, corruption, and supplemental touch
  boundaries remain in their low-level suites. DMX-008 remains the existing
  narrowly skipped API boundary because its minimum-universe contract is not
  implemented.
- Verification: 24/24 semantic cases passed in four-worker runs (8 output,
  10 OSC/cross-surface, 6 time/recovery); the retained boundary run produced
  52 passes and one intentional skip, with its sole OSC timing failure passing
  immediately in isolation. The control UI build, architecture and source-size
  gates, 309-case migration inventory, semantic documentation checks (8/8),
  and diff check passed.
- The first all-area semantic invocation reported all 24 passes but needed an
  interrupt after Playwright lingered during combined teardown; each of the
  three area suites then exited cleanly under the same four-worker parallelism.
