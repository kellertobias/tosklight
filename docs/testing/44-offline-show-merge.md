# Offline Show Merge

## Purpose

Two programmers can take copies of one show, program apart, and bring the work back. The desk
compares three versions — the show when the copy was taken, the show now, and the copy — so work
done on only one side merges without questions, and the operator decides only where both sides
changed the same thing.

## MERGE-001 — Taking an offline copy records where it started

From the Show menu the operator presses **Take Offline Copy**, enters a name and a destination
(Internal or a USB drive) and confirms.

- The desk saves a named revision **Offline copy: <name>** of the open show, then the copy.
- Every Group and Preset in the open show and the copy carries the same permanent identity.
- The copy remembers its source show and the objects it started from. Save As of the copy keeps
  that; Save As of an ordinary show does not make it mergeable.

## MERGE-002 — Separate edits merge without conflicts

Given the copy changed Cue 2 by adding Fixture 3 and the desk changed Cue 1, and each side edited
different objects, **Merge Offline Copy** shows a summary by kind and no conflicts for these
edits. A Cue changed on both sides for different fixtures merges fixture by fixture.

## MERGE-003 — The same value changed on both sides is the operator's choice

Given both sides changed Fixture 1's intensity in Cue 1 to different values, the preview lists one
conflict naming the Cue and the value, with **Keep Mine** and **Take Theirs** showing both values.
**Merge** stays disabled as **Decide 1 Conflict** until every conflict has a choice; **Keep All
Mine** and **Take All Theirs** decide all at once.

## MERGE-004 — The same Group or Preset number created on both sides keeps both

Given the desk and the copy each stored a new Group 858, the preview says the copy's Group 858
becomes Group 859 and this show keeps its own 858. After merging both Groups exist and every Cue,
Preset or Playback the copy pointed at its Group 858 now points at 859. Macros whose command text
names a moved number are listed for checking; their text is not rewritten.

## MERGE-005 — Deleting what the other side changed is a conflict

Given the copy deleted an object the desk changed (or the reverse), the preview lists it as a
whole-object conflict; Keep Mine keeps the object, Take Theirs deletes it.

## MERGE-006 — Patch changes on both sides merge per field

Given the desk re-addressed Fixture 1 and the copy renamed it and patched a new fixture, both
changes and the new fixture arrive; the same field changed differently is a conflict.

## MERGE-007 — The merge is safe

- Pressing **Merge** first saves a named revision **Before merging <copy>**, then writes every
  change in one step. The result names that revision.
- If the show changed after the preview, the merge is refused and the operator previews again.
- **[UND]** reverts the whole merge in one step.
- Choosing a show that is not an offline copy of the open show is refused with a message; an
  ordinary show points to **Partial Load** instead.
