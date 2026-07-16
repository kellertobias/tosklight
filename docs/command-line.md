# ToskLight Command Line Reference

You can program the entire desk from the command line.

With no active command, the command line contains the full editable default `FIXTURE` or `GROUP`. As soon as a selection is entered, those targets shorten to `F` and `G`: Fixture mode displays `F7 + F8`, while Group mode displays `G7 + G8`. Press `[GRP] [ENTER]` by itself to change the persistent default; `[CLR]` and `[ESC]` restore its full word. After Plus, `[GRP]` selects the opposite target for that term, so Fixture mode can display `F7 + G8` and Group mode can display `G7 + F8`. Record operations are the exception: `[REC] [+] [GRP] 3` remains `RECORD + GROUP 3` because `[+]` selects the Merge operation and `GROUP` is the storage target.

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
| `[−]` | Minus | Remove fixtures from a selection. After `[AT]`, subtract from each selected fixture's current value; direct numeric fields use it as a negative sign. |
| `[AT]` | At | Separate the selection from the value. |
| `[TIME]` | Time | Set an explicit fade for values or a Cue. Press twice to enter `DELAY`. |
| `[SHIFT]` | Shift | Latch the shifted layer for the next keypad key. Shifted numbers open built-in windows; `[SHIFT] [TIME]` enters `SPD GRP`. |
| `[DOT]` | Dot | Separate parts, such as `universe.address` or `type.preset`, or enter a decimal point, such as `3.5` meters. |
| `[DIV]` | Division | Edit a selection when used in the selection part, or separate multiple values when used in the value part. Hold for selection options. |
| `[GRP]` | Group | Select a group instead of a fixture. Press twice to reference the fixtures in the group rather than the group itself. |
| `[CUE]` | Cue | Separate a playback address from its cue number. |
| `[SET]` | Set | Set a value, assign a control, or open an element's configuration. |
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
| `[UND]` | Undo | Undo the latest programming change, such as storing a preset, recording a cue, or renaming an item. Fader changes and playback executions are not affected. |

### Software keypad and keyboard shortcuts

The touch keypad is arranged as follows:

```text
[SET] [GRP] [CUE] [UND] [CLR]
[DEL] [ 7 ] [ 8 ] [ 9 ] [ + ]
[MOV] [ 4 ] [ 5 ] [ 6 ] [TRU]
[CPY] [ 1 ] [ 2 ] [ 3 ] [DIV]
[<--] [ 0 ] [ . ] [ AT] [ENT]
[SHIFT] [TIME] [ − ]
```

`[REC]`, `[PRE]`, and `[ESC]` remain beside the command-line display. When no hardware is connected, numpad digits, Escape, Backspace, Enter, Dot, and the documented German-keyboard positions map to these buttons; Shift-Z enters `SELECT`; Page Up/Down change playback pages; F1–F8 press the first button of paged playbacks 1–8; and F9–F13 address speed groups A–E. Regular 0–9 keys are enabled by default and can be disabled under Setup → Inputs. Other letter keys remain available for text and future custom shortcuts. All software shortcuts are disabled when hardware is connected.

`[AT][AT]` is the shortcut for `[AT] [FULL] [ENT]`. `[DOT][DOT]` is the shortcut for `[AT] 0 [ENT]`.

`[SHIFT] 1` through `[SHIFT] 9`, then `[SHIFT] 0`, open Stage, Fixtures, Groups, Presets, Cuelists, Channels, DMX, Dynamics, Help, and Development. Shift cancels after the next key or when pressed again.

`[SHIFT] 4` opens the Cue details for the active playback. The active playback is an operator selection, not merely the most recently running playback.

#### Speed-group shortcut

Press `[SHIFT] [TIME]` to enter `SPD GRP`. Speed-group numbers `1` through `5` correspond to Speed Groups A through E.

