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
