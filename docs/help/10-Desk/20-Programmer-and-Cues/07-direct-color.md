# Direct Color

Direct colour programs a fixture's own colour controls — its emitters, CMY flags or colour wheels — exactly as they are, instead of describing a fixture-independent colour. Use it when only the fixture's own mix will do: a particular LED mix, a wheel slot, or a recipe that has to come back identical on the same fixture type.

This page describes Direct colour inside a [Color Intent](05-color-intent.md) show. It is not the show-wide *Direct* colour model, which keeps the older per-channel colour controls.

Direct colour lives on Color encoder pages 3 and 4 and in the Direct color section on the **Details** tab of the full Color dialog. Pages 1 and 2 stay the ordinary [Color Intent](05-color-intent.md) controls.

## The reference head

Pages 3 and 4 show the controls of one **reference head**, named on every native encoder (for example `Red · 101`) and at the top of the Direct color section (`Reference: 101 · Wash · Main`).

- By default the reference head is the first head in the selection order whose colour layout the desk has verified.
- In the full Color dialog, touch another head under *Reference head* to inspect that head instead. The choice belongs to this desk window only; it is not stored in the show.
- When no selected head has a verified colour layout, pages 3 and 4 are absent and the dialog says so quietly. Pages 1 and 2 keep working.
- A fixture type whose profile authors no colour model, such as a Generic RGB LED, uses the uncalibrated colour layout the desk derives from its colour channels. Its Direct colour works the same way, and it replays exactly on another fixture of the same type and mode.

Each control is named by its attribute, as the attribute list names it — **Red**, **Color Wheel 1**, **Color Temperature** — and shows its current value as a native number at the control's full width (8, 16, 24 or 32 bit). Until you edit, the value is the one the head outputs, and the encoder says **Resolved**. A detent moves 1/255 of the control's range; a coarse turn moves ten times as much, and a typed value is exact. A colour wheel or macro shows its slots as choices, and the encoder shows the active slot's name. Turning it moves one choice per detent, in the order the fixture lists them, whatever the turn size; the value pad and a choice in the Direct color section jump straight to a slot. Changing slot keeps every other control of the recipe exactly as it was. Only a slot with a continuous range (for example a wheel rotation) can be adjusted in place.

On a four-encoder layout Direct is always pages 3 and 4; with Easy controls page 2 is simply empty. Wider encoder layouts continue with the Direct controls after the Color Intent controls.

Pages 3 and 4 hold at most eight controls. Every further control appears in the Direct color section of the full Color dialog, so nothing is hidden.

## Looking never changes anything

Paging to 3 or 4 and back, opening and closing the Color dialog, choosing a reference head, reading values and switching Easy/Advanced change nothing: no Programmer value, no Undo step and no recorded colour.

## The first Direct edit

The first time you turn a native control, the desk takes the reference head's **current output** — the values it showed — once, applies your turn, and switches the whole selection to that Direct recipe in one step. Every further turn of the same control gesture edits that recipe; the output is never taken again. Releasing the encoder ends the gesture as one Undo step.

A head that has no colour programmed shows its fixture defaults, and the first turn starts from those. In Preload, the first turn starts from the head's Preload output instead; until the desk has prepared that preview, the turn does nothing. If the output the encoder showed has moved on, the desk re-reads it and the next detent applies.

## Mixed selections

The whole selection receives the reference head's recipe:

- **Native replay** — heads of the same verified fixture type and colour layout play the recipe exactly, control for control.
- **Best-effort match** — every other head matches the recipe's colour as closely as it can.
- **Native only** — when the recipe's appearance is unknown (for example an unmeasured mix), other heads keep their visible colour instead of guessing; a known UV amount still applies, an unknown one parks UV off.

Native replay means the same controls, not a promise of the same visible colour: the Color approximation shows the measured match separately. Before you edit, the Direct color section shows which heads would replay exactly; afterwards it shows each head's status, UV handling and limitations. None of these rows interrupts you.

## Back to Color Intent

The first edit on page 1 or 2 of a Direct colour turns it back into a Color Intent once:

- The desk starts from the Direct colour's modelled appearance and UV. The Easy controls then show an approximation, and the dialog says *Started from an approximation of the Direct colour*.
- If the Direct colour's appearance is unknown, nothing changes and the Color dialog asks you for a starting colour (*Start from black* or *Start from white*); the Color encoders show *Choose start*. The next turn uses your choice. The desk never assumes white.
- An unknown UV amount starts off.

Presets and Cues keep whatever you record: a Direct recipe stays Direct, a Color Intent stays a Color Intent.
