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
- Shift-Press: If a button is to be pressed with shift, we access the second function on that button. These are indicated with `[^GRP]`. You hold down shift, then press the desired button, then let go shift. The action is performed, once you click the actual button. If you need to press multiple buttons with shift directly after each other, you do not need to release shift in between. e.g. `[^GRP][^GRP]` can be either entered by holding down shift, then pressing `[GRP]`, then releasing shift, then holding down shift again, pressing `[GRP]` again, then releasing shift again; or you can hold down shift, press `[GRP]` twice and then release shift; the outcome is the same. The command line again indicates the intention: `[^GRP]` is `#> FIXTURE`, while `[^GRP][^GRP]` is `#> DMX`. Use the DMX form to address the fixture at a physical `universe.address`—a fast way to find it while quick-patching.
- Long-Press/ Hold: If a button is pressed long, it's indicated with a plus, e.g. `[PRELOAD+]` (which opens the pending Preload for inspection and editing). Long presses do not need to be confirmed and directly trigger an action, such as opening a modal or resetting a value.

> [!danger] Missing graphic
> Add a command-grammar diagram showing selection or source, action, target or value, confirmation, and the press, double-press, Shift-press, and hold notations.


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

<!-- table: columns=12,15,18,55; rows-per-page=17; row-weight=1.5; continue-after-table -->
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
| `[GRP]`  | Group      | -                 | Select a group. Hold for showing the group built-in. Press twice for `#> DEGROUP`|
| `[CUE]`  | Cue        | -                 | Select or Target a particular cue. Press twice for `#> CUELIST` |
| `[PBK]`  | Playback   | -                 | Select or Target a particular playback. Press twice for `#> VPBK` (virtual playback) |
| `[REC]`  | Record     | `[KBD:END]`       | Store cues, presets, and groups. Hold for record options. |
| `[PRELD]`| Preload    | `[KBD:^]`         | Run Preload or Preload GO. Hold to inspect and edit the pending Preload. |
| `[CLR]`  | Clear      | `[KBD:DELETE]`    | First Click: Clear Selection, Second Click: Clear Programmer |
| `[DEL]`  | Delete     | -                 | Delete a cue, preset, or other supported element.  |
| `[MOV]`  | Move       | -                 | Move a cue or preset. |
| `[CPY]`  | Copy       | -                 | Copy a cue or preset. Not available on touch only, reach with `[^MOV]` |
| `[SET]`  | Set        | `[KBD:HOME]`      | Edit a value or open the selected object's configuration. |
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

<!-- table: columns=18,22,60; rows-per-page=13; row-weight=1.5; continue-after-table -->
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
| `[^SET]`  | Assign        | Assign a source object to a playback. |
| `[^TIME]` | Speed Group   | Address Speed Groups A-E as `#> SPD GRP 1-5`. |
| `[^DIV]`  | Go To         | Go directly to a Cue on an addressed playback. Press twice for `#> LOAD`. |
| `[^OFF]`  | Release       | Enter Release as a recordable programmer value. |
| `[^MOV]`  | Copy          | Copy a cue or preset. Same as the `[CPY]` button if it is available. |
| `[^REC]`  | Update        | Updates cues, presets, and groups. Hold for update options. |
| `[^PRELD]`| Clear Preload | Clear Preload. |
| `[^ALIGN]`| Align Off     | Turns off Align mode |

### Software (Touch) and suggested Hardware layout

> [!danger] Missing graphic
> Add labelled software-keypad and suggested hardware-layout diagrams showing every primary and Shift-layer button assignment.


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

### Selecting by DMX Address

Press `[^GRP][^GRP]` to enter `DMX`, then enter one physical `universe.address` and confirm with `[ENT]`. For example, `[^GRP][^GRP] 2 [.] 101 [ENT]` displays `#> DMX 2.101` and selects the fixture that owns that slot. Universes are `1` through `65535`; addresses are `1` through `512`.

The address may be the start slot or any slot inside the fixture's occupied footprint. Secondary split patches and every physical multipatch instance are searched too. A multipatch still selects its one logical fixture. For a multi-head fixture, DMX lookup selects all logical heads, exactly like entering the fixture number without a `.head` suffix.

An unpatched, visual-only, or internal fixture never owns a DMX address. If no fixture owns the entered slot, or the address is invalid, the command is rejected and the current selection remains unchanged.

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

### Releasing Values

