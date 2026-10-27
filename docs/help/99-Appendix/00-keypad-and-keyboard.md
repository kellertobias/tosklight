# Keypad and keyboard

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
| `[PBK]`  | Playback   | -                 | Select or target a unified Playback. Repeated presses keep `PBK`. |
| `[REC]`  | Record     | `[KBD:END]`       | Store cues, presets, and groups. Press twice to choose how to record. Hold for record options. |
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

### Second layer assignment

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
| `[^REC]`  | Update        | Updates cues, presets, and groups. Press twice to choose how to update. Hold for update options. |
| `[^PRELD]`| Clear Preload | Clear Preload. |
| `[^ALIGN]`| Align Off     | Turns off Align mode |

For selection, values, and complete examples, see the [programmer command line](../20-ToskLight-Control/20-Programmer-and-Cues/01-command-line.md).