| Action | Command | Result |
| --- | --- | --- |
| Set a whole-number BPM | `[SHIFT] [TIME] 1 [+] 120 [ENTER]` | Set Speed Group A to 120 BPM. |
| Set a fractional BPM | `[SHIFT] [TIME] 2 [+] 127,5 [ENTER]` | Set Speed Group B to 127.5 BPM. A comma may be used as the decimal separator. |
| Synchronize two groups | `[SHIFT] [TIME] 1 [AT] 2 [ENTER]` | Copy Speed Group A's BPM to Speed Group B and keep A and B synchronized. |

The two speed groups remain synchronized until you set a BPM directly for either group or tap either group to set its tempo.

## Selecting fixtures

A number without `[GRP]` always identifies a fixture. `[ENTER]` completes the selection. Fixture IDs that do not exist are ignored; they do not make the command fail.

| Selection | Command | Result |
| --- | --- | --- |
| One fixture | `1 [ENTER]` | Select fixture 1. |
| Complete multi-head fixture | `100 [ENTER]` | Select master 100.0 followed by every child head. |
| Multi-head masters | `100.0 [THRU] 110.0 [ENTER]` | Select only the masters of fixtures 100 through 110. |
| Multi-head children | `100 [THRU] 110 [ENTER]` | Select every child head in the range, excluding the masters. |
| One fixture head | `501.2 [ENTER]` | Select head 2 of fixture 501, such as the second RGB cell of a Sunstrip. |
| Fixture range | `1 [THRU] 10 [ENTER]` | Select every existing fixture with an ID from 1 through 10. |
| Combined ranges | `1 [THRU] 10 [+] 20 [THRU] 30 [ENTER]` | Select every existing fixture from 1 through 10 and from 20 through 30. |
| Remove from a range | `1 [THRU] 10 [−] 5 [ENTER]` | Select fixtures 1 through 10 except fixture 5. |

`[+]` extends the current selection. All parts joined with `[+]` form one ordered selection for any subsequent subsetting operation.

The selection remains current after a value command, encoder move, or preset recall. The next fixture or group selection replaces those targets unless it begins with `[+]`. For example, after `1 [+] 2 [AT] 75 [ENTER]`, `[AT] 50 [ENTER]` changes fixtures 1 and 2 to 50%, `3 [AT] 80 [ENTER]` starts a new selection for fixture 3, and `[+] 3 [AT] <value-or-preset> [ENTER]` continues the previous selection with fixture 3 added.

Press `[CLR]` once to clear the current selection without clearing programmed values. If programmer values remain, the Clear button blinks; pressing `[CLR]` again clears those programmer values.

Child heads use one-based `fixtureID.headNumber` references, while `.0` addresses the shared master. A standalone parent fixture ID addresses the complete fixture. A bare fixture range expands multi-head fixtures to their children so effects run across the individually controllable light sources; a `.0` range selects the corresponding masters.

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
| Relative increase | `<selection> [AT] [+] 5 [ENTER]` | Add five percentage points to each fixture's current value. |
| Relative decrease | `<selection> [AT] [−] 5 [ENTER]` | Subtract five percentage points from each fixture's current value. |
| All preset | `<selection> [AT] 0.1 [ENTER]` | Apply All preset 1. |
| Intensity preset | `<selection> [AT] 1.1 [ENTER]` | Apply Intensity preset 1. |
| Color preset | `<selection> [AT] 2.1 [ENTER]` | Apply Color preset 1. |
| Position preset | `<selection> [AT] 3.1 [ENTER]` | Apply Position preset 1. |
| Beam preset | `<selection> [AT] 4.1 [ENTER]` | Apply Beam preset 1. |

Relative values are calculated separately for every fixture and clamped to the attribute range. Dereference a live Group with `[GRP][GRP]` before applying a relative value.

### Value fade and delay times

`[TIME] <seconds>` overrides Programmer Fade for the values in that command. `[TIME][TIME]` is displayed as `DELAY`; fade and delay may be entered in either order because the fade always begins after the delay.

