# 06b — Hardware, discrete, and special encoders

## Outcome

Complete the encoder matrix after the normalized absolute/relative foundation landed in 06a.

## Scope

- Add Control and Media group entries whose values are typed discrete enums rather than numbers.
- Add hardware and OSC group/page navigation using live encoder feedback, including repeated group
  presses and attributes that are not in Encoder 1.
- Add relative detents, press, and press-turn only where the catalog declares those operations.
- Add set, release, clear, Position Return Home, alignment, Dynamics, and documented
  attribute-family special dialogs.
- Normalize Programmer assertions, including ordered spreads, discrete values, LTP ownership, and
  selection stepping.
- Keep scenario files free of labels, physical slots, group press counts, modal strings, and
  hardware transport details.

## Verification

- Compile-time group/attribute and operation-capability tests.
- Focused visible, hardware, OSC, and API helper-contract scenarios.
- TypeScript, architecture, and full Playwright regression gates.