`[^OFF]` enters **RELEASE**. Release is a programmer value, not the same action as clearing the programmer. The complete form is `<selection> [AT] RELEASE [ENT]`; as a quick command, `[AT]` may be omitted: `<selection> [^OFF] [ENT]`. Without another attribute-family key, Release affects Intensity.

Because Release is a value, it can be recorded into a Cue. When that Cue runs, the Cuelist releases the addressed fixture attribute and stops supplying or tracking a value for it. The desk then resolves the attribute as if that value had never been introduced by the Cuelist, revealing the next applicable source or default.

Choose another attribute group by adding its second-layer family key. For example, `<selection> [^OFF] [^2] [ENT]` releases only the Color attributes of the selection. To release every attribute group, use `<selection> [^OFF] [^0] [ENT]`. Attribute groups that are not part of the Release remain in the programmer or Cuelist unchanged.

For example, if an earlier Cue gives a fixture 100% intensity and a later Cue contains RELEASE for that intensity, the later Cue does not store 0%. It removes the Cuelist's ownership of the intensity entirely. In contrast, `[CLR]` only removes the current programmer entry and does not create a Release instruction that can be recorded.


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

What is in the programmer is what gets stored in a Cue. First select the fixtures and set the values, presets, Dynamics, fade times, and delay times that should be part of the Cue. Values that only come from a running playback, defaults, Highlight, or the resolved output are not added merely because they are visible on stage.

### Recording the First Cue

After setting up the scene in the programmer, press `[REC]`. You can now touch either a Cuelist in the Cuelist Pool or a playback.

Touching an empty Cuelist creates its first Cue. Touching an empty playback creates a new Cuelist, records its first Cue, and assigns that Cuelist to the playback.

On the touch playback surface, the whole playback is the Record target. On an attached desk, only the topmost playback button is the Record target. If the playback has one button, that button is necessarily the topmost button. If it has no buttons, only the playback's visible section on the screen is a Record target. The hardware-fader is explicitly never intercepted by Record, so its level can still be changed while programming the Cue.

If the Cuelist contains exactly one Cue, the desk asks what you want to do:

- **Add Cue** records the programmer as a new Cue at the end of the Cuelist.
- **Merge Cue** adds the programmer contents to the existing Cue and replaces values at matching fixture/attribute addresses.
- **Overwrite Cue** replaces the contents of the existing Cue with the programmer.

The same choice appears when recording onto a playback whose assigned Cuelist contains exactly one Cue. Once the Cuelist contains two or more Cues, recording onto the Cuelist or playback always chooses **Add Cue** and appends at the end.

> [!danger] Missing graphic
> Add a Record-target diagram comparing the touch playback area, the attached desk's topmost playback button, a buttonless playback's visible screen area, and the excluded fader.

### Recording a Cue from the Command Line

Set up the scene in the programmer first, press `[REC]`, and then enter the recording target.

To record directly into a Cuelist, press the Cue button twice to enter `CUELIST`: `[REC][CUE][CUE] <Cuelist-number> [ENT]`. The programmer is recorded as a new Cue at the end of that Cuelist and it selects this cuelist as the active cuelist.

For the next cue, you can omit the Cuelist number and press the Cue button only once: `[REC][CUE][ENT]`. This records a new Cue at the end of the selected Cuelist.

To record onto a playback, use `[REC][PBK] <playback-number> [ENT]`. A playback number without a dot addresses that playback on the current page. To address an explicit page, enter `<page>.<playback-number>`, for example `[REC][PBK] 2.6 [ENT]` for playback 6 on page 2. If the playback is empty, the desk creates a new Cuelist, records its first Cue, and assigns it to that playback. If it already has a Cuelist, the desk adds the new Cue at the end.

For a Virtual Playback, press the Playback button twice: `[REC][PBK][PBK] <virtual-playback-number> [ENT]`. This creates or extends its assigned Cuelist in the same way. The command line shows `#> RECORD vPBK <virtual-playback-number>`

### Recording a Specific Cue

If you want to decide the Cue number, enter the complete Cuelist and Cue address:

`[REC][CUE][CUE] <Cuelist-number> [CUE] <Cue-number> [ENT]`

For example, `[REC][CUE][CUE] 4 [CUE] 2.1 [ENT]` records Cue `2.1` in Cuelist 4. If that Cue already exists, it is overwritten. If it does not exist, it is inserted at the correct position.

