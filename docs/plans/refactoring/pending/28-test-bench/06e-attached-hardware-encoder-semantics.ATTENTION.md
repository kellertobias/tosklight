# 06e — Attached-hardware encoder control semantics

## DECISION NEEDED

The attached hardware surface sends `up`, `down`, `left`, `right`, and `press` for six encoders,
but Programmer controls currently consume only the four turn directions. The surface has no
attribute-family or encoder-page controls, and the operator help does not assign encoder press or
press-turn behavior to specific attributes.

Before implementation, decide:

1. which attached controls select and cycle Intensity, Color, Position, Beam, Shapers, Focus,
   Control, and Media pages;
2. which logical attributes support press and press-turn, and their exact operator semantics;
3. whether the browser test bench should drive those controls only through OSC, or also through a
   browser-hosted hardware-controls surface.

## Outcome

Once decided, implement the product controls and add live-feedback test-bench adapters without
hard-coded group press counts or encoder slots.

## Verification

- Hardware-controls component coverage for every new control and OSC path.
- Focused end-to-end scenarios proving repeated page navigation, non-first slots, press, and
  press-turn only on declared attributes.
- Software-only behavior remains unchanged.
- TypeScript, architecture, and full Playwright regression gates.
