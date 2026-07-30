# Minor Operator Polish

## Status

**FINISHED 2026-07-30.** Claimed with 66% large-window usage remaining.

## Items

### Command-line fixture history

Fixture history should fold up from the command line instead of appearing as an unrelated modal. The command line remains part of the history surface, so the operator sees the current entry in context while browsing previous entries. Command errors should also be shown in there.

### Missing fixture numbers and select-all range

Fixture selection must resolve against the fixtures that actually exist in the current show. A
positive fixture number that is not present is not an operator error and must never produce
`fixture number is invalid`; it simply contributes no fixture to the resolved selection.

- Selecting only a missing fixture completes successfully with no fixture selected.
- A range selects every existing fixture whose number falls inside the requested bounds and
  silently skips every missing number. For example, if only fixtures 10, 11, and 12 exist,
  `Fixture 1 Thru 999 Enter` selects exactly fixtures 10, 11, and 12.
- The operator must be able to enter a range whose bounds do not themselves identify existing
  fixtures. Missing endpoints and gaps must not prevent the existing fixtures inside the range
  from being selected.
- `Fixture Thru Enter` is the command-line shortcut for selecting every fixture that currently
  exists in the show.
- These rules concern absent fixture IDs. Syntactically malformed fixture numbers remain invalid,
  and an unpatched fixture still exists and therefore remains selectable.

#### Required tests

Add automated regression coverage at the command/runtime boundary and through the operator-facing
command-line path. The tests must prove that:

1. selecting one positive, nonexistent fixture succeeds, reports no `fixture number is invalid`
   error, and leaves an empty fixture selection;
2. with only fixtures 10, 11, and 12 present, `Fixture 1 Thru 999 Enter` succeeds and selects
   exactly those three fixtures in fixture-number order;
3. missing range endpoints and internal gaps are ignored while existing fixtures in the range are
   retained;
4. `Fixture Thru Enter` succeeds and selects every existing fixture, including unpatched fixtures;
5. genuinely malformed fixture-number syntax is still rejected, so the permissive missing-ID
   behavior does not weaken command validation.



### Clock seconds

Make the seconds in the clock slightly larger while preserving the existing clock layout and avoiding overlap in hardware-connected and software-only modes.


## Result

Implemented in `fix(operator): polish command selection and clock`.

- Command Line History now folds upward from the command-line field instead of
  appearing as a detached top-of-screen modal. The field remains visible in
  context, the surface stays above workspace panes, and current command errors
  open and can be acknowledged inside the same history surface.
- Positive missing fixture numbers now resolve to no fixture without weakening
  malformed-number validation. Missing endpoints and internal range gaps are
  skipped. `Fixture Thru Enter` resolves every existing fixture in
  fixture-number order, including unpatched fixtures.
- Clock seconds increased from 7 px to 9 px without changing the clock's outer
  dimensions.
- Focused verification passed: desktop type checking; 16 command-line component
  tests; Rust formatting and the missing-fixture parser regression; the exact
  10, 11, and 12 fixture operator scenario; folded history through reconnect
  and attached-hardware mode; and computed clock geometry in software-only and
  hardware-connected layouts. The full suites were deliberately not run, as
  required by this minor plan.

Commit but do not run full tests suites.
