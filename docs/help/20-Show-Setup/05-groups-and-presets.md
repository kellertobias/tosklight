# Groups and Presets

Groups store ordered fixture selections. Presets store reusable attribute values.

## Groups

Select fixtures, press `[REC]`, and choose a Group target or enter its number. Normal record overwrites; `[REC] [+]` merges; `[REC] [-]` subtracts. Intentionally empty Groups remain valid stored objects and differ from absent Group numbers. Missing Groups in a range are skipped.

A Group reference remains connected to its source; dereference it when a frozen fixture list is required. Derived Groups retain their ordering rule and source relationship. See [Command Line Reference](../30-Programmer/01-command-line.md) for exact syntax.

![Group pool with populated ordered Groups](../assets/screenshots/panes/groups.png)

## Presets

Preset families are All, Intensity, Color, Position, and Beam. Record only values appropriate to the intended family. Recalling a Preset applies compatible values to the current selection while retaining the relationship needed for later updates where supported.

Use pane settings to choose the displayed family and pool colors. Test Presets on representative fixture modes before building Cues from them.

![Preset pool and family-specific tiles](../assets/screenshots/panes/presets.png)
