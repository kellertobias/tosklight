# Groups

Groups store ordered fixture selections. Use a live reference when later membership changes should reach existing programming.

Select fixtures, press `[REC]`, and choose a Group target or enter its number. Normal record merges the selection into the Group and keeps existing members in order. Populated tiles show **REC MRG**; empty Groups and unused slots show **REC**. `[REC] [+]` explicitly merges; `[REC] [-]` subtracts. Choose one-off **Smart** before touching a populated Group to choose Merge or Overwrite; `RECORD SMART GROUP 22` explicitly overwrites Group 22. Intentionally empty Groups remain valid stored objects and differ from absent Group numbers. Missing Groups in a range are skipped.

The Group Pool prominently highlights the live Group references selected for programming, including multiple Groups. The bottom-left fixture count is grey when none of the members are selected. When all members are selected, it shows only the total in white. When some members are selected, it shows selected/total, with the selected count in white and the remaining text grey. These counts also reflect frozen fixture selections without selecting the Group reference itself. Double-pressing a Group selects its current members as frozen fixtures. Clearing selection returns the count to grey.

A Group reference remains connected to its source; dereference it when a frozen fixture list is required. Overwriting a Group from a selection made only of live Groups preserves their ordered references, so later source membership changes also reach the recorded Group. Derived Groups retain their ordering rule and source relationship. See [Command Line Reference](01-command-line.md) for exact syntax.

Plain-click a Group to select its live membership. Double-press it quickly to select the current
members as frozen fixtures. Right-clicking a populated Group is the pointer shortcut for `[SET]`
then tapping that Group: it selects the Group as the source for a Playback assignment. Touch-hold,
or enter `[SET] [GRP] <number> [ENT]`, to open the Group settings modal. **General** contains only name, icon, and color. See [Group projection and phase](22-group-projection-and-phase.md) for spatial ordering.

A value programmed on a Group reaches every member that can hold it. A member without that
attribute is left alone rather than given a value it has no channel for: a colour set on a Group of
washes and profiles reaches the washes, and the profiles keep whatever they already had. Output and
the Fixture Sheet are unchanged either way, because neither has ever shown an attribute a fixture
does not have.

Group settings never contain a master. To create a Group Master, press `[SET]`, choose the explicit
Group, then press the destination Playback. Every Playback targeting the same
Group controls one shared master; Playbacks targeting different Groups remain independent HTP
contributors. Changing a Playback to **None** removes that assignment without changing the Group.

![Group pool with populated ordered Groups](../../assets/screenshots/panes/groups.png)

Use [Presets](23-presets.md) to store reusable attribute values.