For the selected Cuelist, the shorter form is `[REC][CUE] <Cue-number> [ENT]`.

Use `[REC][+]` instead of `[REC]` to merge the programmer into an existing Cue. Use `[REC][-]` to remove the programmer's fixture/attribute addresses from an existing Cue without changing its other values.

### Inserting Cues, Cue Numbers, and Their Order

Cue numbers are not decimal numbers. They are paths made from numeric parts separated by dots, and they can contain as many dots as required. Cue `2` and Cue `2.0` are therefore different Cues.

The desk compares each part in order. A Cue without another part comes before a Cue that continues from the same number. This produces the following order:

`2`, `2.0`, `2.1`, `2.1.0`, `2.1.1`, `2.2`, `3`

This allows more Cues to be inserted between existing Cues without first renumbering the Cuelist. To insert one, record the programmer at an unused Cue number in the required position. If Cues 2 and 3 already exist, `[REC][CUE] 2.1 [ENT]` inserts Cue `2.1` between them in the selected Cuelist. More levels remain available: Cue `2.1.1` can be inserted between `2.1` and `2.2`.

### Selecting Playbacks and Cuelists

There is no separate Select button. To select a playback on the current page, press `[PBK] <playback-number> [ENT]`. To select a playback on an explicit page, press `[PBK] <page> [.] <playback-number> [ENT]`. The part before the dot is always the playback page; the part after it is always the playback number. The selected playback's assigned Cuelist becomes the selected Cuelist. To select a Virtual Playback, press `[PBK][PBK] <virtual-playback-number> [ENT]`.

To select a Cuelist directly, press `[CUE][CUE] <Cuelist-number> [ENT]`. A single `[CUE]` always addresses a Cue inside the selected Cuelist. For example, `[CUE] 2.1 [ENT]` addresses Cue `2.1` in the selected Cuelist, while `[CUE][CUE] 4 [CUE] 2.1 [ENT]` addresses Cue `2.1` in Cuelist 4.

Running a different playback does not silently change the selection.

### Going To and Loading Cues

`[^DIV]` enters **GO TO**. A Go To command must address a playback and then a Cue: `[^DIV][PBK] <playback-number> [CUE] <Cue-number> [ENT]`. When `[ENT]` is pressed, that playback goes directly to the addressed Cue in its assigned Cuelist.

For example, `[^DIV][PBK] 6 [CUE] 2.1 [ENT]` goes to Cue `2.1` on playback 6 of the current page. Use `[PBK] 2.6` to address playback 6 on page 2, or `[PBK][PBK] <virtual-playback-number>` to address a Virtual Playback.

The Cuelist must be assigned to the addressed playback. A Cuelist pool address alone cannot execute Go To because it does not identify the playback that should run the Cue.

Press `[^DIV]` twice to enter **LOAD**: `[^DIV][^DIV][PBK] <playback-number> [CUE] <Cue-number> [ENT]`. Load does not run the Cue immediately. It makes the addressed Cue the selected next Cue for that playback; the next forward GO runs it.

### Cue Timing and Triggers

The individual fade and delay times in the programmer are stored with the Cue. Append `[TIME] <seconds>` to the complete record command to give that Cue a master fade. For example, `[REC][CUE][CUE] 4 [CUE] 2.1 [TIME] 3 [ENT]` records Cue `2.1` in Cuelist 4 with a three-second master fade.

The Cue master fade behaves like a Programmer Fade that belongs only to that Cue. When Programmer Fade is enabled and configured to apply to Cues, its current value overrides the Cue master fade during playback. Press `[TIME]` twice to enter the Cue's trigger delay. The complete GO, FOLLOW, TIME, per-value timing, and Cuelist timing behavior is explained in [Cues and Playbacks](10-cues-and-playbacks.md).

## Updating Existing Programming

Hold `[^]`, press `[REC]` twice, and then release `[^]`. This is written as `[^REC][^REC]` in this manual. It opens the **Update Update** modal with everything that can currently be updated from the programmer. Opening the modal does not update anything by itself; choose the target and Update mode there, or close it without making a change.

### Updating Cues

Use `[^REC]` to Update programming that is already stored instead of recording a replacement. Update reads actual programmer changes only; it does not pull Highlight, defaults, unchanged tracked values, or resolved playback output into the Cue.

After arming Update, touch the Cuelist or playback that should receive the changes, or enter an explicit address with the new grammar:

