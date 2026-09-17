# Record and Update Options

## Purpose

Prove that pressing Record twice, or Update twice, asks how this Record or Update stores the
programmer. The choice applies once, or becomes the desk's stored default. Every plain Record and
Update then respects that default on every surface, and the operator can always return to Smart.

## RECUPD-001 — RECORD RECORD: one-off option, stored default, and reset to Smart

Given an open show and a Cuelist 101 with one Cue that stores Fixture 1 at 50%, pressing `[REC]`
arms Record, and pressing it again opens a modal titled **Record**. The modal offers **Smart**,
**Merge**, **Add Existing**, and **Add Cue**, has a **Set as default** toggle that is off, shows
**Current default: Smart**, and carries **Record** in its title bar. Record stays armed while the
modal is open.

- With Fixture 1 at 80% and Fixture 2 at 40% in the programmer, choosing **Add Existing** and
  pressing **Record** arms `RECORD ADD EXISTING`. Completing the target `CUELIST 101` on the
  keypad adds Fixture 2 to Cue 1 and leaves Fixture 1 at 50%. The stored default stays Smart.
- Choosing **Add Cue** with **Set as default** on stores Add Cue for the desk and arms a plain
  `RECORD`. `RECORD CUELIST 101` then adds a second Cue. `RECORD CUELIST 101 CUE 1` is refused
  and Cue 1 stays unchanged, because Add Cue never replaces a Cue.
- The modal then shows **Current default: Add Cue**. A one-off **Merge** with Fixture 1 at 90% arms
  `RECORD MERGE`, and `CUELIST 101 CUE 1` merges the value into Cue 1, where the programmer value
  wins. The default stays Add Cue.
- Choosing **Smart** with **Set as default** on restores the regular behaviour. `RECORD CUELIST 101
  CUE 1` overwrites Cue 1 with the programmer again.

The same default applies when a Cuelist or playback is touched, and when an attached desk records
onto a playback button:

- **Smart** still asks Add, Merge, or Overwrite for a one-Cue list.
- **Merge** and **Add Existing** store into the playback's active Cue, or into the only Cue of a
  stopped list, without asking.
- **Add Cue** always appends.

On an attached desk, a second Record key press while Record is armed opens the same modal. The
retired browser settings are migrated once: **Merge into active Cue** on becomes the Merge default,
and the never-applied **Record mode** is dropped.

## RECUPD-002 — UPDATE UPDATE: the same layout and a stored Update default

Given the same Cuelist, holding Shift and pressing `[REC]` twice opens a modal titled **Update**
with the same four options, **Set as default** off, the current default, and **Update** in its
title bar. Its **Targets** action opens the Update Targets list.

- Choosing **Merge** with **Set as default** on stores Merge. A plain `UPDATE CUELIST 101 CUE 1`
  then behaves like Update All and adds Fixture 2 to Cue 1.
- Choosing **Smart** with **Set as default** on restores plain Update. An address the Cue does not
  store yet is then ignored.
- **Add Existing** updates Cues like Update Known.
- **Add Cue** stores the programmer as a new Cue at the end of the addressed Cuelist.
- Presets and Groups follow Merge as Update All; for them, Add Existing and Add Cue update like Smart.
