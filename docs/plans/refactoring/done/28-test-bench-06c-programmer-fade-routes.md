# 06c — Programmer Fade routes

## Outcome

Add the typed Programmer Fade helper and prove every truthful route against deterministic
application time.

## Scope

- Add `timing.programmerFade.set(duration)`, `.double()`, `.half()`, and `.off()`.
- Add explicit `.via.fader`, `.via.valueEntry`, hardware, OSC, and meaningful API routes.
- Make the fader route perform a real pointer/touch slide and the value-entry route use the visible
  **Set value** dialog; neither may substitute a direct range fill or API mutation.
- Add seeded, replayable unqualified route reporting.
- Wait for authoritative timing feedback before the next Programmer write.
- Prove subsequently written values reach exact deterministic fade boundaries and retain timing
  when recorded.

## Verification

- Focused fader, value-entry, hardware/OSC, and API helper-contract scenarios.
- Exact application-time boundary and recorded-timing assertions.
- TypeScript, architecture, and full Playwright regression gates.

## Result

- Added `timing.programmerFade` with typed durations, seeded route reports, explicit API,
  value-entry, pointer-fader, and attached-hardware OSC routes, plus Double, Half, Off, and a
  normalized millisecond observation.
- The value-entry route presses the production **Set value** action and its visible modal keypad.
  The fader route switches to the real Playback controls, drags the native vertical fader, and
  calibrates small thumb-inset differences through authoritative 0.1-second feedback without
  filling the range input.
- Every route waits for the shared configuration authority. The focused scenarios prove exact
  half-way and completed four-second output boundaries, matching UI routes, seeded replay
  reporting, and the attached OSC fader.
- Recorded per-value timing remains covered by the existing TIME-002 regression contract. A
  bench-native Cue-recording assertion belongs with the typed Cue helpers in step 08 rather than
  introducing an early raw-object inspection escape hatch here.
- Focused helper contracts, TypeScript, and all three 06c browser scenarios pass. The full
  Playwright regression completed with 311 passed and 9 skipped; its sole unrelated
  HIGHLIGHT-003 selection-convergence timeout passed immediately when rerun in isolation.
