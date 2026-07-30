# Minor Operator Polish

## Status

**DOING.** Claimed on 2026-07-30 with 66% large-window usage remaining.

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


Commit but do not run full tests suites.
