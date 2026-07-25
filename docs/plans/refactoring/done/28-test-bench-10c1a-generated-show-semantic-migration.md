# 10c1a — Generated-show semantic migration

## Outcome

Migrate the ordinary `SHOW-000 @ui` Save As workflow to the semantic world while
preserving the serial generated-show contract.

## Done gate

- The API half and artifact-generation supplementals retain their low-level
  boundaries.
- The UI half uses public Show, selection, Group, and command intent.
- Canonical Compact Rig and Default Stage shows remain unchanged while their
  copies and named revision copy retain the expected mutations.
- Focused API/UI cases, architecture, inventory, and diff checks pass.

## Result

Completed on 2026-07-25.

- Added surface-selective paired registration so `SHOW-000 @api` retains its
  existing arrangement and authoritative assertion without registering the
  legacy UI body.
- Replaced that UI body with an enforced semantic-world root scenario using
  Show, Desktop, Fixture selection, and Group pool/keypad intent.
- Show actions now adopt the active Save As or named-revision copy, so subsequent
  semantic helpers observe and mutate the actual active show before a canonical
  working-copy reset.
- The generated inventory still contains 308 cases; `SHOW-000 @ui` is now
  `migrated-semantic-world`, leaving 109 pending rows.

Verification:

- Control UI typecheck, architecture, inventory, and diff checks: passed;
- focused semantic UI case: 1 passed;
- retained API, both supplementals, and migrated UI together: 4 passed using four
  workers.