| Timing | Command | Result |
| --- | --- | --- |
| Fade override | `<selection> [AT] 100 [TIME] 2 [ENTER]` | Fade these values over two seconds. |
| Delay and fade | `<selection> [AT] 100 [TIME][TIME] 1 [TIME] 2 [ENTER]` | Display `DELAY 1 TIME 2`, wait one second, then fade for two seconds. |

The programmer stores explicit fade and start-delay metadata per changed value. Recording values with different timings into the same Cue preserves them. A value without an explicit fade uses the Cue's master Fade and then the configured Cue Fade fallback; a value without an explicit start delay uses the Cue's master Delay, which is edited in the Cuelist View. In a Cue-record command, `DELAY` instead stores the Cue's GO/FOLLOW/TIME trigger rather than Cue Delay or an attribute start delay.

## Recording

The key immediately after `[REC]` selects the operation: no modifier overwrites, `[+]` merges, and `[-]` subtracts. For a Group or explicit Cue, record-minus with an empty applicable source deletes the target. Consequently, empty-selection `[REC] [-] [GRP] 3 [ENTER]` is equivalent to `[DEL] [GRP] 3 [ENTER]`. Merge and subtract require an existing explicit target. Cancel always cancels a recording and writes nothing.

After building a scene in the programmer, press `[REC]` and choose a recordable target in the UI. Targets include presets, groups, and playbacks in their pools, as well as playback buttons and faders on physical or simulated hardware.

### Presets and groups

| Target | Command | Result |
| --- | --- | --- |
| UI target | `[REC] <target+>` | Record the programmer into the chosen UI or hardware target. |
| Numbered preset | `[REC] <preset-type> [DOT] <preset-number> [ENTER]` | Record a preset. Types 0 through 4 are All, Intensity, Color, Position, and Beam. |
| Overwrite Group | `[REC] [GRP] <group-number> [ENTER]` | Replace ordered membership with the resolved selection; recording Group 3 back onto Group 3 materializes fixtures instead of creating a self-reference. |
| Merge into Group | `[REC] [+] [GRP] <group-number> [ENTER]` | Keep existing order and append newly selected fixtures. |
| Subtract from Group | `[REC] [-] [GRP] <group-number> [ENTER]` | Remove selected fixtures while retaining the other members' relative order. |
| Delete Group | Empty-selection `[REC] [-] [GRP] <group-number> [ENTER]`, or `[DEL] [GRP] <group-number> [ENTER]` | Delete the Group unless a derived Group depends on it. |

For example, click fixture 5 and fixture 6, then press `[REC] [+] [GRP] 3 [ENTER]` to merge them into Group 3. To overwrite from a live reference plus additions, press `[GRP] 3 [+] 5 [+] 6 [ENTER]`, followed by `[REC] [GRP] 3 [ENTER]`.

### Cuelists, Cues, and playbacks

Cuelist and Cue selection uses one address grammar. A playback is the page slot containing the fader and buttons; a Cuelist is the ordered collection of Cues assigned to that playback. `[SET] <Cuelist-number>` selects a Cuelist, while `[SET] <page> [DOT] <playback-number>` selects a playback by page position. Adding `[CUE] <Cue-number>` selects a Cue in the addressed Cuelist.

Press Shift-Z to enter `SELECT`, then touch a playback to make it the active playback. The active playback supplies the default Cuelist whenever a command omits both a playback address and a Cuelist Pool number. It is also the playback whose Cue details open with `[SHIFT] 4`. Selecting and retaining the active playback is not implemented yet; the shortcut currently enters only `SELECT` so the complete workflow can be added with its executable tests.

