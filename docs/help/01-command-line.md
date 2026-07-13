# Command Line Reference

You can program the entire desk from the command line.

## Syntax in this document

The examples below use the following notation:

| Notation | Meaning |
| --- | --- |
| `<part>` | A part of a command that usually consists of multiple button presses. |
| `<part*>` | A part entered on the touchscreen by selecting an element in the UI. |
| `<part+>` | A software element or physical control, such as a playback button. |
| `[KEY]` | Press a specific button once. |
| `[KEY][KEY]` | Press a specific button twice. |
| `[KEY+]` | Hold a specific button until its action occurs. |
| `[KEY*]` | An optional button press. It is accepted, but not required. |
| `1.3` | A specific value; in this example, the same as `[1][DOT][3]`. |
| `\|` | Separates alternative command forms. |

## Available buttons

| Key | Button | What it does |
| --- | --- | --- |
| `[0-9]` | Numbers | Enter numeric values. |
| `[THRU]` | Thru | Define a range. |
| `[+]` | Plus | Add to a range or offset a subset. |
| `[AT]` | At | Separate the selection from the value. |
| `[DOT]` | Dot | Separate parts, such as `universe.address` or `type.preset`, or enter a decimal point, such as `3.5` meters. |
| `[DIV]` | Division | Edit a selection when used in the selection part, or separate multiple values when used in the value part. Hold for selection options. |
| `[GRP]` | Group | Select a group instead of a fixture. Press twice to reference the fixtures in the group rather than the group itself. |
| `[SET]` | Set | Set a value, assign a control, or open a context menu. |
| `[REC]` | Record | Store cues, presets, and groups. Hold for record options. |
| `[DEL]` | Delete | Delete a cue, preset, or other supported element. |
| `[MOV]` | Move | Move a cue or preset to a new number or location. |
| `[CPY]` | Copy | Copy a cue or preset to a new number or location. |

The following buttons usually do not appear in the command line:

| Key | Button | What it does |
| --- | --- | --- |
| `[ENTER]` | Enter | Confirm the command. |
| `[PRE]` | Preload | Run Preload or Preload GO. Hold to clear the preload. |
| `[CLR]` | Clear | Clear the selection first, then the programmer. |
| `[ESC]` | Escape | Close menus. If all menus are closed, clear the command line. |
| `[UNDO]` | Undo | Undo the latest programming change, such as storing a preset, recording a cue, or renaming an item. Fader changes and playback executions are not affected. |

## Selecting fixtures

A number without `[GRP]` always identifies a fixture. `[ENTER]` completes the selection. Fixture IDs that do not exist are ignored; they do not make the command fail.

| Selection | Command | Result |
| --- | --- | --- |
| One fixture | `1 [ENTER]` | Select fixture 1. |
| Fixture range | `1 [THRU] 10 [ENTER]` | Select every existing fixture with an ID from 1 through 10. |
| Combined ranges | `1 [THRU] 10 [+] 20 [THRU] 30 [ENTER]` | Select every existing fixture from 1 through 10 and from 20 through 30. |

`[+]` extends the current selection. All parts joined with `[+]` form one ordered selection for any subsequent subsetting operation.

### Subsetting a selection

`[DIV]` selects fixtures by their position in the full ordered selection, not by their fixture ID. A missing divisor defaults to 2.

| Subset | Command | Result |
| --- | --- | --- |
| Every second fixture | `<selection> [DIV] 2 [ENTER]` | Select positions 1, 3, 5, and so on. `<selection> [DIV] [ENTER]` is equivalent. |
| Every third fixture | `<selection> [DIV] 3 [ENTER]` | Select positions 1, 4, 7, and so on. |
| Offset a subset | `<selection> [DIV] 2 [+] 1 [ENTER]` | Select positions 2, 4, 6, and so on. |
| Other offsets | `<selection> [DIV] 3 [+] 1 [ENTER]`<br>`<selection> [DIV] 3 [+] 2 [ENTER]` | Shift the starting position of every-third-fixture selection by one or two positions. |
| Even-selection shortcut | `<selection> [DIV][DIV] [ENTER]` | Shortcut for `<selection> [DIV] 2 [+] 1 [ENTER]`. |

For example, when `<selection>` is `1 [THRU] 10 [+] 20 [THRU] 30`, `[DIV]` continues through that entire combined selection rather than restarting at fixture 20.

### Groups and group references

`[GRP] <group-number>` selects a group by reference. The reference remains connected to the source group: if the fixtures in the source group change later, programming and derived groups that retain this reference change with it.

| Group selection | Command | Result |
| --- | --- | --- |
| Reference a group | `[GRP] 1 [ENTER]` | Select group 1 as a live reference. |
| Reference a subset | `[GRP] 1 [DIV] 2 [ENTER]` | Select every second fixture in group 1 while retaining the group reference. |
| Dereference a group | `[GRP][GRP] 1 [ENTER]` | Select the fixtures currently in group 1 as individual fixtures. Later changes to group 1 do not affect this selection. |

Double-pressing a group in the Groups pool also dereferences it. A group recorded from a referenced or subdivided group retains that relationship; a group recorded from a dereferenced selection stores the individual fixtures instead.

## Setting values

