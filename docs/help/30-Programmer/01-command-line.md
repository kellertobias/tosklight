# Command Line Reference & Programming your Show

## Intro
You can program the entire desk from the command line or mixed with touch commands in the UI, encoder changes and inputs from the command line.

### Command Types
Commands can be of the following types:

- Selecting Fixtures (many commands start with selecting fixtures, in the following sections we often abbreviate this with `<selection>`)
- Programmer Actions (setting values or presets. If the value isn't relevant for explaination, we show `<value>` here in the manual.)
- Altering Show Stage (e.g. recording, updating, deleting presets, cues and other things)
- Altering Playback State

### Notation & Multiple Button Roles
In the following, we indicate hardware buttons with `[BTN]`, touch commands with `{TOUCH}` and the text that then is shown on the command line `#> FIXTURE`. Defined blocks that usually represent multiple commands are denoted as `<section>`. Values are shown as regular text for simplicity. `12.34` means `[1][2][.][3][4]`. Computer-Keyboard keys are marked by `[KBD:KEY]`.

Another important concept to understand is that every button can have multiple functions depending on how often and long they are pressed and wether they are pressed together with shift.
- Regular Press: You hold down and release a button in a regular typing speed, e.g. `[GRP]`. Once you held down the button the action is shown in the command line `#> GROUP`
- Double-Press: If a button is pressed twice, it's indicated twice in this manual, e.g. `[GRP][GRP]`. The command line indicates the actual intention. When you press `[GRP]` the first time, it shows `#> GROUP`, once you pressed it the second time, it shows `#> DEGROUP`
- Shift-Press: If a button is to be pressed with shift, we access the second function on that button. These are indicated with `[^GRP]`. You hold down shift, then press the desired button, then let go shift. The action is performed, once you click the actual button. If you need to press multiple buttons with shift directly after each other, you do not need to release shift in between. e.g. `[^GRP][^GRP]` can be either entered by holding down shift, then pressing `[GRP]`, then releasing shift, then holding down shift again, pressing `[GRP]` again, then releasing shift again; or you can hold down shift, press `[GRP]` twice and then release shift; the outcome is the same. The command line again indicates the intention, e.g. for `[^GRP]` it's `#> FIXTURE`, while `[^GRP][^GRP]`
- Long-Press/ Hold: If a button is pressed long, it's indicated with a plus, e.g. `[PRELOAD+]` (which resets the preload store). Long presses do not need to be confirmed and directly trigger an action, such as opening a modal or resetting a value.


### Command Line Abbreviations
Entries in the command line are also sometimes abbreviated especially for `#> FIXTURE`, `#> GROUP`, `#> DEGROUP`, `#> DMX` (`#> F7`, `#> G1`, `#> g4`, `#> D2.4`). They abbreviate once the number gets typed.

### Empty the command line
To remove the last typed command, press the `[<--]` key (backspace). If the command line is already empty (only shows `#> FIXTURE` or `#> GROUP` this is a no-op)

To empty the command line, press `[ESC]` once. This removes all typed commands from the command line, but keeps an eventual confirmed selection in tact.


### Inspect recent commands

The desk keeps track of all confirmed commands. To show that history, simply click or typ the command line to unfold **Command Line History** upward. It lists this desk's 50 most recent completed commands newest-first with the time it was executed and the user that executed it, as well as the amount of targets it was applied to.

It also shows errors for the current command (and the current command only, since it will not be accepted if it has an error). There errors can be ackgnowledged. This window opens automatically when a command error occures.

Opening, closing, or inspecting history does not change the unfinished command in the normal command-line field. Choose **Reuse** to copy an earlier command into that field; it is not executed until you press `[ENT]`. `[ESC]`, the close button, and a pointer press outside the panel close it while preserving the current input.

History is transient desk state. Reconnecting to the same running desk restores its recent entries, while restarting the server starts a fresh history. Authentication-like command text containing password, passcode, token, secret, authorization, or API-key terms is retained only as a redacted entry.

## Available buttons and computer shortcuts

The **Desk key** is the button shown on the touchscreen keypad or console.
The **Computer keyboard** column is a computer keyboard shortcut, not another console command.
Keyboard positions describe the position of the key on a German keyboard. The software shortcuts are disabled while hardware is connected.

| Desk key | Button     | Computer keyboard | What it does |
| ---      | ---        | ---               | ---          |
| `[^]`    | Shift      | `[KBD:SHIFT]`     | Shift into the second key command layer. On Touch, it toggles. |
| `[0-9]`  | Numbers    | `[KBD:NUMPAD 0-9]`| Enter numeric values. Regular number-row shortcuts can be disabled in settings. |
| `[ENT]`  | Enter      | `[KBD:ENTER]`     | Confirm the command. |
| `[ESC]`  | Escape     | `[KBD:ESC]`       | Close menus; with all menus closed, clear the command line. |
| `[UND]`  | Undo       | `[KBD:CTRL+Z]`    | Undo the latest programming change; playback execution and fader changes are unaffected. Not all desks have this as a dedicated button. |
| `[<--]`  | Backspace  | `[KBD:BACKSPACE]` | Remove the last command token. |
| `[AT]`   | At         | `[KBD:#]`         | Separate the selection from the value. Press twice for `[AT] 100 [ENT]`. |
| `[.]`    | Dot        | `[KBD:.]`         | Separate address/value parts or enter a decimal point. Press twice for `[AT] 0 [ENT]`. |
| `[+]`    | Plus       | `[KBD:NUMPAD +]`  | Add to a selection; increase a value |
| `[-]`    | Minus      | `[KBD:NUMPAD -]`  | Remove from a selection; subtract a value. |
| `[THRU]` | Thru       | `[KBD:ß]`         | Define a range or spread. |
| `[DIV]`  | Division   | `[KBD:´]`         | Modulo operator on a selection (e.g. select every second). Divide value parts for direct value entry.  Press twice for `#> OFFSET` |
| `[TIME]` | Time       | -                 | Give a value or recorded Cue an explicit fade time.  Press twice for `#> DELAY` |

| `[GRP]`  | Group      | `[KBD:SHIFT + ^]` | Select a group. Hold for showing the group built-in. Press twice for `#> DEGROUP`|
| `[CUE]`  | Cue        | `[KBD:SHIFT + ?]` | Select or Target a particular cue. Press twice for `#> CUELIST` |
| `[PBK]`  | Playback   | -                 | Select or Target a particular playback. Press twice for `#> PBKPAGE` |

| `[REC]`  | Record     | `[KBD:END]`       | Store cues, presets, and groups. Hold for record options. |
| `[PRELD]`| Preload    | `[KBD:^]`         | Run Preload or Preload GO. Hold to clear Preload. |
| `[CLR]`  | Clear      | `[KBD:DELETE]`    | First Click: Clear Selection, Second Click: Clear Programmer |

| `[DEL]`  | Delete     | `[KBD:SHIFT + ´]` | Delete a cue, preset, or other supported element.  |
| `[MOV]`  | Move       | `[KBD:SHIFT + #]` | Move a cue or preset. |
| `[CPY]`  | Copy       | `[KBD:SHIFT + +]` | Copy a cue or preset. Not available on touch only, reach with `[^MOV]` |
| `[SET]`  | Set        | `[KBD:HOME]`      | Set a non-output value, or open configuration.  Press twice for `#> ASSIGN` |
| `[OFF]`  | Off        | -                 | Turn off the target; Press twice for opening "Running & Output" |


| `[HIGH]` | Highlight  | `[KBD:ALT + H]`   | Toggle Highlight and capture the current ordered selection as its frozen original set. |
| `[PREV]` | Prev item  | `[KBD:ALT + <-]`  | While HIGH is active, single the previous original member and wrap at the start. |
| `[NEXT]` | Next item  | `[KBD:ALT + ->]`  | While HIGH is active, single the next original member and wrap at the end. |
| `[ALL]`  | All        | `[KBD:ALT + A]`   | Restore the frozen original set as the actual selection. |

| `[ENC]`  | Enc/Playbk | -                 | Toggle the screen between programmer/encoder and playbacks |
| `[PGUP]` | Page Up    | `[KBD:PAGEUP]`    | Open Next Playback Page |
| `[PGDN]` | Page Down  | `[KBD:PAGEDOWN]`  | Open Previous Playback Page. Hold `[PGUP]` and `[PGDN]` together to open the Playback Page menu |
| `[ALIGN]`| Align      | -                 | Toggles between different alignment/ fan modes |
| `[FADE]` | Prog. Fade | -                 | Enables/ Disables Programmer & Preload Fade time (only on hardware/ osc) |

Hardware is also expected provide at least 4 attribute encoders and one navigational encoder with push.


### Second Layer Assignment

The following table shows the second layer assignment, the user can reach with holding down `[^]`

| Desk key  | Button        | What it does |
| ---       | ---           | ---          |
| `[^0]`    | Preset All    | Open "All" preset built in or select an All preset |
| `[^1]`    | Preset Int.   | Open "Intensity" preset built in or select an Intensity preset |
| `[^2]`    | Preset Color  | Open "Color" preset built in, select a Color preset or set a Color value |
| `[^3]`    | Preset Pos.   | Open "Position" preset built in, select a Position preset or set a Position value |
| `[^4]`    | Preset Beam   | Open "Beam" preset built in, select a Beam preset or set a Beam value |
| `[^5]`    | Dynamics      | Open "Dynamics" built in or select a dynamic preset |
| `[^6]`    | Preset Shaprs | Open "Shapers" preset built in, select a Shapers preset or set a Shapers value |
| `[^7]`    | Preset Focus  | Open "Focus" preset built in, select a Focus preset or set a Focus value |
| `[^8]`    | Preset Ctrl   | Open "Control" preset built in, select a Control preset or set a Control value |
| `[^9]`    | Preset Media  | Open "Media" preset built in, select a Media preset or set a Media value |
| `[^AT]`   | FixAT         | Enters a Fixed AT value or preset |
| `[^ENT]`  | Lock          | Lock or Unlock the Desk |
| `[^ESC]`  | Undo          | Undo the last action |
| `[^CLR]`  | Freeze        | Freeze (or when pressed twice Unfreeze) a fixture or selection |
| `[^GRP]`  | Fixture       | Select a fixture in group mode. Hold for showing the fixture built-in.  Press twice for `#> DMX` |
| `[^CUE]`  | Timecode      | Select or Target a particular timecode |
| `[^PBK]`  | Macro         | Select or Target a particular macro |
| `[^MOV]`  | Copy          | Copy a cue or preset. Same as the `[CPY]` button if it is available. |
| `[^REC]`  | Update        | Updates cues, presets, and groups. Hold for update options. |
| `[^PRELD]`| Clear Preload | Clear Preload. |
| `[^ALIGN]`| Align Off     | Turns off Align mode |

### Software (Touch) and suggested Hardware layout

> [!danger] Content Missing
> We are missing screenshots here


## Selecting fixtures and Groups

Lamps are arranged in fixtures, heads and groups. Selections are always in the order you enter them; The earlier a lamp comes in the input, the earlier it comes in the selection.

You can select lamps either by clicking in the fixture sheet to select individual lamps (and multiple, since the selection does not clear automatically), select ranges (by holding down shift after selecting the first lamp, then selecting the last lamp), or by selecting groups.
Important: You cannot mix touch and hardware selection. Once you change from touch to hardware, a new selection is started.

After you have selected your fixtures and groups with touch, you can directly select a preset with touch or hardware commands. If you want to assign a preset with touch to a hardware selection, you must either press `[ENT]` to finalize the selection or press `[AT]`. Pressing `[ENT]` allows you to directly manipulate the values via encoders, while pressing `[AT]` expects you to enter a value via the command line.

### Selecting Ranges, Adding, Subtracting

You can select either single fixtures `[1][ENT]`, multiple explicit fixtures `[1][+][3]`, ranges `[1][THRU][5]` or a combination of them.
A `THRU` always has the highest specifitiy, meaning values always assume they belong to the `THRU` first:  `[1][THRU][3][+][11][THRU][13]` selects fixtures 1,2,3,11,12 and 13 in that order. `[5][THRU][1]` selects them in the descending order: 5,4,3,2,1.

You can also subtract from a selection: `[1][THRU][5][-][2]` selects fixtures 1,3,4,5 and `[1][THRU][9]-[3][THRU][7]` selects 1,2,8,9. You can also add back to a selection: `[1][THRU][9]-[3][THRU][7][+][5]` selects 1,2,8,9,5 or subtract multiple ranges: `[1][THRU][9]-[3][THRU][4][-][6][-][7]` selects 1,2,5,8,9.

Helpful to know: If a fixture does not exist, it is simply skipped when selecting a range. This includes the start and end fixture. So even if we only have fixtures 3,4,6 and 7, the following selection is a valid selection: `#> 1 THRU 9`

### Selecting Heads

Some fixtures contain multiple outputs, so called heads. they can either be addressed by their fixture ID (e.g. `#> FIXTURE 100`) which selects all heads or by only selecting their master head (usually e.g. the common pan or dimmer or the master layer for a media server) `#> FIXTURE 100.0`.

If you want to select sub-heads, you address the individual heads with `#> FIXTURE 100.1` or ranges `#> FIXTURE 100.1 THRU 100.10`.

If a range contains a sub-head, we automatically only select sub-heads (and fixtures without any sub-heads), but not master heads: `#> FIXTURE 100.1 THRU 109` selects all sub heads of fixtures 100 thru 109 but not their master heads. This helps quickly selecting heads for e.g. pixel effects.

### Storing Groups

To be able to reuse selections over and over, you can store groups. For that, you select the fixtures first, then press `[REC]` and then either a group tile in the group pool or stay fully in the command line: `[REC][GRP][22][ENT]`. This stores the fixtures including their selection order in that group.

When the group is already occupied, you get asked if you want to overwrite the group (if recording via touch) or you can explicitly merge with the existing group: `[REC][+][GRP][22][ENT]`

You can also remove a selection from a stored group. That only works via command line: `[REC][-][GRP][22][ENT]` removes the current selection from group 22.

A full command line would be for example: `#> F1 THRU F12 + F15 + F17 RECORD GROUP 21` and then confirmed with `[ENT]`

### Selecting Groups

Selecting group works exactly like selecting fixtures.

`[GRP][1][THRU][5][+][GRP][22]` selects groups 1,2,3,4,5 and group 22.
You can also add fixtures to a group selection: `[GRP][22][+][42]` selects group 22 and fixture 42.
You can but do not have to add the group keyword directly after a `THRU`, however, if you add groups together, the second selection needs the group keyword again, otherwise you select a fixture.

You can also remove fixtures from a group selection `[GRP][21][-][15]` selects the group, but explicitly excludes fixture 15. You cannot subtract a group from another group though.

### Dereferencing Groups
Whenever you use groups for programming, the cues and presets you store reference the actual group. When the group is updated, all cues referencing that group update as well. If you do not want this behavior for a particular cue, you can dereference a group. That works by pressing `[GRP]` twice which then expands the given group into its individual fixtures (not visibly in the command line) and then everything is programmed directly on the fixtures. Changing the group afterwards then has no effect on these cues.

A dereferenced group is shown with `#> DEGROUP` or `#> g` in the command line.

### Division and Offsetting

Now that we can selecting, store and referencing groups, understanding divisions and offsetting is helpful.
You subset a selection by selecting only every n-th item with a custom offset. The keywords here are `DIV` and `OFFSET`. You can divide and offset any selection, not only groups.

Imagine a group with 12 fixtures, `#> F1 THRU F12 RECORD GROUP 30`. We can now take that group and take every second lamp from the group: `[GRP] 30 [DIV] 2 [ENT]` we now have selected lamp 1,3,5,7,9 and 11. If we want the other selection, we add an offset `[GRP] 30 [DIV] 2 [DIV][DIV] 1 [ENT]`. This expands to `#> G30 DIV 2 OFFSET 1` in the command line and selects every 2nd +1 lamp, namely 2,4,6,8,10 and 12.

This also works with fixture ranges: `#> F1 THRU 12 DIV 2` selects every second lamp from fixture 1 thru 12.

Since dividing by 2 is the most common usecase and offsetting by one as well, we can omit the 2 and 1: `#> G30 DIV OFFSET` is the same as `#> G30 DIV 2 OFFSET 1`. We however also can divide by larger values then 2, e.g. `#> G30 DIV 3 OFFSET 1` which would select 2,5,8 and 11. Its important that the OFFSET value must always be byat least 1 smaller than the DIV value, otherwise the command is erroneous.

When storing a division or offset as a group again, this usually references the group. So if you update the original group, this updates the referenced group.

### Selection Shortcuts

For speed, there are shortcuts for selecting large ranges:
- `#> THRU ` selects all fixtures
- `#> 10 THRU ` selects all fixtures with IDs larger or equal than 10
- `#> THRU 99 ` selects all fixtures with IDs smaller or equal than 99

### Command Line Default Mode
The command line can be in two default modes. Fixture selection and Group selection. In Fixture mode, the user does not need to (but may) enter the `FIXTURE` keyword manually, while in group mode, the user does not need to enter the `GROUP` keyword manually.

In fixture mode `[1] + [4]` automatically becomes `#> F1 + F4`, while in group mode the same input becomes `#> G1 + G4`.

To toggle between the modes, press `[GRP][GRP][ENT]`. You can see in which mode you are by checking the keyword that is shown in an empty command line. In fixture mode it shows `#> FIXTURE` and vice versa.

### Clearing a Selection

Press `[CLR]` once to clear the current selection explicitly without clearing its programmed values. If programmer values remain, the Clear button blinks; press `[CLR]` again to clear those programmer values.


## Setting Values and Recording Presets

Now that you understand how we can make a selection and record groups, we use `<selection>` for all the different types we can make a selection and can start assigning values to the lamps. The easiest way is assinging intensity (dimmer) values: `#> <selection> AT <intensity>`, e.g. `[1][AT][100][ENT]` this sets fixture 1 to 100% dimmer value. You can also set fractal values, e.g. `#> <selection> At 12.34` which sets the intensity to 12.34%

Alternatively, you can also select the fixtures or groups and then turn the intensity encoder. This sets the value as well. Encoders work incrementally/ decrementally; They do not set the absolute value of all fixtures, but usually increase the value the fixture currently has by 1 per ratchet. If you want to do finer adjustments, hold `[^]` while turning the encoder. Increasing / Decreasing intensity also works via command line: `<selection>[AT][+] 10` adds 10% to the current values of intensity of the selected fixtures, while `<selection>[AT][-] 10` removes 10%.

You can set more than just dimmer values. If you switch the encoder to the encoder attribute group of e.g. "color", the first encoder is usually the value for "red", the second the value for "green", then "blue", and so on. Clicking the encoder attribute group again sometimes reveals more pages. For color this for example could be a color wheel.

You can also access other encoder attribute groups by pressing and holding `[^0]` through `[^9]`. The fastest way however is still the touch buttons.

Whenever you changed a value, the selection phase of the command is over and when you now select a group or fixture via either UI or command line, this starts a new selection.

### Setting Non-Intensity values

The default after `[AT]` is the intensity value. You however also can set other values directly. This works by typing `<selection> [AT][^2] <red>[DIV]<green>[DIV]<blue>`. The order is always the same as the encoders. Values you do not want to change can be left empty (e.g. `<selection> [AT][^2] [DIV][DIV]100` only sets blue to 100%) Which number is which encoder group can be taken from [[Second Layer Assignment]].

Here you can also increase/ decrease: `<selection> [AT][^2] [+] 10 [DIV][+] 0 [DIV][-] 10` adds 10 to red, does not change green and subtracts 10 from blue.

### Value Ranges

If you want to spread values across multiple selected fixtures, you can use the `[THRU]` keyword: `<selection> [AT] 10 [THRU] 40` sets the first fixture in the selection to 10% and the last to 40% and the fixtures in between to levels between. You can also chain multiple `THRU` blocks, e.g. `<selection> [AT] 10 [THRU] 100 [THRU] 10`. This sets the outermost selected fixtures to 10% and the innermost to 100%.


What's important is that every value you provide in a spread range always is taken by at least one lamp. This means that you cannot provide more values than lamps.

You can also use spread ranges for other values, e.g. `<selection>[AT][^2] 100 [DIV] 0 [DIV] 0 [THRU] 0 [DIV] 100 [DIV] 0`. This lets the lamps range from red via yellow to green.


### Recording a Preset

Now that we have created a look, we might want to be able to recall this look later. For this, we can use presets.

There are two different types of presets:
- "All Presets" store all attribute types that are currently in the programmer. This can be used to store full looks.
- "Attribute Group Presets" (e.g. a Color Preset or Intensity Preset) only store attributes of the given attribute group. That means that e.g. a color preset only stores color values, but never intensity values.

After you have set the fixtures onto the values you want to store, you can record your preset with `[REC]` and then touching a preset tile in one of the preset pools. Alternatively you can use `[REC][^2] 22` for e.g. storing the color preset 22. When the preset is already recorded, the touch version asks you if you want to overwrite the existing preset, while the command line version directly overwrites it. If you wan to merge/ append, use `[REC][+][^2] 22` this adds.

You can also remove the current values from a preset. `[REC][-][^2] 22` this removes the active attribues from the given preset.

More about the preset pool can be found in the chapter about the preset pane.

### Recalling a Preset

Once you have stored your look into a preset, you can recall that preset.

You can recall a preset for only your current selection by pressing the preset after you have a selection. This applies the preset to all fixtures from your selection for which you have recorded that preset. This also works from the command line by pressing the corresponding attribute group button twice: `[^2][^2]` shows then `#> COLOR PRESET` you now can enter the number of the preset you want, e.g. `#> <selection> AT COLOR PRESET 22` followed by `[ENT]`. The list of attribute group button mappings can be found in [[Second Layer Assignment]].

You can also spread preset ranges: `#> <selection> AT COLOR PRESET 22 THRU 24 [ENT]` if e.g. color preset 22 is red and 24 is green, the selected fixtures now produces a stable gradient from red to green. The command means: take values from 22 and 24 and interpolate a gradient. It does not mean trigger preset 22, 23 and 24. This also works with touch: once you have your selection, touch the first preset, then press the `[THRU]` button, then the next preset. You can chain multiple of these and they are applied immediately and do not require `[ENT]`.

If the selected presets from a spread range do not have an overlap, the fixtures that are missing in one of the presets participating at that spread range section do not have a gradient, but only the single value from the preset they are included in.

There is also a second way to recall a preset, a full preset recall. This works by having no selection and an empty command line and then directly start with the preset recall command: `[^2][^2] 22 [ENT]`. This loads the selection of all fixtures in that preset and applies the preset. It also works in the preset pane via touch: if you have no selection, the first touch on a preset tile activates the selection, the second one applies the preset. For full recall, spreading is not possible.

### Programmer Fade Time

When recalling a preset, the programmer fade time applies. This is the time from triggering the preset until the last lamp has fully faded to the selected value. The button below the fade time slider can toggle between off and on. Long pressing it opens a modal where you can select to which additional changes this fade time applies. By default, it only applies to preset selections and Preload GO, but you can also enable/ disable:
- Preload Go
- Virtual Playback GO/Toggle
- Physical Playback Go/Toggle (for playbacks with fader)
- Physical Playback Go/Toggle (for playbacks with only buttons)

You can also configure the maximum value of that fader and wether the fade is linear or exponential (e.g. lower half 0s-1s upper half 1s-10s)

If you have stored individual fade or delay time spreads and this active for the given category, the fade time decides the maximum time of fade + delay for the longest fade in the given transition. All other times are then scaled accoding to the ratio.

### Recording Fade and Delay Times

You can also store fixed fade and delay time in your presets (and also cues). This works directly from the programmer:

`<slection> [AT] <values> [TIME] 3` which shows `#> <slection> AT <values> FADE 3s` in the command line makes the values fade with 3 seconds. Pressing `[TIME]` a second time, makes it `#> …DELAY`. A delay is the wait time from the start of the preset/ cue until the fade or value change happens.

You can either have multiple individual commands after one another with different values and then store this as a preset (e.g. Fixture 1 goes to 100% with a 10s fade, then fixture 2 has a 5s delay and a 5s fade until it goes to 0%) but you can also use spread operators in fade and delay times: `#> G1 AT 100% FADE 0s THRU 5s DELAY 5s THRU 0s`. This lets the first fixture in the group wait 5s and then snap (0s) to 100%, while the last fixture in the group does not wait and immediately fades over 5s. The fixture in between interpolate their delay and fade time.

You can now store this onto a preset or cue.

Hint: As you have read in the earlier section - if programmer fade time is enabled, recalling a preset scales the selected fade times. This also applies if the programmer fade time slider is at 0s; In that case changes are immediate.

## Recording Cues

We finally arrived at the point where we are able to

### Inserting Cues

### Updating Cues

### Moving and Copying Cues

## Blind Programming & Preload GO

## Dynamics and FixAT


## Freezing

`[SHIFT] [CLR]` enters **FREEZE** on the shared command line. Enter a Fixture or Group selection and
press `[ENT]` to capture a full Freeze. A Group resolves to its current fixtures before the action;
Freeze state is never stored on the Group object. A full
Freeze ignores later Programmer, Cue, Dynamic, direct-control, Group Master, Grand Master, and
Blackout changes. Removing it immediately reveals the current underlying state; Freeze does not
rewrite that state.


To remove Freeze, keep `[SHIFT]` held and press `[CLR]` twice, then release `[SHIFT]`. The command
line shows **UNFREEZE**. Enter a selection and `[ENT]` to remove the complete Freeze state, or add
the same `[SHIFT] [1]` through `[SHIFT] [4]` family keys before `[ENT]` to remove only those partial
families. `[UND]` uses ordinary desk-local Programmer history to restore the Freeze state from
before the last Freeze or Unfreeze action. Freeze has no Redo action.

The Fixture Sheet marks a full Freeze as `❄ FREEZE`. Partial Freeze state uses the same marker plus
the frozen family names: Intensity, Color, Position, and Beam. Partial families keep ordinary Group
Master, Grand Master, and Blackout behavior. Setting a full Freeze replaces partial-family metadata;
removing the full Freeze therefore removes every family Freeze on that target.

### Freezing individual attribues only
For a partial Freeze, enter **FREEZE**, the selection, and one or more family keys before `[ENT]`:
`[SHIFT] [1]` Intensity, `[SHIFT] [2]` Color, `[SHIFT] [3]` Position, and `[SHIFT] [4]` Beam.
These mappings apply while FREEZE or UNFREEZE is pending. Otherwise, with a current selection,
Shift+1 through Shift+4 begin the Intensity, Color, Position, or Beam Preset category. Without a
selection, Shift+1 through Shift+9 select the Intensity, Color, Position, Beam, Dynamics, Shapers,
Focus, Control, or Media encoder page. Shift+5 through Shift+9 deliberately do not create a Preset
command when a selection exists.

## Moving and copying Cues

Address a Cue through its pool playback number: `[CPY] [SET] 1 [CUE] 2 [AT] [SET] 2 [CUE] 2 [ENT]`, or begin with `[MOV]` to move it. Entering the complete command opens a required choice instead of guessing the transfer meaning:

- **Plain Copy** or **Plain Move** transfers only the selected Cue's stored commands and deltas.
- **Status Copy** or **Status Move** materializes the complete tracked source status for attributes touched at or before that Cue.
- **Cancel** closes the choice without changing either Cuelist.

Copy retains the source Cue. Move removes it and recalculates tracking from the remaining stored Cues. The Plain/Status choice independently controls the destination contents.


### Dynamics and Fixed At

`DYNAMIC <number>` toggles a Dynamic on the current ordered selection. Put an explicit fixture or
Group selection before `DYNAMIC` to address a targetless Dynamic, for example
`FIXTURE 1 THRU 10 DYNAMIC 12`. A target-bound Dynamic accepts only its exact stored target scope;
it is never retargeted by a command.

The live instance forms are:

| Operation | Command |
| --- | --- |
| Toggle | `DYNAMIC 12` |
| Off | `DYNAMIC 12 OFF` |
| Size | `DYNAMIC 12 SIZE AT 50` |
| Local speed | `DYNAMIC 12 SPEED AT 2` |
| Phase offset | `DYNAMIC 12 PHASE AT 90` |
| Assign to Playback | `SET DYNAMIC 12 PLAYBACK 5` |

Size uses percent, speed uses a positive multiplier, and phase uses degrees. When more than one
targetless running instance could match, the desk opens an exact instance choice with each running
controller and target count. Choosing one executes a revision-checked `INSTANCE` command; the desk
never selects one by array order.

The command line spells a Fixed At value **`FixAT`**; operator buttons and help labels use **FAT**.
FAT temporarily fixes the current attribute above a Dynamic without stopping its clock. Clearing
or releasing FAT reveals the current phase of the underlying winning Dynamic.

`[SHIFT] [AT]` enters `FixAT` without also entering ordinary `AT`. `FixAT 100` uses the most
recently authored continuous parameter as the active parameter context. Use
`ATTRIBUTE <attribute-name>` before `FixAT` for an explicit parameter, with the normal fixture or
Group selection grammar before it:

| FAT command | Result |
| --- | --- |
| `FixAT 100` | Fix the active parameter on the current selection at full. |
| `FIXTURE 1 THRU 10 ATTRIBUTE pan FixAT 50` | Fix Pan at 50% on the explicit ordered fixture selection. |
| `GROUP 2 ATTRIBUTE intensity FixAT 75 TIME 2 DELAY 1` | Fix Group 2 intensity after one second, fading for two seconds. |

An absent active parameter, unsupported parameter, or discrete parameter is rejected before any
Programmer value changes.

### Value fade and delay times

Append `[TIME] <seconds>` to set an explicit fade for only the values in this command. Pressing `[TIME]` twice changes the second press to `DELAY` in the command line; append the delay in seconds after it. Fade and delay may appear in either order because fading always begins after the delay.

| Timing | Command | Result |
| --- | --- | --- |
| Fade override | `<selection> [AT] 100 [TIME] 2 [ENTER]` | Fade these values over two seconds instead of using Programmer Fade. |
| Delay then fade | `<selection> [AT] 100 [TIME][TIME] 1 [TIME] 2 [ENTER]` | Display `DELAY 1 TIME 2`, wait one second, then fade for two seconds. |
| Fade then delay | `<selection> [AT] 100 [TIME] 2 [TIME][TIME] 1 [ENTER]` | Produce the same timing with the clauses entered in the opposite order. |

The programmer remembers explicit fade and start delay on each changed value. Direct entry is **Immediate** on a fresh desk: command-line `AT` without `[TIME]` and absolute encoder Set Value retain no per-value fade override. Enable **Direct entry uses Programmer Fade** when those direct entries should instead capture the current Programmer Fade. Explicit `[TIME]` remains authoritative in either mode. Recording several values with different command times into one Cue preserves those individual timings. A value with no retained per-value fade uses the Cue's master Fade, then the configured Cue Fade fallback. A value without an explicit start delay uses the Cue's master Delay. Cue Delay is edited in the Cuelist View. `DELAY` has a different scope in a Cue-record command: there it stores the Cue's GO/FOLLOW/TIME trigger as described below, not Cue Delay or an attribute start delay.

## Recording

After building a scene in the programmer, press `[REC]` and choose a recordable target in the UI. Targets include presets, groups, and Cuelists in their pools, as well as the complete Playback card on touch, hardware-layout, physical, and simulated surfaces. The outlined Playback target includes its full label area, every assigned button, and its fader. Touching any part records to that Playback once and suppresses the touched control's normal press, hold, release, or fader action. Recording a Cuelist in the pool does not assign it to any playback page.

### Command target outlines

Bare **Record**, **Set**, **Copy**, **Move**, and **Delete** commands outline only entries that already support the corresponding operation. The literal operation appears inside every outlined target, so the target is not identified by color alone. Press anywhere inside the complete outline; nested controls belong to the outlined target while the command is active.

The command does not turn unsupported entries into new storage operations. For example, whole Playback cards accept Record and Set but not Copy, Move, or Delete. Group cards accept Record, Set, and Delete. Preset cards accept all five operations: Copy and Move first outline occupied sources, then outline only empty destinations in the same Preset family. Dynamic tiles accept their existing Set and Delete operations. Ineligible entries are not outlined.

View changes, Playback page controls, pool navigation, scrolling, search, settings, and other navigation remain ordinary navigation while a target command is armed. Attached Playback controls follow the same rule: an eligible Record or Set target is intercepted for the whole gesture, while Page, NAV, MENU, PROG/PLAYBACK, ESCAPE, and Speed Group controls retain their navigation function.

A Pane supports only bare **Delete**: it outlines the Pane title with a literal **DELETE** badge, and touching that title removes the Pane after its normal confirmation. The Pane body, Settings button, and all Pane navigation remain ordinary controls. Record, Set, Copy, and Move never outline a Pane title.

The key immediately after `[REC]` chooses the record operation:

- no modifier means **Overwrite**;
- `[+]` means **Merge** the current selection or values into the target; and
- `[-]` means **Subtract** the current selection or values from the target.

For a Group or a specific Cue, `[-]` with an empty applicable source deletes the target instead. Thus `[REC] [-] [GRP] 3 [ENTER]` with an empty selection is exactly equivalent to `[DEL] [GRP] 3 [ENTER]`. `[REC] [+]` and `[REC] [-]` require an existing, explicit target; they never append to an implicitly chosen next Cue. Cancel in a recording dialog always cancels the operation and writes nothing.

### Presets and groups

| Target | Command | Result |
| --- | --- | --- |
| UI target | `[REC] <target+>` | Record the programmer into the chosen UI or hardware target. |
| Numbered preset | `[REC] <preset-type> [ . ] <preset-number> [ENTER]` | Record a preset. Types 0 through 4 are Mixed, Intensity, Color, Position, and Beam; only Mixed accepts attributes from any family. |
| Overwrite Group | `[REC] [GRP] <group-number> [ENTER]` | Replace the complete ordered membership with the resolved current selection. Recording a live reference back onto the same Group materializes concrete fixtures and cannot create a self-reference. |
| Merge into Group | `[REC] [+] [GRP] <group-number> [ENTER]` | Retain the existing order and append selected fixtures that are not already members. |
| Subtract from Group | `[REC] [-] [GRP] <group-number> [ENTER]` | Remove every currently selected fixture and retain the relative order of the other members. |
| Delete Group | `[REC] [-] [GRP] <group-number> [ENTER]` with an empty selection, or `[DEL] [GRP] <group-number> [ENTER]` | Delete the Group. Deletion is rejected while a derived Group depends on it. |

To merge fixtures 5 and 6 into Group 3 entirely from the keypad, first click fixture 5 and then fixture 6, without a modifier or value change. Press `[REC]`, `[+]`, `[GRP]`, `[3]`, `[ENTER]`. To overwrite Group 3 with the resolved selection `Group 3 + fixture 5 + fixture 6`, first press `[GRP] [3] [+] [5] [+] [6] [ENTER]`, then press `[REC] [GRP] [3] [ENTER]`.

### Updating existing programming

`[SHIFT] [REC]` arms **UPDATE**. Update reads only actual programmer changes; Highlight, defaults, resolved playback output, and unchanged tracked values are never pulled into storage. The same armed state is shared by the software desk and attached OSC hardware for that desk. While armed, an attached playback button or fader identifies that playback as the Update target and is intercepted before its normal playback action; the main desk opens the same touch-confirmation workflow.

The Record gesture has three mutually exclusive Update forms:

| Gesture | Result |
| --- | --- |
| Short `[SHIFT] [REC]` | Arm Update and wait for a target. |
| While Shift remains held, press `[REC]` a second time | Open **Update Update**, the eligible-target menu. |
| Hold `[SHIFT] [REC]` for about 2.5 seconds | Open **Update Settings** without arming or applying an Update. |

After arming Update, touch an existing Cuelist, assigned playback, Preset, or Group. Touch normally opens a preview that identifies the concrete target, current Cue when applicable, eligible changes, ignored changes, and the storage location. **Cancel** disarms Update and writes nothing. **Show Update modal on touch** can be disabled in Update Settings; touch then applies the configured default directly. Completing an address with `[ENT]` always applies that default directly.

For a playback target, `[UPDATE] [SET] <playback-number> [ENT]` addresses the slot on this desk's current page. `[UPDATE] [SET] <page> [ . ] <playback-number> [ENT]` pins the explicit page. Append `[CUE] <Cue-number>` to address a particular Cue. Changing the desk page changes only the current-page form; an explicit-page address remains pinned.

Cue targets offer exactly these modes:

| Mode | Result |
| --- | --- |
| **Existing Only** | Update eligible fixture/attribute values at the Cue events currently supplying the tracked values. New addresses are ignored. |
| **Existing in Current Cue** | Update only exact addresses already stored in the current Cue. |
| **Add to Current Cue** | Write eligible addresses that exist somewhere in the Cuelist into the current Cue. This is the initial default. |
| **Add New** | Merge all applicable programmer addresses into the current Cue, including addresses new to the Cuelist. |

Preset and Group targets offer **Update Existing** and **Add New**. Preset eligibility is per exact fixture/attribute address. For a Group, existing-only retains its ordered membership and does not introduce a fixture; add-new appends selected fixtures according to normal ordered Group Merge behavior without implicit removal or reordering.

**Update Update** initially shows **Eligible for Update Existing**. Switch to **Show All Active** to include active targets that would otherwise be no-ops and choose Update Existing or Add New per target. Distinct playbacks keep their concrete current-Cue context even when they share one Cuelist. No-op rows cannot report success.

Update Settings stores desk workflow preferences, not show programming. It controls the Cue, Preset, and Group defaults and whether touch opens the modal. A confirmed Update performs one revision-checked show mutation, retains the programmer like Record, and reports the changed object, changed Cue/source events, ignored values, and new revision. A preview also fingerprints the exact programmer contents and live playback/current-Cue context it displayed; changing either before confirmation rejects the stale operation and writes nothing. The single resulting object revision is one Undo step. Missing, ambiguous, stale, or empty targets fail atomically.

### Cuelists, Cues, and playbacks

Cuelist and Cue selection uses one unambiguous address grammar. A playback is the page slot containing the fader and buttons; a Cuelist is the ordered collection of Cues assigned to that playback.

The logical `[LINK]` command key is available to attached OSC control surfaces as `programmer/link`; on a computer keyboard press `[KBD:SHIFT]` + `[KBD:L]`.

Press `[KBD:SHIFT]` + `[KBD:Z]` to enter `SELECT`, then touch a playback to make it the selected playback. The selection is retained for that desk and show: another session attached to the same desk sees the same selection, while another desk used by the same operator may select a different playback. Running a different playback never changes it implicitly. The selected playback supplies the default Cuelist whenever a command omits both a playback address and a Cuelist Pool number. It is also the playback whose Cue details open with `[SHIFT] 4`.

- `[SET] <Cuelist-number>` selects a Cuelist.
- `[SET] <Cuelist-number> [CUE] <Cue-number>` selects a Cue in that Cuelist.
- `[SET] <playback-page> [ . ] <playback-number>` selects a playback by its page position.
- `[SET] <playback-page> [ . ] <playback-number> [CUE] <Cue-number>` selects a Cue in the Cuelist assigned to that playback.

| Target | Command | Result |
| --- | --- | --- |
| Go To on selected playback | `[CUE] <Cue-number> [ENTER]` | Make the Cue current immediately, activate an Off playback, set its fader to full, and use normal tracked state, timing, arbitration, Grand Master, and Blackout. |
| Load on selected playback | `[CUE] [CUE] <Cue-number> [ENTER]` | Mark the Cue as the loaded next Cue without changing current output, activation, or fader level. The next forward GO consumes it. |
| Explicit Go To | `[CUE] [SET] <Cuelist/playback-number> [CUE] <Cue-number> [ENTER]` | Go To on the concrete addressed playback. Page form: `[CUE] [SET] <page> [ . ] <playback> [CUE] <Cue> [ENTER]`. |
| Explicit Load | `[CUE] [CUE] [SET] <Cuelist/playback-number> [CUE] <Cue-number> [ENTER]` | Load on the concrete addressed playback. The same page form is accepted after the two initial Cue keys. |
| Cue on the active playback | `[REC] [CUE] <Cue-number> [ENTER]` | Record the numbered Cue in the Cuelist assigned to the active playback. The omitted playback/Cuelist address resolves only through the explicit active-playback selection. |
| Cuelist | `[REC] [SET] <Cuelist-number> [ENTER]` | Create a Cuelist in an empty pool slot, or append a Cue to an existing Cuelist. The Cuelist remains unassigned. |
| Specific Cue | `[REC] [SET] <Cuelist-number> [CUE] <Cue-number> [ENTER]` | Record at the specified Cue number. |
| Page playback | `[REC] [SET] <page> [ . ] <playback-number> [ENTER]` | Append a Cue to the Cuelist assigned to that playback. |
| Cue on a page playback | `[REC] [SET] <page> [ . ] <playback-number> [CUE] <Cue-number> [ENTER]` | Record at a specified Cue in the assigned Cuelist. |
| Cue with explicit fade | `[REC] [SET] <Cuelist-number> [CUE] <Cue-number> [TIME] 3 [ENTER]` | Record the Cue with a three-second default fade while retaining per-value timing overrides. |
| Cue with FOLLOW trigger | `[REC] [SET] <Cuelist-number> [CUE] <Cue-number> [TIME] [TIME] 0 [ENTER]` | The second consecutive Time becomes `DELAY`; zero, or `DELAY` confirmed without a number, stores FOLLOW. This Cue starts when the preceding Cue has finished all value delays and fades. |
| Cue with TIME trigger | `[REC] [SET] <Cuelist-number> [CUE] <Cue-number> [TIME] [TIME] 4 [ENTER]` | Store `DELAY 4`, displayed as a TIME trigger of four seconds. This Cue starts four seconds after the preceding Cue receives GO. |
| Link from one Cue | `[LINK] [SET] <Cuelist-number> [CUE] <source-Cue> [AT] [CUE] <destination-Cue> [ENTER]` | Store the destination Cue's stable identity on the source Cue. After the source completes, playback jumps to that identity. Omit SET and the Cuelist number to use the selected playback. Page form: `[LINK] [SET] <page> [ . ] <playback> [CUE] <source> [AT] [CUE] <destination> [ENTER]`. |
| Delayed Link | `[LINK] [SET] <Cuelist-number> [CUE] <source-Cue> [AT] [CUE] <destination-Cue> [TIME] [TIME] 2 [ENTER]` | Add a two-second Link delay after the source Cue's latest actual incoming/outgoing completion. |
| Merge into a Cue | `[REC] [+] [SET] <Cuelist-number> [CUE] <Cue-number> [ENTER]` | Add the programmer's fixture/group attribute addresses to the existing Cue; an incoming address replaces the value already stored at that same address. |
| Subtract from a Cue | `[REC] [-] [SET] <Cuelist-number> [CUE] <Cue-number> [ENTER]` | Remove the fixture/group attribute addresses currently present in the programmer from that Cue. Values at all other addresses remain unchanged. |
| Delete a Cue with Record-minus | `[REC] [-] [SET] <Cuelist-number> [CUE] <Cue-number> [ENTER]` with no programmer values | Delete that Cue. The only Cue in a Cuelist cannot be deleted this way. |

Dots after `[CUE]` form decimal Cue numbers. For example, `[REC] [SET] 1 [CUE] 2 [ . ] 5 [ENTER]` records Cue `2.5` in Cuelist 1. The `Cues · Cuelist1` view can renumber the Cuelist later. A fully entered command uses its explicit operation without opening a confirmation dialog. Clicking an existing Cuelist pool cell records the next Cue; it does not target an existing Cue. Use the complete command-line address above to overwrite, merge, subtract, or delete a specific Cue.

The two initial Cue keys are the operation: one means Go To and two consecutive keys mean Load. The later Cue key after `SET ...` is only the address separator. Load is transient and visibly replaces the ordinary next Cue; GO minus preserves it, while Off or release clears it. Renumbering follows the Cue's stable identity, deleting the loaded Cue clears the override, and reopening the show does not persist a Load. A missing selection, missing Cue, incomplete address, unassigned target, or ambiguous Cuelist assignment is rejected without moving any playback or fader.

A Cue-record command without `DELAY` stores the Cue with a **GO** trigger, so it waits indefinitely for GO. Bare `DELAY` and `DELAY 0` normalize to **FOLLOW**. A positive `DELAY <seconds>` stores **TIME** with that duration. The trigger belongs to the Cue being recorded: FOLLOW starts after the preceding Cue's latest value `start delay + fade` endpoint, while TIME counts from the preceding Cue's GO. A Cue 2 set to TIME 4 can therefore begin four seconds after Cue 1's GO even while Cue 1 still fades.

LINK is an explicit Cue edit and captures no programmer values. Its source and destination numbers are resolved inside one Cuelist when Enter is pressed; the stored destination is the Cue's stable identity. Renumbering keeps the Link, while a missing destination, self-link, or cycle rejects the complete command without changing the show or output. LINK timing starts after the source Cue's latest actual incoming/outgoing completion. A delayed Link uses the same `DELAY` entry produced by pressing Time twice. Live Timecode is authoritative and suppresses Link execution until Timecode is absent.

The Cuelist setting **Force Cue Timing** makes each Cue's master Fade and Delay authoritative for every value during playback, ignoring stored per-value fades and start delays without deleting them. When the setting is disabled again, the original per-value timing applies on the next execution.

The Cuelist setting **Disable Cue Timing** is a rehearsal bypass. It treats per-value and Cue Fade/Delay, TIME-trigger waits, and the effective Chaser X-fade duration as zero without rewriting them. Chaser X-fade remains stored as its `0–100%` share of the current Speed Group/BPM/multiplier step, and Chaser step cadence remains active. Disable Cue Timing takes precedence over Force Cue Timing; turning it off restores every configured duration.

## Deleting, moving, and copying

### Groups

| Action | Command | Result |
| --- | --- | --- |
| Delete | `[DEL] [GRP] <group-number> [ENTER]` | Delete the Group if no derived Group depends on it. This is equivalent to empty-selection `[REC] [-] [GRP] <group-number> [ENTER]`. |

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

Deleting the active Cue removes it from the stored Cuelist but holds its fully reconstructed output until another playback action occurs. GO executes the next surviving Cue; GO minus executes the previous surviving Cue. Navigation then reconstructs tracking from the modified Cuelist, so values introduced only by the deleted Cue release according to the destination Cue's timing. Deleting the sole Cue remains prohibited.

## Assigning and configuring playbacks

On the touch UI, press `[SET]`, tap an existing entry in the Cuelist Pool, then tap the target playback fader. The selected Cuelist replaces the current assignment at that page position. To create a Group Master with the same physical workflow, press `[SET]`, tap the source Group tile, then tap the target playback. The command line remains armed as `SET GROUP <number>` between the two touches so the destination is explicit.

Press `[SET] [GRP] <Group-number> [ENTER]` to open that Group's settings. Enter is significant:
while `SET GROUP <number>` is pending, pressing a physical or Virtual Playback assigns that explicit
Group to that explicit Playback instead. Pressing `[SET]` and a Playback without first choosing a
Group always opens Playback Configuration, regardless of the current fixture or Group selection.
`[CLR]`, `[ESC]`, a show/desk change, or leaving the originating surface cancels the pending source.

In the Tauri app and browser UI, right-clicking an element is a shortcut for pressing `[SET]` and then left-clicking that same element. Use it wherever `[SET]` followed by a click configures an element, edits a SET-only value, renames an entry, or starts a SET assignment; the native context menu does not open. This includes Presets, Cuelist and Group assignment sources, Dynamics, Playback Page rename, File Manager rename, compact Cue values, and editable Patch values. A populated Cuelist is the deliberate exception to SET-source routing: right-click opens that exact Cuelist's settings, matching its hold action. Where no desk action is assigned, right-click does nothing and browser Reload, Inspect Element, and Autofill menus remain suppressed. On a touchscreen, continue to press `[SET]` and then tap the element.

Right-click follows the SET-click action even when another gesture opens settings. For example, right-clicking a Group chooses it as an assignment source, while touch-hold or `[SET] [GRP] <Group-number> [ENTER]` opens Group settings. Right-clicking a Dynamic chooses it as an assignment source, while Shift-click or touch-hold opens its editor.

To configure an assigned page playback, press `[SET]` and then tap the playback, press `[SHIFT]` and then its first button, or right-click anywhere on the playback. All three gestures open the same Playback configuration modal. **Unassign Playback** removes the Cuelist or Group from that page position and leaves the playback slot empty.

| Action | Command | Result |
| --- | --- | --- |
| Assign a Cuelist | `[SET] <Cuelist-number> [AT] <page> [ . ] <playback-number> [ENTER]` | Assign a Cuelist to a playback on a page. |
| Open Group settings | `[SET] [GRP] <Group-number> [ENTER]` | Open General, Projection, and Phase settings for that explicit Group. |
| Assign a Group Master | `[SET]`, choose the Group, then press the destination Playback | Create or replace that physical or Virtual Playback with a Group Master targeting the explicit Group. |
| Configure a Cuelist | `[SET] <Cuelist-number> [ENTER]` | Open the Cuelist configuration. |
| Configure a page playback | `[SET] <page> [ . ] <playback-number> [ENTER]` | Open the configuration for the playback at that page position. |

### Starting a Macro

`MACRO <pool-number>` starts that show-owned Macro through the same authenticated, desk-scoped execution queue as a pool tap or assigned Playback. The command returns after the one-shot execution is queued; the Macro and Running windows show its live result. A Macro body cannot use `MACRO` to call another Macro, because Macro lines deliberately contain only non-interactive ordinary programming commands and have no call or recursion feature.

Inside a Macro source document, either a newline or `;` ends a command; the final semicolon on a line is optional. `DEFINE _name <expansion>` declares an underscore-prefixed, no-space identifier whose expansion can contain arbitrary command text. The editor validates the expanded command through this same authoritative grammar and shows the expansion when the identifier is hovered. `RESTORE SELECTION` restores the concrete ordered fixtures selected when that individual Macro run began; it does not store or recall a Group. `DEFINE` and `RESTORE SELECTION` are Macro-source instructions, not commands for ordinary interactive entry.

## OSC playback addressing

Every keypad key is also accepted at `/light/{desk}/programmer/{key}` with a pressed value. Inputs include `playback`, `off`, `diff`, `page-up`, `page-down`, `minus`, `time`, `shift`, and `digit-0` through `digit-9`. OSC Shift follows the same shifted action labels; a held Shift remains active until release. Existing inputs such as `plus`, `at`, `thru`, `set`, `record`, `enter`, and `backspace` continue to use the same address family.

The desk alias scopes interaction, not ownership of programmer values. A Tauri or browser desk and the OSC controllers subscribed to its alias share one in-progress command line, page, and button state, so a physical key continues the command visible in that desk UI exactly as an on-screen key would. Different desk aliases keep those partial interactions separate. After a command is completed, its values land in the logged-in user's programmer and are therefore visible in every session for that same user, including sessions attached to other desks.

- `/light/{desk}/page-playback/{playback}/{fader-or-button}` addresses a numbered playback on the page currently active for that desk or screen.
- `/light/playback/{page}/{playback}/{fader-or-button}` addresses that page and playback globally, independent of every desk's current page.
- `/light/cuelist/{Cuelist}/{action}` directly operates a Cuelist when a page playback is not the intended target.

The hardware simulator uses `page-playback`. The former `paged-playback`, `/light/qlist/{number}/{action}`, and direct `/light/playback/{Cuelist}/{action}` forms remain compatibility aliases for existing integrations.

### OSC Dynamics

Dynamics OSC is scoped to a subscribed desk alias. Pool actions take one pressed Boolean or numeric
value; release values are ignored:

- `/light/{desk}/dynamic/{pool-number}/toggle`
- `/light/{desk}/dynamic/{pool-number}/off`

Live-instance actions use the runtime instance UUID from feedback and one numeric argument:

- `/light/{desk}/dynamic/instance/{uuid}/size` — normalized `0.0` through `1.0`
- `/light/{desk}/dynamic/instance/{uuid}/speed` — positive multiplier
- `/light/{desk}/dynamic/instance/{uuid}/phase` — finite degrees

`/light/{desk}/programmer/fix-at` takes an attribute-name string followed by one normalized numeric
value and uses the desk's current selection. Definition editing is not exposed through OSC.

Rejected actions return
`/light/{desk}/feedback/dynamic/error <original-address> <message>`. Subscription snapshots publish
`global-paused`, `runtime-count`, and one
`runtime/{runtime-uuid}/{active|pool-number|name|target-count|controller-count|winning-controller|paused}`
family per running instance. Each controller publishes
`controller/{controller-uuid}/{runtime-instance|source|priority|size|speed|phase|paused|winning|releasing}`.
Programmer-owned summaries remain under `feedback/dynamic/instance/{uuid}` and
`feedback/dynamic/{pool-number}/active`. Treat each refresh as authoritative: `runtime-count` and
the identities present in that snapshot replace a locally cached list.