| Target | Command | Result |
| --- | --- | --- |
| Cue on the active playback | `[REC] [CUE] <Cue-number> [ENTER]` | Record the numbered Cue in the Cuelist assigned to the active playback. The omitted playback/Cuelist address resolves only through the explicit active-playback selection. |
| Cuelist | `[REC] [SET] <Cuelist-number> [ENTER]` | Create a Cuelist in an empty pool slot, or append a Cue to an existing Cuelist. The Cuelist remains unassigned. |
| Specific Cue | `[REC] [SET] <Cuelist-number> [CUE] <Cue-number> [ENTER]` | Record at the specified Cue number. |
| Page playback | `[REC] [SET] <page> [DOT] <playback-number> [ENTER]` | Append a Cue to the Cuelist assigned to that playback. |
| Cue on a page playback | `[REC] [SET] <page> [DOT] <playback-number> [CUE] <Cue-number> [ENTER]` | Record at a specified Cue in the assigned Cuelist. |
| Cue with explicit fade | `[REC] [SET] <Cuelist-number> [CUE] <Cue-number> [TIME] 3 [ENTER]` | Record the Cue with a three-second default fade while retaining per-value overrides. |
| Cue with FOLLOW trigger | `[REC] [SET] <Cuelist-number> [CUE] <Cue-number> [TIME] [TIME] 0 [ENTER]` | The second Time becomes `DELAY`; zero, or `DELAY` confirmed without a number, stores FOLLOW. This Cue starts when the preceding Cue has completely finished. |
| Cue with TIME trigger | `[REC] [SET] <Cuelist-number> [CUE] <Cue-number> [TIME] [TIME] 4 [ENTER]` | Store `DELAY 4`, displayed as TIME 4 seconds. This Cue starts four seconds after the preceding Cue has completely finished. |
| Merge into a Cue | `[REC] [+] [SET] <Cuelist-number> [CUE] <Cue-number> [ENTER]` | Merge programmer value addresses into the Cue, replacing the stored value at each matching address. |
| Subtract from a Cue | `[REC] [-] [SET] <Cuelist-number> [CUE] <Cue-number> [ENTER]` | Remove the programmer's value addresses and keep every other Cue value. |
| Delete a Cue with Record-minus | The same subtract command with an empty programmer | Delete the Cue, unless it is the Cuelist's only Cue. |

Dots after `[CUE]` form decimal Cue numbers. For example, `[REC] [SET] 1 [CUE] 2 [DOT] 5 [ENTER]` records Cue `2.5` in Cuelist 1. Fully entered commands use the explicit operation without a dialog. Clicking an existing Cuelist pool cell appends the next Cue; use a complete command-line address to overwrite, merge, subtract, or delete a specific existing Cue.

A Cue-record command without `DELAY` stores **GO** and waits indefinitely for GO. Bare `DELAY` and `DELAY 0` normalize to **FOLLOW**. Positive `DELAY <seconds>` stores **TIME**. The trigger belongs to the Cue being recorded: if Cue 1 takes two seconds to finish and Cue 2 is TIME 4, Cue 2 starts six seconds after Cue 1's GO. FOLLOW and TIME measure from the latest value `start delay + fade` endpoint of the preceding Cue.

The Cuelist setting **Force Cue Timing** makes each Cue's master Fade and Delay authoritative during playback, ignoring stored per-value fades and start delays without deleting them. Disabling it restores the original per-value timing on the next execution.

The Cuelist setting **Disable Cue Timing** treats per-value and Cue Fade/Delay, TIME-trigger waits, and Chaser X-fade as zero without rewriting them. Chaser step cadence remains active. It takes precedence over Force Cue Timing; disabling the bypass restores every configured duration.

## Deleting, moving, and copying

### Groups

| Action | Command | Result |
| --- | --- | --- |
| Delete | `[DEL] [GRP] <group-number> [ENTER]` | Delete the Group unless a derived Group depends on it. |

### Presets

| Action | Command | Result |
| --- | --- | --- |
| Delete | `[DEL] <preset-type> [DOT] <preset-number> [ENTER]` | Delete the specified preset. |
| Move | `[MOV] <preset-type> [DOT] <preset-number> [AT] <new-preset-number> [ENTER]` | Move the preset within its current type. |
| Copy | `[CPY] <preset-type> [DOT] <preset-number> [AT] <new-preset-number> [ENTER]` | Copy the preset within its current type. |

