# Groups and Presets

Groups store ordered fixture selections. Presets store reusable attribute values.

## Groups

Select fixtures, press `[REC]`, and choose a Group target or enter its number. Normal record overwrites; `[REC] [+]` merges; `[REC] [-]` subtracts. Intentionally empty Groups remain valid stored objects and differ from absent Group numbers. Missing Groups in a range are skipped.

A Group reference remains connected to its source; dereference it when a frozen fixture list is required. Derived Groups retain their ordering rule and source relationship. See [Command Line Reference](../30-Programmer/01-command-line.md) for exact syntax.

![Group pool with populated ordered Groups](../assets/screenshots/panes/groups.png)

## Presets

Preset families are Mixed, Intensity, Color, Position, and Beam. Intensity stores only intensity attributes. Color stores RGB, CMY, color-wheel, and other Color attributes. Position stores only Position attributes. Beam stores Beam and Focus attributes. Mixed stores any attributes the operator chooses; it does not mean a combined list of every preset family. Recalling a Preset applies compatible values to the current selection while retaining the relationship needed for later updates where supported.

Each family is a separate pool with its own local preset numbers. The command-line address combines type and number: `0.1` is Mixed 1, `1.1` is Intensity 1, `2.1` is Color 1, `3.1` is Position 1, and `4.1` is Beam 1. The dotted address is not a global preset ID, so all five presets numbered 1 can coexist.

Use pane settings to choose the displayed family and pool colors. Test Presets on representative fixture modes before building Cues from them.

![Preset pool and family-specific tiles](../assets/screenshots/panes/presets.png)
