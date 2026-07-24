# 05c — OSC multi-target selection and Stage Shift order

## Outcome

Close two route-specific gaps discovered by the 28/05b browser proofs without advertising
unverified semantic helpers.

## Findings

- A subscribed OSC controller reliably selects semantic Fixtures when each press waits for the
  authoritative command-line revision. Waiting for any feedback packet was not a valid barrier
  because the server also broadcasts command-line feedback every 500 ms.
- Stage Shift-click uses visible Stage order, which can differ from Fixture-number order. A real
  click plus Shift-click works with an established Stage anchor, but `range(1, 5)` cannot honestly
  promise the numeric Fixture range without an explicit ordering contract.

## Scope

- Diagnose OSC multi-target command behavior against the existing cross-surface OSC helper and
  command-line feedback after every phase; add an end-to-end ordered oracle before enabling the
  public helpers.
- Decide and document whether semantic Stage Shift-click range follows visible order or numeric
  Fixture order. If visible order remains authoritative, expose a typed visible-order result
  instead of treating numeric endpoints as an inclusive numeric range.
- Keep unsupported public combinations failing before their first mutation.

## Verification

- Focused public route scenarios for OSC items/ranges and Stage click/Shift-click.
- Full architecture and Playwright regression gates.

## Result

- Enabled public OSC item, ordered-items, and range selection after replacing periodic-feedback
  synchronization with an authoritative command-line revision barrier for every press.
- Required a pristine command line before OSC semantic selection because OSC Escape is a desk
  action rather than a revisioned Programmer edit; unsupported state fails before mutation.
- Kept numeric Stage Shift-click range unavailable and made the real anchored Shift-click return
  the observed visible-order selection plus its truthful gesture expression.
- Passed 26 focused adapter tests, TypeScript typechecking, architecture ratchets, and both focused
  Playwright scenarios. The full catalog reached 304 passed and 9 skipped with one unrelated
  Speed Group UI timing failure; its exact rerun passed.
