# Position and aiming

## Position encoders and special dialog

Position programs fixture-independent angles and targets instead of raw Pan and Tilt channel levels. A show that still holds Pan and Tilt as channel percentages is not opened; see [Shows programmed before fixture-independent programming](../10-Show-Setup/05-users-sessions-and-recovery.md#shows-programmed-before-fixture-independent-programming).

**Encoders.** Position has two encoder pages. Page 1 holds **Pan** and **Tilt** in degrees; page 2 holds **Point**, **X**, **Y** and **Z**, with the offsets in metres. Press the **Position** family button again to switch pages. Switching pages only changes what the encoders show; it never activates Angle or Target and sends nothing. Software encoders, the keyboard, attached hardware and OSC `encode/N` controls all make the same edit: up and down move one step (1° or 0.1 m), and the coarse direction moves ten. Typing a range such as `270 [THRU] −270 [THRU] 270` in an encoder's value modal spreads that angle or offset over the ordered selection, exactly as an intensity range does; the encoder then shows the range, for example `-270°...270°`.

**Readouts.** **Point** shows the active Target reference: **Origin** or the 3D Point's name. **X**, **Y** and **Z** show the Target offsets in metres, for example `0.3 m`. At the Origin these are stage coordinates; at a Point they are relative to the Point. The values update with every encoder step, on screen and on an attached hardware desk. They read the same whether you selected fixtures or a group, and whether the fixtures are patched or not. When the Programmer holds no Position but a Cue or Preset aims at a Target, the encoders show that Target. Before any Target is active, Point, X, Y and Z read **—**; while the show holds no 3D Point, Point reads **No Points** instead. When the selected fixtures hold different numbers, the encoder shows the lowest and highest, for example `-12.5°...31°`.

**Choosing and creating Points.** Turning **Point** steps through **Origin** and then every 3D Point of the show in fixture ID order, patched or not, and back to Origin; the encoder names the chosen Point by its fixture ID and name. Tap the middle of the Point encoder, or touch the Point display on a hardware-connected desk, to pick a Point by name; the current one is marked. The picker also offers **Create Point**, which opens **Show > Show Patch > Points** with a new aim Point, and **Manage Points**, which opens the same view (see [Points](../../10-ToskLight-Architect/10-tracking-points.md#points)). A Point that was deleted while the Programmer, a Cue or a Preset still aims at it reads **Missing point**: the reference is kept, never silently replaced, and the next Point step goes on to the Points that exist.

Angle and Target are exclusive. Turning Point, X, Y or Z while the selection holds Angles, or has no semantic Position value yet, activates Target at the Origin, or at the chosen Point, and applies that offset in the same edit. Turning Pan or Tilt while Target is active switches back to Angles.

## Enter angles from the command line

`1 [AT][^3] 45 [DIV] 30 [ENT]` sets fixture 1 to Pan 45° and Tilt 30°. Use `[-][-]` for a negative absolute angle; a single `[-]` means subtraction. See [non-intensity values](01-command-line.md#setting-non-intensity-values) for omitted components and ranges.

Use the [Position special dialog](31-position-special-dialog.md) for the Pan circle, Tilt fader, Aim joystick, and Return Home.