- `[^REC] [CUE] <Cue-number> [ENT]` updates that Cue in the selected Cuelist.
- `[^REC] [CUE][CUE] <Cuelist-number> [CUE] <Cue-number> [ENT]` updates a Cue in an explicit Cuelist.
- `[^REC] [PBK] <playback-number> [ENT]` updates the current Cue of that playback on the current page; use `<page>.<playback-number>` for an explicit page.
- `[^REC] [PBK][PBK] <virtual-playback-number> [ENT]` updates the current Cue of that Virtual Playback.

The four Cue Update modes decide where each programmer address is written:

- **Update** changes only values that are already stored directly in the current Cue. Inherited values and completely new values are ignored.
- **Tracked** changes each value in the Cue where that value currently comes from. If the current look inherits a value from an earlier Cue, that earlier Cue is updated. Values that do not yet exist anywhere in the Cuelist are ignored.
- **Known** puts the programmer value into the current Cue when that fixture attribute already exists somewhere in the Cuelist. This can bring an inherited value forward into the current Cue, but it does not introduce a completely new fixture attribute.
- **All** puts every applicable programmer value into the current Cue, including fixture attributes that have never appeared anywhere in the Cuelist.

Touch targets open the Update preview with **Update** selected by default and let you choose another mode. A complete keypad command selects the mode before the target:

- `[^REC] <target> [ENT]` uses the default **Update** mode and displays `UPDATE`.
- `[^REC][-] <target> [ENT]` uses **Tracked** and displays `UPDATE TRACKED`.
- `[^REC][+][+] <target> [ENT]` uses **Known** and displays `UPDATE KNOWN`.
- `[^REC][+] <target> [ENT]` uses **All** and displays `UPDATE ALL`.

For example, `[^REC][+][+][CUE][CUE] 4 [CUE] 2.1 [ENT]` adds eligible, already-known addresses to Cue `2.1` in Cuelist 4. The command line displays the full selected mode instead of only the `+` or `-` shorthand.

### Updating Presets

Use `[^REC]` to Update a Preset from the current programmer instead of recording it again. After arming Update, touch the Preset tile or enter its preset-family key and number. For example, `[^REC][^2] 22 [ENT]` displays `#> UPDATE COLOR PRESET 22` and changes only fixture/attribute addresses that are already part of Color Preset 22.

Use **Update All** when programmer addresses that are not yet part of the Preset should also be added. For example, `[^REC][+][^2] 22 [ENT]` displays `#> UPDATE ALL COLOR PRESET 22`. The Update preview shows what will change and what will be ignored before the desk writes the Preset.

## Assigning, Moving, and Copying

### Moving and Copying Cues

Both sides of a Move or Copy use complete Cue addresses. `[AT]` separates the source from the destination:

`[CPY] [CUE][CUE] 1 [CUE] 2.1 [AT] [CUE][CUE] 2 [CUE] 4 [ENT]`

This copies Cue `2.1` from Cuelist 1 to Cue 4 in Cuelist 2. Use `[MOV]` instead of `[CPY]` to remove the source after creating the destination.

When both Cues are in the selected Cuelist, the Cuelist address can be omitted:

`[MOV] [CUE] 2.1 [AT] [CUE] 2.2 [ENT]`

The desk asks whether the operation should use only the Cue's explicitly stored commands and deltas or its complete tracked status. Copy keeps the source Cue. Move removes it and recalculates tracking through the remaining Cues.

### Moving and Copying Objects

Groups, Presets, Cuelists, and other movable or copyable pool objects use the same source-and-destination structure. Press `[MOV]` or `[CPY]`, choose the source object, and then choose the destination. This can be completed entirely by touch, entirely through the command line, or by choosing one side with each input method.

In a complete command, `[AT]` separates source and destination: `<operation> <source-object> [AT] <destination-object> [ENT]`. The source and destination must be compatible object types. A Preset remains in its Preset family, while Cue-specific tracked-status choices use the Cue workflow above.

### Assigning Objects to Playbacks

`[^SET]` enters **ASSIGN**. A regular `[SET]` does not assign anything; it edits a value or opens the selected object's configuration.

If no source object has been chosen, pressing `[^SET]` and then touching a playback button or the playback's touch area opens that playback's configuration modal. The command-line equivalent is `[^SET][PBK] <playback-number> [ENT]`. Use `<page>.<playback-number>` for an explicit playback page, or `[PBK][PBK] <virtual-playback-number>` for a Virtual Playback. Completing the playback address with `[ENT]` opens the same configuration modal.

