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
| `1.3` | A specific value; in this example, the same as `[1][ . ][3]`. |
| `\|` | Separates alternative command forms. |

## Available buttons

| Key | Button | What it does |
| --- | --- | --- |
| `[0-9]` | Numbers | Enter numeric values. |
| `[TRU]` | Thru | Define a range. |
| `[+]` | Plus | Add to a range or offset a subset. |
| `[AT]` | At | Separate the selection from the value. |
| `[ . ]` | Dot | Separate parts, such as `universe.address` or `type.preset`, or enter a decimal point, such as `3.5` meters. |
| `[DIV]` | Division | Edit a selection when used in the selection part, or separate multiple values when used in the value part. Hold for selection options. |
| `[GRP]` | Group | Select a group instead of a fixture. Press twice to reference the fixtures in the group rather than the group itself. |
| `[CUE]` | Cue | Separate a playback address from its cue number. |
| `[SET]` | Set | Set a value, assign a control, or open a context menu. |
| `[REC]` | Record | Store cues, presets, and groups. Hold for record options. |
| `[DEL]` | Delete | Delete a cue, preset, or other supported element. |
| `[MOV]` | Move | Move a cue or preset to a new number or location. |
| `[CPY]` | Copy | Copy a cue or preset to a new number or location. |

The following buttons usually do not appear in the command line:

| Key | Button | What it does |
| --- | --- | --- |
| `[ENT]` | Enter | Confirm the command. |
| `[PRE]` | Preload | Run Preload or Preload GO. Hold to clear the preload. |
| `[CLR]` | Clear | Clear the selection first, then the programmer. |
| `[ESC]` | Escape | Close menus. If all menus are closed, clear the command line. |
| `[UND]` | Undo | Undo the latest programming change, such as storing a preset, recording a cue, or renaming an item. Fader changes and playback executions are not affected. |

## Typical Layout on the Software Desk:

[SET] [GRP] [CUE] [UND] [CLR]

[DEL] [ 7 ] [ 8 ] [ 9 ] [ + ] 

[MOV] [ 4 ] [ 5 ] [ 6 ] [TRU] 

[CPY] [ 1 ] [ 2 ] [ 3 ] [DIV] 

[<--] [ 0 ] [ . ] [ AT] [ENT]


The remaining [REC], [PRE] and [ESC] buttons are located in or next to the command line display.

## Keyboard shortcuts in touch/ software mode.

If no hardware is connected, the following keyboard shortcuts are always active, unless you focus an input field that requires text input. If you focus an input field that requires command input, the keyboard shortcut commands are routed towards that field and [ENT] confirms that field, while [ESC] usually acts as close. If no fields are focused or input modals are open, the keyboard shortcuts always go directly into the main command line.

- Numpad Numbers 0-9 (and by default, but disableable in settings regular 0-9) -> [0] - [9]
- `ESC` -> [ESC]
- `Backspace` -> [<--]
- `ENTER` -> [ENT]
- `.` -> [ . ]
- `+` (key right of ü) -> [+]
- `ß` (key right of the 0) -> [TRU]
- `^` (key left of 1) -> [PRE]
- `´` (key right of ß) -> [DIV]
- `#` (key below +) -> [AT]
- `DEL` -> [CLR]
- `HOME` -> [SET]
- `END`-> [REC]
- `SHIFT`+ `´` -> [DEL]
- `SHIFT`+ `+` -> [CPY]
- `SHIFT`+ `#` -> [MOV]
- `SHIFT`+ `^` -> [GRP]
- `SHIFT`+ `?` -> [CUE]
- `PAGEUP` / `PAGEDN` -> Page Plus and Page Minus for Playback pages
- `F1` - `F8` -> first (lowest) button of Paged Playback 1-8
- `F9` - `F13` -> Speed Group A-E

this leaves all letter keys free for regular typing as well as a future feature of custom shortcuts

The software shortcuts are disabled while hardware is connected. `[AT] [AT]` expands to `[AT] [FULL] [ENT]`, and `[ . ] [ . ]` expands to `[AT] 0 [ENT]`.


## Selecting fixtures

A number without `[GRP]` always identifies a fixture. `[ENTER]` completes the selection. Fixture IDs that do not exist are ignored; they do not make the command fail.

### Building a selection across the desk

