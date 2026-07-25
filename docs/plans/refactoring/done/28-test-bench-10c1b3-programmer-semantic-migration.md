# 10c1b3 — Foundational Programmer semantic migration

## Outcome

Migrate the ordinary `PROG-001` through `PROG-004` UI workflows, including both
`PROG-002` command/spread cases.

## Done gate

- Selection retention/replacement, live ordered spreads, multi-point anchors,
  LTP release, and two-stage Clear retain their exact vectors and ownership.
- API halves and exhaustive supplemental boundaries remain unchanged.
- Focused API/UI cases, architecture, inventory, and parallel stress pass.

## Result

- Added five semantic browser cases for selection retention and replacement,
  ordered live Group spreads, retained-selection and multi-point command
  spreads, fixture-over-Group LTP release, and the exact two-stage Clear path.
- Retained the four paired API cases and all supplemental coverage while
  removing only their superseded legacy UI registrations.
- The inventory remains at 308 cases across 44 root files. The five Programmer
  UI rows are migrated, leaving 98 pending rows.

Verification:

- Control UI typecheck, architecture, inventory, and diff checks: passed;
- paired API cases, semantic browser cases, and five-bench isolation stress:
  10 passed using four workers.
