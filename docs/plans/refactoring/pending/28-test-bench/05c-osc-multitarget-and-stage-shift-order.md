# 05c — OSC multi-target selection and Stage Shift order

## Outcome

Close two route-specific gaps discovered by the 28/05b browser proofs without advertising
unverified semantic helpers.

## Findings

- A subscribed OSC controller reliably selects a single semantic Fixture when each press and
  release waits for command-line feedback. The compiled `6 TRU 8 ENT` sequence currently leaves
  only Fixture 8 selected in the isolated browser bench, so public OSC `items/range` helpers fail
  before mutation until the product or adapter cause is resolved.
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