Selection is additive until you use or clear it. You can select one fixture, then another fixture, then a group, and all of them remain selected. This works the same way with Stage clicks, a Stage marquee, Fixture Sheet rows, the Groups pool, and command-line selections. You do not need to hold a modifier key when moving between those surfaces.

Each additional selection behaves like adding another range with `[+]`. Fixtures and groups can be combined, and overlapping fixtures appear only once in the resolved selection. Group selections remain identifiable as group references.

The selection stays open while you are only selecting. It closes when you do any of the following:

- Add or change a programmer value, including a command-line value.
- Move an encoder or recall a preset for the selection.
- Press `[CLR]` once to clear the selection explicitly.

Applying a value does not immediately hide or deselect the fixtures. It marks the current selection as used. The next fixture or group you select replaces those used targets and begins a new additive selection. Values already applied to the previous fixtures remain in the programmer.

For example, select fixture 1 in the Fixture Sheet, marquee fixtures 2 through 4 on the Stage, and then select group 2. The complete combined selection remains active. Set its intensity, then select fixture 21: fixture 21 starts a new selection. Alternatively, press `[CLR]` once before selecting fixture 21 to clear the old selection explicitly without clearing its programmed values.

| Selection | Command | Result |
| --- | --- | --- |
| One fixture | `1 [ENTER]` | Select fixture 1. |
| Complete multi-head fixture | `100 [ENTER]` | Select master 100.0 followed by every child head. |
| Multi-head masters | `100.0 [THRU] 110.0 [ENTER]` | Select only the masters of fixtures 100 through 110. |
| Multi-head children | `100 [THRU] 110 [ENTER]` | Select every child head in the range, excluding the masters. |
| One child head | `100.2 [ENTER]` | Select only child head 2 of fixture 100. |
| Fixture range | `1 [THRU] 10 [ENTER]` | Select every existing fixture with an ID from 1 through 10. |
| Combined ranges | `1 [THRU] 10 [+] 20 [THRU] 30 [ENTER]` | Select every existing fixture from 1 through 10 and from 20 through 30. |

`[+]` extends the current selection. All parts joined with `[+]` form one ordered selection for any subsequent subsetting operation.

Child heads use one-based sub-addresses, while `.0` identifies the shared master. A standalone multi-head fixture ID expands to its master and children. A bare range expands multi-head fixtures to their children so effects run across their individually controllable light sources; a `.0` range selects the masters instead.

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

`<selection> [AT] <value> [ENTER]` assigns a value to the selection. A plain number is an intensity value from 0 through 100. A value containing `[ . ]` references a preset.

| Value | Example | Result |
| --- | --- | --- |
| Intensity | `<selection> [AT] 75 [ENTER]` | Set the selected fixtures to 75% intensity. |
| All preset | `<selection> [AT] 0.1 [ENTER]` | Apply All preset 1. |
| Intensity preset | `<selection> [AT] 1.1 [ENTER]` | Apply Intensity preset 1. |
| Color preset | `<selection> [AT] 2.1 [ENTER]` | Apply Color preset 1. |
| Position preset | `<selection> [AT] 3.1 [ENTER]` | Apply Position preset 1. |
| Beam preset | `<selection> [AT] 4.1 [ENTER]` | Apply Beam preset 1. |

## Recording

After building a scene in the programmer, press `[REC]` and choose a recordable target in the UI. Targets include presets, groups, and Cuelists in their pools, as well as playback buttons and faders on physical or simulated hardware. Recording a Cuelist in the pool does not assign it to any playback page.

### Presets and groups

| Target | Command | Result |
| --- | --- | --- |
| UI target | `[REC] <target+>` | Record the programmer into the chosen UI or hardware target. |
| Numbered preset | `[REC] <preset-type> [ . ] <preset-number> [ENTER]` | Record a preset. Types 0 through 4 are All, Intensity, Color, Position, and Beam. |
| Numbered group | `[REC] [GRP] <group-number> [ENTER]` | Record the current selection as a group. |

### Cuelists, Cues, and playbacks

Cuelist and Cue selection uses one unambiguous address grammar. A playback is the page slot containing the fader and buttons; a Cuelist is the ordered collection of Cues assigned to that playback.

- `[SET] <Cuelist-number>` selects a Cuelist.
- `[SET] <Cuelist-number> [CUE] <Cue-number>` selects a Cue in that Cuelist.
- `[SET] <playback-page> [ . ] <playback-number>` selects a playback by its page position.
- `[SET] <playback-page> [ . ] <playback-number> [CUE] <Cue-number>` selects a Cue in the Cuelist assigned to that playback.

