# 10c1 — Show and foundational semantic migration

## Outcome

Migrate the remaining ordinary UI cases from the generated-show and foundational
Dimmer, command, Group, and Programmer families.

## Scope

- `00-generate-show-files.spec.ts`
- `01-foundational-dimmers-and-groups.spec.ts`

Keep generated-show entrypoints serial. Retain low-level API and supplemental
boundaries only where transport, exhaustive permutations, or artifact generation
is the acceptance contract.

## Done gate

- All 12 pending inventory rows in scope are migrated or narrowly justified.
- Generated show names and paths, ordered Group semantics, unpatched-fixture
  behavior, and Programmer LTP/relative behavior remain unchanged.
- Focused API/UI cases, architecture, inventory, and parallel stress pass.