`<selection> [AT] <value> [ENTER]` assigns a value to the selection. A plain number is an intensity value from 0 through 100. A value containing `[DOT]` references a preset.

| Value | Example | Result |
| --- | --- | --- |
| Intensity | `<selection> [AT] 75 [ENTER]` | Set the selected fixtures to 75% intensity. |
| All preset | `<selection> [AT] 0.1 [ENTER]` | Apply All preset 1. |
| Intensity preset | `<selection> [AT] 1.1 [ENTER]` | Apply Intensity preset 1. |
| Color preset | `<selection> [AT] 2.1 [ENTER]` | Apply Color preset 1. |
| Position preset | `<selection> [AT] 3.1 [ENTER]` | Apply Position preset 1. |
| Beam preset | `<selection> [AT] 4.1 [ENTER]` | Apply Beam preset 1. |

## Recording

After building a scene in the programmer, press `[REC]` and choose a recordable target in the UI. Targets include presets, groups, and playbacks in their pools, as well as playback buttons and faders on physical or simulated hardware.

### Presets and groups

| Target | Command | Result |
| --- | --- | --- |
| UI target | `[REC] <target+>` | Record the programmer into the chosen UI or hardware target. |
| Numbered preset | `[REC] <preset-type> [DOT] <preset-number> [ENTER]` | Record a preset. Types 0 through 4 are All, Intensity, Color, Position, and Beam. |
| Numbered group | `[REC] [GRP] <group-number> [ENTER]` | Record the current selection as a group. |

### Playbacks and cues

`[SET] <playback-number>` addresses a playback by its absolute number. `[SET] <page> [SET] <page-playback>` addresses a playback by its position on a page.

| Target | Command | Result |
| --- | --- | --- |
| Absolute playback | `[REC] [SET] <playback-number> [ENTER]` | Create a cue list on an empty playback, or append a cue when the playback already contains a cue list. |
| Specific cue | `[REC] [SET] <playback-number> [DOT] <cue-number> [ENTER]` | Record at the specified cue number. |
| Page playback | `[REC] [SET] <page> [SET] <page-playback> [ENTER]` | Create a cue list or append a cue on a page-relative playback. |
| Page playback cue | `[REC] [SET] <page> [SET] <page-playback> [DOT] <cue-number> [ENTER]` | Record at a specified cue on a page-relative playback. |

Additional dotted parts insert cues between existing cue numbers. For example, `[REC] [SET] 1 [DOT] 2 [DOT] 5 [ENTER]` records cue `2.5` on playback 1. More dotted parts may be added when needed. The Playback Sequence view can renumber the sequence later. If the specified cue already exists, a dialog asks whether to merge into it or overwrite it.

## Deleting, moving, and copying

### Presets

| Action | Command | Result |
| --- | --- | --- |
| Delete | `[DEL] <preset-type> [DOT] <preset-number> [ENTER]` | Delete the specified preset. |
| Move | `[MOV] <preset-type> [DOT] <preset-number> [AT] <new-preset-number> [ENTER]` | Move the preset within its current type. |
| Copy | `[CPY] <preset-type> [DOT] <preset-number> [AT] <new-preset-number> [ENTER]` | Copy the preset within its current type. |

The destination omits the preset type because command-line copy and move operations cannot change a preset's type.

### Cues

In a source address, `[SET] <playback-number>` selects an absolute playback and `[SET] <page> [SET] <page-playback>` selects a page-relative playback. After `[AT]`, omit the first `[SET]`: use `<playback-number> [DOT] <cue-number>` for an absolute destination or `<page> [SET] <page-playback> [DOT] <cue-number>` for a page-relative destination.

| Action | Command | Result |
| --- | --- | --- |
| Delete an absolute cue | `[DEL] [SET] <playback-number> [DOT] <cue-number> [ENTER]` | Delete a cue from an absolute playback. |
| Delete a page-relative cue | `[DEL] [SET] <page> [SET] <page-playback> [DOT] <cue-number> [ENTER]` | Delete a cue from a playback on a page. |
| Move or copy between absolute playbacks | `<operation> [SET] <playback-number> [DOT] <cue-number> [AT] <playback-number> [DOT] <cue-number> [ENTER]` | Move or copy a cue to an absolute playback. `<operation>` is `[MOV]` or `[CPY]`. |
| Move or copy using pages | `<operation> [SET] <page> [SET] <page-playback> [DOT] <cue-number> [AT] <page> [SET] <page-playback> [DOT] <cue-number> [ENTER]` | Move or copy a cue using page-relative source and destination addresses. Absolute and page-relative addresses may also be mixed. |

## Assigning and configuring playbacks

| Action | Command | Result |
| --- | --- | --- |
| Assign an absolute playback | `[SET] <playback-number> [AT] <page> [DOT] <page-playback> [ENTER]` | Assign an absolute playback to a position on a page. |
| Assign a group | `[SET] [GRP] <group-number> [AT] <page> [DOT] <page-playback> [ENTER]` | Assign a group to a position on a page. |
| Configure an absolute playback | `[SET] <playback-number> [ENTER]` | Open the playback configuration. |
| Configure a page playback | `[SET] <page> [DOT] <page-playback> [ENTER]` | Open the configuration for a playback at a position on a page. |