For touch assignment, press `[^SET]`, choose the source Cuelist, Group, Dynamic, Macro, Timecode, or other assignable object, and then touch the target playback. The same source-then-target order applies on an attached desk.

The command-line form places `[AT]` between source and target. For example, `[^SET][CUE][CUE] 4 [AT][PBK] 6 [ENT]` assigns Cuelist 4 to playback 6 on the current page. `[^SET][CUE][CUE] 4 [AT][PBK] 2.6 [ENT]` assigns it to playback 6 on page 2. Use `[PBK][PBK]` for a Virtual Playback target. The command remains armed between source and target so touch and command-line input can be mixed.

### Setting Values and Opening Editors

A regular `[SET]` acts on the value or object you choose next. Press `[SET]` and then touch an editable value in a table to open its value-entry modal. Depending on the value type, the modal provides the virtual keyboard or virtual number pad required to enter the new value.

In most places, right-clicking an editable value or object is a shortcut for pressing `[SET]` and then touching it. Pressing `[SET]` before a pool object opens that object's editor or configuration instead of changing output. For example, `[SET]` followed by a Dynamic opens the Dynamic editor, while `[SET]` followed by a Timecode opens the Timecode editor. The same rule applies to other editable pool objects where an editor is available.

## Blind Programming & Preload GO

Preload is the desk's blind programming workflow and a tool to perform multiple actions at the same time.

Press `[PRELD]` to enter Preload, then program values or operate the playback domains configured for capture. Live output remains unchanged for captured domains while Preload-aware Fixture Sheet and Stage panes show the prepared result.

Press `[PRELD]` again for **Preload GO**. It applies all captured programmer values and queued playback actions together at one commit point. At that moment, the values move out of the Preload programmer and into a temporary transition queue. The programmer is therefore empty immediately after Preload GO, while the temporary queue remains responsible for completing the transition.

The Programmer Fade value at the moment of Preload GO supplies the transition time for the queued programmer values; changing it afterwards does not alter the running transition. Playback actions with explicit Cue timing retain that timing.

Hold `[PRELD+]` to open the Preload modal. It lists the currently preloaded programmer changes and captured playback actions, and allows individual pending changes to be removed before Preload GO.

Press `[^PRELD]` to clear the complete pending Preload without applying it. After Preload GO has committed the work, the values are no longer pending programmer values; clearing Preload does not cancel the temporary transition queue or playback actions already committed by Preload GO.

Desk Setup independently decides whether programmer changes, physical playback actions, and Virtual Playback actions are captured by Preload or executed live. See [Preload and Preload GO](12-preload.md) for those capture domains and the complete live-output behavior.

## Dynamics and FixAT

Use `[^5]` to open Dynamics or to address a Dynamic preset. A Dynamic continuously modulates one or more fixture attributes over the ordinary programmer or playback value. Starting or stopping a Dynamic does not rewrite that underlying value.

A Dynamic without a fixed Group assignment can behave like a Preset: select the fixtures first, then apply the Dynamic to that current selection. A Dynamic with a fixed Group assignment always uses its stored Group target instead of the current selection.

`[^AT]` enters **FixAT** (Fixed At). FixAT temporarily fixes the active attribute at a static programmer value above a running Dynamic without stopping the Dynamic's clock. When FixAT is removed or cleared, the Dynamic is revealed again at its current phase rather than restarting from the beginning.

For example, select the fixtures and active attribute, then enter `[^AT] 50 [ENT]` to hold that attribute at 50%. An explicit attribute may also be chosen before FixAT. Fade and start-delay clauses work like other programmer values.

FixAT can also use a Preset instead of a numeric value. Enter `[^AT]` first and then select the Preset, either by touching its tile or by entering the normal Preset recall command. For example, `<selection> [^AT] [^2][^2] 22 [ENT]` displays `FixAT COLOR PRESET 22` and fixes the applicable Color Preset 22 values above the running Dynamic. Removing FixAT reveals the Dynamic again at its current phase.

