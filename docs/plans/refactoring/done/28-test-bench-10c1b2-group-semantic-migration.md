# 10c1b2 — Foundational Group semantic migration

## Outcome

Migrate the ordinary `GROUP-003`, `GROUP-004`, and `GROUP-005` UI halves.

## Done gate

- Derived ordering, frozen/dereferenced storage, unpatched-fixture programming,
  and stored-empty versus absent behavior remain exact.
- API halves and exhaustive supplemental boundaries remain unchanged.
- Focused API/UI cases, architecture, inventory, and parallel stress pass.

## Result

Completed on 2026-07-25.

- Kept the three paired API halves and disabled only their legacy UI
  registrations.
- Added enforced semantic UI cases for derived Group ordering, dereferenced
  frozen storage through source edits, unpatched-fixture programming, and stored
  empty versus absent Groups.
- Added a narrow public Patch helper for visible fixture unpatching and its
  authoritative unpatched expectation.
- The inventory remains at 308 cases; the three Group UI rows are migrated,
  leaving 103 pending rows.

Verification:

- Control UI typecheck, architecture, inventory, and diff checks: passed;
- paired API and semantic UI representatives: 6 passed using four workers;
- semantic cases plus five-bench isolation stress: 5 passed using four workers.
