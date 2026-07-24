# 08b — Independent overlapping Group Master HTP semantics

## Context

The refactor must preserve the desk meaning of a Group Master when stored Groups overlap.
A Group is a reusable selection, not a master merely because fixtures belong to it.
Assigning a Group to a Group Master playback opts that Group into master arbitration.

The required operator scenario starts with one stored Group containing six fixtures.
Create and store its two alternating selections using `[DIV] 2` and `[DIV] 2 [+] 1`,
then assign those odd and even Groups to separate Group Master playbacks. Select the
even Group, program its fixture intensities to 100%, and raise the even Group Master.
Those fixtures must produce output.

## Contract

- Group Masters are independent HTP intensity limiters.
- A Group with no Group Master playback assignment has no master contribution. Mere
  membership in the original six-fixture Group must not reduce or suppress output from
  either stored alternating Group.
- Overlapping assigned Group Masters are resolved by HTP: the highest applicable Group
  Master level wins for a fixture.
- Group Masters are never LTP, never lowest-takes-precedence, never multiplied or
  chained in membership order, and never masters of one another.
- The Grand Master is the only ordinary master above the resolved Group Master level
  and is applied exactly once.
- Programmer intensity remains Programmer-owned; moving a Group Master limits resolved
  output without rewriting the programmed 100% value.

## Work

1. Re-verify Group Master compilation and fixture-membership resolution in
   `crates/engine`, including direct, derived, frozen, and overlapping stored Groups.
2. Keep Group Master participation tied to an actual playback assignment rather than
   ordinary Group membership.
3. Add engine coverage for both disjoint odd/even selections and an overlapping
   original Group with a second assigned Group Master. Assert the highest applicable
   Group Master wins and the Grand Master scales that result once.
4. Add focused operator-level acceptance coverage that creates the six-fixture Group,
   stores `[DIV] 2` and `[DIV] 2 [+] 1`, assigns both as Group Master playbacks,
   programs one alternating selection to 100%, raises its master, and proves exact
   output.
5. Exercise the actual assignment and control surfaces used by the scenario; do not
   substitute direct implementation-object mutation for the acceptance path.

## Definition of done

- The odd/even six-fixture scenario produces output through the raised matching Group
  Master.
- The unassigned source Group has no limiting effect.
- When the source Group is also assigned as a Group Master, overlap resolves to the
  highest applicable master level; neither master becomes subordinate to the other.
- Lowering one overlapping Group Master cannot reduce a higher applicable Group Master.
- The Grand Master scales the HTP-resolved Group Master result exactly once.
- Exact engine and operator-level regression coverage prevents LTP, lowest-takes-
  precedence, serial multiplication, or implicit-membership behavior from returning.

## Verification

Run the smallest new engine test first, then the focused Group/Playback acceptance spec,
followed by the normal full-suite gate for a refactoring chunk. Verify final rendered
intensity/DMX values, not only stored Group membership or fader state.

## Decisions

None. These semantics are part of the refactoring acceptance contract.

## Result

- Group Master participation is compiled from actual Group-target playback assignments;
  stale legacy Group fader pointers and ordinary Group membership are ignored.
- Live Group Master values are preserved across same-show snapshot and topology
  replacements, while show activation continues to release the previous show's state.
- Six-fixture engine and paired API/UI coverage proves disjoint and overlapping Groups,
  highest-applicable HTP, one Grand Master application, unchanged Programmer ownership,
  and exact logical DMX, Art-Net, and sACN output.
- Verification passed: `cargo test -p light-engine`, `npm run test:unit`,
  `npm run test:e2e-api`, and all 296 full E2E cases (285 passed, 11 skipped). The full
  runner required interruption after every case completed because its teardown did not
  exit or print the summary on its own.