The full Dynamic editor, target ordering, phases, lanes, speed behavior, and the relationship between the static base, Dynamic modulation, and FixAT are explained in [Dynamics](../30-Windows/02-programming-and-visualization.md#dynamics).

## Freezing Fixtures and Fixture Attributes

Freeze captures the resolved output of a fixture or attribute family and keeps it unchanged while later programmer, Cue, Dynamic, direct-control, Group Master, Grand Master, and Blackout values change underneath it.

Press `[^CLR]`, enter a Fixture or Group selection, and press `[ENT]` to apply a full Freeze. A Group resolves to its current fixtures; Freeze is stored on those targets, not on the Group object. Removing Freeze immediately reveals the current underlying state without rewriting it.

Press `[^CLR][^CLR]` to enter **UNFREEZE**, then enter the selection and press `[ENT]`. To Freeze or Unfreeze only specific attribute families, add the corresponding second-layer family keys before `[ENT]`: `[^1]` Intensity, `[^2]` Color, `[^3]` Position, and `[^4]` Beam.

The Fixture Sheet marks a full Freeze as `❄ FREEZE`. A partial Freeze shows the same marker together with its frozen family names. `[UND]` restores the state from before the most recent Freeze or Unfreeze action; Freeze has no separate Redo action.

## Timecodes and Macros

`[^CUE] <timecode-number> [ENT]` runs the addressed Timecode. Add `[+]` before Enter to arm it for autoplay: `[^CUE] <timecode-number> [+] [ENT]`. Add `[-]` instead to disarm its autoplay: `[^CUE] <timecode-number> [-] [ENT]`.

`[^PBK] <macro-number> [ENT]` runs the addressed Macro.

A Macro's lines never pass through the command line. They run against the Programmer as it stands — including the current selection — but they do not type themselves into the command line, clear it, or leave an error on it. Whatever you have half-entered is still there, unchanged, after the Macro has run, which matters because a Macro can also be started by a Playback, OSC, a Schedule, or a Timecode while you are typing.

Inside a Macro source document, put `DELAY <seconds>` on its own source line to wait before the next line runs. The duration is non-negative and may use up to three decimal places, so `DELAY 1.5` waits for one and a half seconds and `DELAY 0.025` waits for 25 milliseconds. This is a Macro execution pause, not a Programmer or Cue timing clause. The Macro is fully validated before its first line runs, and it keeps its desk interaction sequence while waiting, so another command cannot interleave. Output, fades, and Dynamics continue running. The Macro execution display shows the active `DELAY` line and can cancel the wait immediately.

Prefix an address with `[SET]` to edit instead of run it. `[SET][^CUE] <timecode-number> [ENT]` opens that Timecode's editor, and `[SET][^PBK] <macro-number> [ENT]` opens that Macro's editor. The same rule opens a Cuelist: `[SET][CUE][CUE] <Cuelist-number> [ENT]`.

## Speed Groups

`[^TIME]` enters **SPD GRP**. ToskLight has five Speed Groups: command-line numbers `1` through `5` address Speed Groups A through E.

Set an absolute speed with `[^TIME] <Speed-Group-number> [AT] <BPM> [ENT]`. For example, `[^TIME] 1 [AT] 120 [ENT]` sets Speed Group A to 120 BPM, and `[^TIME] 2 [AT] 127.5 [ENT]` sets Speed Group B to 127.5 BPM.

Use `[+]` or `[-]` after `[AT]` for a relative change. `[^TIME] 1 [AT][+] 5 [ENT]` increases Speed Group A by 5 BPM, while `[^TIME] 1 [AT][-] 5 [ENT]` decreases it by 5 BPM.

To synchronize two Speed Groups, address both of them: `[^TIME] 1 [AT][^TIME] 3 [ENT]` displays `SPD GRP 1 AT SPD GRP 3` and synchronizes Speed Groups A and C to the same speed and phase. Directly setting or tapping either synchronized Speed Group breaks that synchronization and returns the changed group to independent control.

A Cuelist, Playback, or Dynamic can use a Speed Group as its authoritative rate. Choose the source first, press `[AT]`, address the Speed Group, and press `[ENT]`:

- `[CUE][CUE] 4 [AT][^TIME] 2 [ENT]` sets Cuelist 4 to Speed Group B.
- `[PBK] 6 [AT][^TIME] 2 [ENT]` sets the object assigned to playback 6 on the current page to Speed Group B.
- `[^5] 29 [AT][^TIME] 2 [ENT]` sets Dynamic 29 to Speed Group B.

The source's own multiplier is applied to the Speed Group rate without changing the Speed Group itself. Selecting a different Speed Group replaces the source's previous Speed Group assignment.