The destination omits the preset type because command-line copy and move operations cannot change a preset's type.

### Cues

Cue source and destination addresses both use the complete `[SET] ... [CUE] ...` Cuelist/playback selection grammar.

| Action | Command | Result |
| --- | --- | --- |
| Delete a Cue from a Cuelist | `[DEL] [SET] <Cuelist-number> [CUE] <Cue-number> [ENTER]` | Delete a Cue from a Cuelist. |
| Delete a Cue through a playback | `[DEL] [SET] <page> [DOT] <playback-number> [CUE] <Cue-number> [ENTER]` | Delete a Cue from the Cuelist assigned to a playback. |
| Move or copy between Cuelists | `<operation> [SET] <Cuelist-number> [CUE] <Cue-number> [AT] [SET] <Cuelist-number> [CUE] <Cue-number> [ENTER]` | Move or copy a Cue between Cuelists. `<operation>` is `[MOV]` or `[CPY]`. |
| Move or copy using playbacks | `<operation> [SET] <page> [DOT] <playback-number> [CUE] <Cue-number> [AT] [SET] <page> [DOT] <playback-number> [CUE] <Cue-number> [ENTER]` | Move or copy a Cue using page-relative playback source and destination addresses. Cuelist and playback addresses may be mixed. |

Deleting the active Cue removes it from the stored Cuelist but holds its fully reconstructed output until another playback action occurs. GO executes the next surviving Cue; GO minus executes the previous surviving Cue. Navigation then reconstructs tracking from the modified Cuelist, so values introduced only by the deleted Cue release according to the destination Cue's timing. The sole Cue cannot be deleted.

## Assigning and configuring playbacks

On the touch UI, press `[SET]`, tap an existing entry in the Cuelist Pool, then tap the target playback fader. The selected Cuelist replaces the current assignment at that page position. Playback pages accept Cuelists only; groups remain in the Groups pool.

In the Tauri app and browser UI, right-clicking an element is a shortcut for pressing `[SET]` and then left-clicking that same element. Use it wherever `[SET]` followed by a click configures an element or starts a SET assignment; the native context menu does not open. On a touchscreen, continue to press `[SET]` and then tap the element.

To configure an assigned page playback, press `[SET]` and then tap the playback, press `[SHIFT]` and then its first button, or right-click anywhere on the playback. All three gestures open the same Playback configuration modal. **Unassign Playback** removes the Cuelist or Group from that page position and leaves the playback slot empty.

| Action | Command | Result |
| --- | --- | --- |
| Assign a Cuelist | `[SET] <Cuelist-number> [AT] <page> [DOT] <playback-number> [ENTER]` | Assign a Cuelist to a playback on a page. |
| Configure a Cuelist | `[SET] <Cuelist-number> [ENTER]` | Open the Cuelist configuration. |
| Configure a page playback | `[SET] <page> [DOT] <playback-number> [ENTER]` | Open the configuration for the playback at that page position. |

## OSC playback addressing

All keypad keys are accepted at `/light/{desk}/programmer/{key}` with a pressed value. `minus`/`subtract`, `time`, `delay`, and `shift` are the new names; digits use `digit-0` through `digit-9`. OSC Shift is latched, so `shift` followed by `digit-1` opens Stage.

- `/light/{desk}/page-playback/{playback}/{fader-or-button}` addresses a numbered playback on the page currently active for that desk or screen.
- `/light/playback/{page}/{playback}/{fader-or-button}` addresses that page and playback globally, independent of every desk's current page.
- `/light/cuelist/{Cuelist}/{action}` directly operates a Cuelist when a page playback is not the intended target.

The hardware simulator uses `page-playback`. The former `paged-playback`, `/light/qlist/{number}/{action}`, and direct `/light/playback/{Cuelist}/{action}` forms remain compatibility aliases for existing integrations.
