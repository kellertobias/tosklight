# 10c1b1 — Dimmer and command semantic migration

## Outcome

Migrate the ordinary `DIM-001`, `DIM-002`, and `CMD-001` UI halves from the
foundational paired registrar.

## Done gate

- API halves keep their shared arrangement and authoritative assertions.
- Ordered Group edit commands, exact fade/output boundaries, and Fixture/Group
  default-mode toggling use public semantic intent.
- Focused API/UI cases, architecture, inventory, and parallel stress pass.

## Result

Completed on 2026-07-25.

- Kept the three paired API halves in the foundational registrar and disabled
  only their legacy UI registrations.
- Added enforced semantic-world UI cases for ordered Group command edits, exact
  2.999/3.000-second Dimmer output boundaries, and visible Fixture/Group
  default-mode toggling with scoped explicit prefixes.
- The inventory remains at 308 total cases; these three UI rows are now migrated,
  leaving 106 pending rows.

Verification:

- Control UI typecheck, architecture, inventory, and diff checks: passed;
- paired API and semantic UI representatives: 6 passed using four workers;
- semantic cases plus five-bench isolation stress: 5 passed using four workers.