| Target | Command | Result |
| --- | --- | --- |
| Cuelist | `[REC] [SET] <Cuelist-number> [ENTER]` | Create a Cuelist in an empty pool slot, or append a Cue to an existing Cuelist. The Cuelist remains unassigned. |
| Specific Cue | `[REC] [SET] <Cuelist-number> [CUE] <Cue-number> [ENTER]` | Record at the specified Cue number. |
| Page playback | `[REC] [SET] <page> [ . ] <playback-number> [ENTER]` | Append a Cue to the Cuelist assigned to that playback. |
| Cue on a page playback | `[REC] [SET] <page> [ . ] <playback-number> [CUE] <Cue-number> [ENTER]` | Record at a specified Cue in the assigned Cuelist. |

Dots after `[CUE]` form decimal Cue numbers. For example, `[REC] [SET] 1 [CUE] 2 [ . ] 5 [ENTER]` records Cue `2.5` in Cuelist 1. The `Cues · Cuelist1` view can renumber the Cuelist later. If the specified Cue already exists, a dialog asks whether to merge into it or overwrite it.

## Deleting, moving, and copying

### Presets

| Action | Command | Result |
| --- | --- | --- |
| Delete | `[DEL] <preset-type> [ . ] <preset-number> [ENTER]` | Delete the specified preset. |
| Move | `[MOV] <preset-type> [ . ] <preset-number> [AT] <new-preset-number> [ENTER]` | Move the preset within its current type. |
| Copy | `[CPY] <preset-type> [ . ] <preset-number> [AT] <new-preset-number> [ENTER]` | Copy the preset within its current type. |

The destination omits the preset type because command-line copy and move operations cannot change a preset's type.

### Cues

Cue source and destination addresses both use the Cuelist/playback selection grammar above. A move or copy therefore has a complete `[SET] ... [CUE] ...` address on each side of `[AT]`.

| Action | Command | Result |
| --- | --- | --- |
| Delete a Cue from a Cuelist | `[DEL] [SET] <Cuelist-number> [CUE] <Cue-number> [ENTER]` | Delete a Cue from a Cuelist. |
| Delete a Cue through a playback | `[DEL] [SET] <page> [ . ] <playback-number> [CUE] <Cue-number> [ENTER]` | Delete a Cue from the Cuelist assigned to a playback. |
| Move or copy between Cuelists | `<operation> [SET] <Cuelist-number> [CUE] <Cue-number> [AT] [SET] <Cuelist-number> [CUE] <Cue-number> [ENTER]` | Move or copy a Cue between Cuelists. `<operation>` is `[MOV]` or `[CPY]`. |
| Move or copy using playbacks | `<operation> [SET] <page> [ . ] <playback-number> [CUE] <Cue-number> [AT] [SET] <page> [ . ] <playback-number> [CUE] <Cue-number> [ENTER]` | Move or copy a Cue using page-relative playback source and destination addresses. Cuelist and playback addresses may be mixed. |

## Assigning and configuring playbacks

On the touch UI, press `[SET]`, tap an existing entry in the Cuelist Pool, then tap the target playback fader. The selected Cuelist replaces the current assignment at that page position. Playback pages accept Cuelists only; groups remain in the Groups pool.

| Action | Command | Result |
| --- | --- | --- |
| Assign a Cuelist | `[SET] <Cuelist-number> [AT] <page> [ . ] <playback-number> [ENTER]` | Assign a Cuelist to a playback on a page. |
| Configure a Cuelist | `[SET] <Cuelist-number> [ENTER]` | Open the Cuelist configuration. |
| Configure a page playback | `[SET] <page> [ . ] <playback-number> [ENTER]` | Open the configuration for the playback at that page position. |

## OSC playback addressing

- `/light/{desk}/page-playback/{playback}/{fader-or-button}` addresses a numbered playback on the page currently active for that desk or screen.
- `/light/playback/{page}/{playback}/{fader-or-button}` addresses that page and playback globally, independent of every desk's current page.
- `/light/cuelist/{Cuelist}/{action}` directly operates a Cuelist when a page playback is not the intended target.

The hardware simulator uses `page-playback`. The former `paged-playback`, `/light/qlist/{number}/{action}`, and direct `/light/playback/{Cuelist}/{action}` forms remain compatibility aliases for existing integrations.
