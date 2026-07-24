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
