# Programmer Relative Encoders and Fade-Time Scope

## Status and source contract

Pending. Implement the complete behavior contract in
[`../../Next/00-programmer-relative-encoders-and-fade-time-scope.md`](../../Next/00-programmer-relative-encoders-and-fade-time-scope.md).
That specification is authoritative; this queue file records the execution boundary.

Estimated effort: 1–2 Codex days.

## Required work

1. Resolve the specification's remaining decisions: per-attribute fine/coarse steps, continuous
   hold-drag rate curve, undo grouping, clamping/wrapping, and configurable command-line `AT`
   timing.
2. Add one typed relative-programmer operation used by software encoders, physical/OSC encoders,
   keyboard/wheel paths, and hardware-connected UI.
3. Replace the software encoder's fader-like absolute gesture with the vertical
   `+10`, `+1`, `Set Value`, `-1`, `-10` touch zones and continuous displacement-rate drag.
4. Keep explicit Set Value as the absolute-entry path.
5. Ensure encoder-originated changes and channel faders bypass Programmer Fade without storing an
   explicit zero-second Cue timing override.
6. Preserve Programmer Fade for the specified Preset recall and Preload Go paths.
7. Update command/API, OSC, hardware feedback, operator help, testing Markdown, and executable
   coverage together.

## Acceptance and verification

- Use a non-zero Programmer Fade and assert immediate authoritative engine/output state, not only
  the encoder label.
- Cover mixed selection, unpatched fixtures, released values, indexed attributes, fine/coarse
  steps, hold-drag, Set Value, undo grouping, software, hardware, OSC, and keyboard paths.
- Prove recorded Cue timing still uses normal fallback.
- Run focused engine/programmer tests, frontend component tests, API/UI Playwright scenarios,
  `npm run test:unit`, and the real desktop path.

## Non-goals

Do not redesign playback/channel faders, weaken hardware/software mode boundaries, or implement
unrelated Programmer features.
