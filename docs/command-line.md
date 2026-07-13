# ToskLight Command Line Reference

You can completely program the desk via the command line.

## Available buttons

We do have the following buttons:
- Numbers 0-9: They are usually just the numeric values
- THRU: Defines a range
- ADD: Adds to a range or value
- AT: divides between selection and value
- DOT: separator between parts (e.g. universe.address or type.preset) or decimal comman (e.g. 3.5<meters>)
- DIV: division. On the selection part this edits the selection, in the value part, this separates multiple values from each other - long pres for selection options
- GRP: GROUP - selects a group rather than a fixture. Pressing twice references the fixtures in the group rather than the group
- SET: Sets a value, assigns a control, opens context menus
- REC: Record - stores cues, presents and groups - long press for record options

The use following buttons usually do not show up in the command line
- ENTER: Confirms the command
- PRE: Preload/ Preload GO - long press for preload clear
- CLR: Clear - Clears the selection, then the programmer
- ESC: Closes menus and clears command lines if all menus are closed
- UNDO: Undoes the latest programming change (e.g. reverts a stored preset to its last state, unrecords a cue, undoes a rename - does not affect fader changes, playback executions, etc.)

## Syntax in this document

In the following section we use the following syntax:
- `<part>`: a part of a command that usually consists of multiple button presses
- `<part*>`: a part of a command that is entered via the touch screen, by pressing somewhere in the UI
- `<part+>`: a part of a command that is either a software element or a physical button (e.g. a playback button)
- `[KEY]`: a specific button press
- `[KEY][KEY]`: a specific button pressed twice
- `[KEY*]`: a specific button held until the action happens
- `([KEY])`: an optional button press. Pressing the button is not an error but not required
- `1.3` specific value. In this case the same as [1][.][3]
- `|` splits up different commands

## Structure of a command

- Setting Values: <selection> [AT] <value> [ENTER] 
  is setting the selection at this selected value. the value can be either an intensity value, a preset or a complex value

- Recording Groups: <selection> [REC] <target*> | <selection> [REC] ([GRP]) <number> [ENTER]
  is storing the selection onto a group slot, either by selecting in the UI or by referencing the number

- Recording Presets & Cues: <set-values> [REC] <target+> | <set-values> [REC] <preset-type>.<preset-number>
  records whatever we currently have in the programmer onto the selected preset

- Setting Attributes: [SET] <target*> | [SET] 1.4 <target*>
  Opens the set attribute modal or directly sets the attribute to the given value

- Assigning Playbacks: [SET] <target*> <target+> | [SET] [GRP] <number> <target+> | [SET] [GRP] <number> [AT] <page>.<playback> | [SET] <playback-number> [AT] <page>.<playback>
  Assigns an element to a playback

- Configuring Playbacks: [SET] <playback-number> | [SET] <page>.<playback>
  Opens the config modal for the given playback

-
