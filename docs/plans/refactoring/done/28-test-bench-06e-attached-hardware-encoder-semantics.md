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

## Decision

Decided 2026-07-25:

1. attached hardware encoder 7 is the Programmer navigation encoder. Ordinary rotation emits
   `up`/`down`; held rotation emits `left`/`right`; clicking it performs the action represented by
   the current navigation cell;
2. encoders 1–6 use ordinary rotation for their primary function and held rotation for either the
   declared secondary function or, when no secondary function is declared, a coarse primary
   adjustment. Normalized primary adjustments use ±1% ordinarily and ±10% while held. Clicking an
   encoder performs the same cell action as activating that encoder cell in the browser;
3. cover both browser touch interaction with the hardware-controls surface and direct OSC input.

## Outcome

Once decided, implement the product controls and add live-feedback test-bench adapters without
hard-coded group press counts or encoder slots.

## Verification

- Hardware-controls component coverage for every new control and OSC path.
- Focused end-to-end scenarios proving repeated page navigation, non-first slots, press, and
  press-turn only on declared attributes.
- Software-only behavior remains unchanged.
- TypeScript, architecture, and full Playwright regression gates.

## Result

Completed 2026-07-25:

- routed desk-scoped OSC `nav` actions into the same attached-encoder event path and made encoder
  7 cycle and wrap all eight Programmer family cells with ordinary and held rotation;
- made encoder press invoke the same editable hardware cell action as browser activation, while
  discrete and unassigned cells remain mutation-free;
- retained normalized primary fine/coarse semantics at ±1% and ±10%, and verified the existing
  Stage declarations route held turns to their displayed secondary target instead;
- added accessible browser controls plus component and real browser-touch coverage for `up`,
  `down`, `left`, `right`, and `press` on both encoder and NAV OSC paths;
- added a live OSC/WS scenario covering repeated and wrapped navigation, a non-first encoder slot,
  held-turn adjustment, and press activation.

Verification:

- focused Vitest: **68 passed**;
- focused Playwright: **8 passed**;
- control-ui and hardware-controls TypeScript: passed;
- architecture, source-size, and command-boundary checks: passed;
- full Playwright regression: **316 passed / 9 skipped / 0 failed**.

No follow-up chunk was required.
