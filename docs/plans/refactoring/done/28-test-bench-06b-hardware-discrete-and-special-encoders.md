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

## Result

- Added a relative-only `.via.osc` encoder port and kept it type-distinct from absolute visible
  value entry and the API port.
- The adapter selects the requested visible family, resolves the logical attribute from the live
  attached-hardware display, sends fine OSC detents to its current Encoder 1–6 slot, and waits for
  every authoritative Programmer revision before continuing.
- Added a focused attached-hardware scenario proving Tilt resolves to its live non-first slot and
  reaches the expected 8-bit output after one detent.
- Passed 21 focused catalog/programmer/route-capability tests, TypeScript, the architecture and
  source-size gates, and all four focused encoder browser scenarios. The full Playwright run
  reached 308 passed / 9 skipped with only the known in-suite CMD-002 Speed Group UI synchronization
  flake; its exact isolated rerun passed.
- Auditing the requested remainder found that current Programmer controls deliberately ignore
  encoder `press`, expose no OSC/hardware family or page switch, and render discrete encoder values
  as read-only profile-derived feedback. Those capabilities cannot be represented truthfully by
  test helpers alone, so the product-and-helper work was split into 06d and the decision-gated 06e.
- Programmer Fade remains independently queued as 06c.
