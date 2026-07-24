# 05b — Visible and OSC selection routes

## Outcome

Complete the route matrix split from chunk 05 after its typed selection authority, command,
Highlight, and simulated-hardware seams landed.

## Scope

- Add truthful semantic selection adapters for Fixture Sheet, Stage, Group pool, keypad, touch,
  and OSC.
- Expose plain click and real Shift-click where the product assigns those gestures distinct
  behavior.
- Add seeded, replayable unqualified-route choice reporting; reject unsupported route/target
  combinations before mutation.
- Prove that every explicit route reaches the same ordered semantic oracle, including
  multi-head addressing, absent and intentionally empty Groups, live and dereferenced Group
  sources, wrapping selection steps, desk-alias isolation, and the shared OSC desk state.
- Keep scenario bodies free of complete command strings, locators, coordinates, and UUID
  lookups.

## Verification

- Focused adapter unit tests and public helper-contract Playwright scenarios.
- `npm --prefix apps/control-ui run typecheck`
- `npm run test:architecture`
- `npm run test:e2e`

## Result

- Added semantic Fixture Sheet, Stage, Group pool, touch, keypad, API, and subscribed OSC
  selection routes over the shared ordered selection oracle.
- Added seeded unqualified-route choice reports with deterministic replay and pre-mutation
  rejection for unsupported route/target combinations.
- Verified real click, Shift-click, double-click, and touch gestures without exposing locators,
  coordinates, UUIDs, or complete command strings to scenario bodies.
- Kept Stage numeric Shift-click ranges and OSC multi-target/range selection unavailable until
  their observed ordering and transport behavior are resolved in chunk 05c.
- Passed 25 focused adapter tests, three focused browser scenarios, TypeScript typechecking,
  architecture ratchets, and the full Playwright regression: 303 passed and 9 skipped.
