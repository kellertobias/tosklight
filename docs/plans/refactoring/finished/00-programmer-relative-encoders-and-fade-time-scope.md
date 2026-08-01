# Programmer Relative Encoders and Fade-Time Scope

## Status and source contract

Finished. Implemented the complete behavior contract in
[`../../Done/00-programmer-relative-encoders-and-fade-time-scope.DONE.md`](../../Done/00-programmer-relative-encoders-and-fade-time-scope.DONE.md).
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

## Result

Implemented in the semantic-release commit
`feat(programmer): add relative immediate encoders`.

### Changes

- Replaced software encoder faders with touch encoder cards exposing `+10`, `+1`, `Set Value`,
  `-1`, and `-10`, plus wheel, Shift-wheel, keyboard-accessible buttons, and continuous
  displacement-rate drag.
- Added one typed, revisioned relative Programmer intent for ordered fixture selections and live
  Groups across WebSocket/HTTP, software controls, and attached hardware/OSC. The application
  resolves mixed values, Group spreads, activation policy, clamping, and continuous-drag undo
  grouping atomically.
- Made encoder, Set Value, and channel-fader writes immediate without storing an explicit
  zero-second timing override. Preset recall and Preload retain Programmer Fade.
- Added the persisted **AT uses Programmer Fade** setting, defaulting on for compatibility.
  Disabling it makes untimed command-line `AT` immediate while preserving `DELAY`; explicit
  `TIME` remains authoritative.
- Updated operator help, the CROSS-003 acceptance contract, semantic scenario catalogs, generated
  wire artifacts, and the affected bench helpers.

### Tests

- Focused Rust Programmer, application, engine, command-line timing, configuration, and generated
  wire-contract tests passed.
- Focused desktop component/writer tests passed, including touch zones, continuous undo identity,
  live Group relative intent, wire encoding, hardware events, and Channels immediacy.
- Playwright CROSS-003 passed with a five-second Programmer Fade across software touch, mixed API
  selection, attached hardware/OSC, and immediate DMX. Both semantic TIME-002 fixture and Group
  scenarios passed at zero and intermediate virtual times.
- `npm run test:architecture`, semantic documentation checks, bench types/unit tests, and
  `npm run manual` passed.
- `npm run test:unit` passed: 2,012 desktop tests, 16 hardware-control tests, the complete Rust
  workspace, generated-contract checks, and doc tests.
- `npm run open` built and launched both Tauri apps. The real desktop service reported ready with
  the active show loaded and no recovery mode.

### Limitations

- Indexed/discrete attributes are deliberately shown as constrained because the current fixture
  attribute contract does not expose a safe universal next/previous or wrapping policy.
- Scalar steps use normalized display-percent units; future profile-specific physical units can
  extend the server policy without changing the typed relative transport.
