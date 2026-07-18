# Playback Configuration Implementation Contract

This document defines the implemented playback-assignment and control-layout feature. Its behavioral acceptance scenarios run as executable Playwright coverage in [`tests/07-playback-configuration.spec.ts`](../../../tests/07-playback-configuration.spec.ts).

## Opening Playback Configuration

`[SET]` followed by touching any part of a playback opens one **Playback Configuration** modal for that playback. Valid targets include every physical or simulated button, the fader track or handle, the playback representation in the software, and a Virtual Playback cell. While Set is armed, the touched control identifies the playback only:

- a button must not execute its assigned action;
- a fader touch or drag must not change output or the stored fader level; and
- touching an empty playback must still open its configuration.

All entry surfaces address the same page/playback identity and open the same persisted configuration. The title-bar Close button changes nothing.

Virtual Playback Pane Settings contains Rows and Columns, not a separate per-cell assignment list. Assign a virtual target through **Set Source**, **Add Target**, or the normal `[SET]`, source, target sequence. A direct `[SET]` followed by a Virtual Playback cell opens this same modal. The virtual target reports one button and no fader, and additionally exposes an icon or image-background choice in its presentation settings.

## Modal structure

The modal has three tabs:

1. **Function** uses a reusable two-column Selection Tree built from independently scrollable Selection Lists: the left list chooses Cue List, Group Master, Speed Master, Special, or None; the right list exposes the choices belonging to that family and fills the available column height. These choices are visible list items, not dropdowns. An empty-list message occupies exactly the same first-row position and dimensions as an available option. Playback name and playback color occupy a compact full-width section below both columns without an additional section heading.
2. **Behavior** explains what happens when Flash or Swap is released, when another playback takes full control, and how Protect from Swap works.
3. **Layout** assigns the available physical/software buttons and fader for that playback's actual hardware topology.

The three-tab strip runs edge to edge inside the modal. The title already identifies the page/playback address, so the modal does not repeat a Page, Playback, button-count, or fader summary row below it.

**Apply** is in the modal title bar. It is enabled only while the normalized draft differs from the configuration that was opened; reverting every field disables it again. Clearing an already empty slot is not a change. There is no footer Cancel action because the standard title-bar Close button already dismisses the draft without changing the playback.

Playback color uses the reusable regular input-shaped color dropdown. Its closed state shows the active color across the full input. Opening it creates a padded, viewport-positioned overlay outside the modal's document flow, so it does not resize the modal. The overlay exposes the approximately 16 prominent, high-contrast colors across the full field width, normally in two rows and wrapping to additional rows when space is narrow. The chosen color persists and drives both the background LED color of the playback's hardware buttons and the corresponding software playback representation. Playback Layout has no separate color field.

Choosing the red **None** function makes the options column inactive and previews that the playback will be cleared. The clear occurs only when the operator chooses title-bar **Apply**; Close remains mutation-free. Applying None removes the page/playback assignment and its playback-specific layout, color, and behavior settings, returning the slot to the normal empty defaults. It stops and releases that playback contribution, but it must not delete the assigned Cuelist, Group, or other referenced show object from its pool. None plus Apply is the explicit clear confirmation.

## Assignable playback functions

Function offers these visible assignment families:

- **Cue List**, with a Cue List selector;
- **Group Master**, with a Group selector;
- **Speed Master**, with a selector for Speed Group A, B, C, D, or E;
- **Special**, with Programmer Fade, Cue Fade, and Grand Master choices; and
- **None**, which clears the playback when applied.

Special and None are UI groupings only. Existing persisted target discriminants remain unchanged, so legacy shows load without schema migration.

“Speed Master” is the playback function controlling one of the existing Speed Groups A–E; do not create a second duplicate persisted speed-object family unless a future requirement gives it distinct semantics.

Changing the function immediately changes the choices offered by Playback Layout. An incompatible prior button or fader assignment must be replaced by that function's defaults rather than retained invisibly. The configuration model records only controls physically present on that playback: a one-button playback has one assignment, a two-button playback has two, and a faderless playback has no fader mode.

## Cuelist controls

The default three-button Cuelist layout is:

| Control | Default |
| --- | --- |
| Top button | **GO −** |
| Middle button | **GO +** |
| Bottom button | **Flash** |
| Fader | **Master** |

Each available Cuelist button opens a dedicated choice modal rather than a dropdown. Every choice shows its visible function label and a short explanation. Functions are grouped by operator intent:

- **Step Control** contains GO +, GO −, FFW, FRW, and Pause.
- **Permanent State** contains On, Off, and Toggle.
- **Temporary State** contains Flash, Temp, and Swap.
- **Selection** contains Select and Select contents, plus Select dereferenced where that function is available.

Speed-specific functions use **Speed Control**, and Grand Master functions use **Grand Master Control**. Fader choices use the same explanatory-modal pattern, grouped as **Level Control**, **Cue Transition**, or **Speed Control** where applicable.

An empty assignment is not shown as a Disabled function among the button choices. **Empty Button** appears in the choice modal title beside Close and clears that physical button's assignment. The Layout field then reads **Empty Button** and uses a subdued, dashed treatment so it cannot be mistaken for an assigned action. Available button functions include:

- **GO +**: advance to the next Cue using configured timing.
- **GO −**: return to the previous Cue using configured timing.
- **FFW**: advance to the next Cue with all fades and delays bypassed for that transition.
- **FRW**: return to the previous Cue with all fades and delays bypassed for that transition.
- **On**: activate the playback at a virtual playback level of 100%, regardless of the current physical fader position, and enter the first or remembered Cue according to the Cuelist Restart mode.
- **Off**: release the playback while retaining the physical fader position. To take control again from the physical fader, the operator must move it fully to zero and then raise it.
- **Toggle**: alternate normal On and Off behavior on successive presses.
- **Flash**: apply the playback only while held.
- **Temp**: toggle a temporary playback contribution on with one press and off with the next.
- **Swap**: behave like Flash while temporarily forcing every other unprotected playback to zero.
- **Select**: make this the explicitly selected playback without executing it.
- **Select contents**: select every fixture and live Group reference addressed by any Cue in the Cuelist, in deterministic first-appearance order, without executing or changing the Cuelist.

The Cuelist fader offers:

- **Master**, which continuously scales that playback's intensity output;
- **X-fade**, which manually progresses from the current Cue to the next. Reaching the opposite end completes the transition and makes that Cue current. The next transition requires travel in the opposite direction, so successive Cues are taken by alternating full fader travel; and
- **Temp**, which applies the same temporary, non-destructive LTP-stack behavior as the Temp button at a continuously variable level.

Manual X-fade position replaces elapsed Cue fade progress for the transition being controlled; it must not rewrite stored Cue timing. Direction, current/next feedback, and takeover after reopening or reloading must be derived from authoritative playback state.

## Flash, Temp, and Swap ownership

Flash and Temp remain temporary priority-stack entries while they are active. They may win normal HTP/LTP resolution, but they must not trigger the “fully overwritten playback turns Off” rule against an underlying playback. Toggling Temp off or returning a Temp fader to zero always removes only that temporary entry so the previous source becomes authoritative again. Flash release follows the configured mode below.

A Cuelist playback has a persisted **When Flash or Swap is released** two-value toggle, matching the Stage 2D/3D toggle pattern:

- **Release all** switches/releases the flashed playback and removes every attribute contributed by the Flash when the button is released.
- **Intensity only** leaves that playback active at zero intensity while retaining its applicable non-intensity state such as color and position according to normal tracking and arbitration. This is an intentional persistent post-release state, not a hidden temporary entry.

Each Cuelist playback also has a persisted **Turn off when other playbacks take full control** setting, enabled by default. A normal non-temporary playback at full level may automatically switch it Off only when every active attribute address it contributes has been overwritten. Partial overwrite, Flash, Temp button, and Temp fader never satisfy this condition. The full arbitration contract remains covered by MERGE-003.

Swap uses the same held lifetime as Flash, but additionally forces other playbacks to zero for that lifetime. Every playback has a persisted **Protect from Swap** switch. Protected playbacks remain at their resolved level during another playback's Swap. Releasing Swap restores all temporarily suppressed playback levels and does not restart or retrigger their Cues.

## Speed Master controls

The available Speed Master buttons are **Learn**, **Double**, **Half**, and **Pause**. For a three-button playback the default layout is Top = Double, Middle = Half, Bottom = Learn. Double and Half modify the selected Speed Group's current rate, Learn uses tap tempo, and Pause stops phase advancement without discarding the learned rate.

The fader offers three modes:

1. **Direct BPM** maps 0–100% fader travel linearly from 0 to 300 BPM.
2. **Centered relative** uses 50% as the learned speed, slows below 50%, and speeds up above 50%. The exact minimum/maximum multiplier and curve must be fixed in the implementation schema before executable boundary tests are enabled; 50% must always be exactly `1×`.
3. **Learned-speed percentage** maps 0% to Pause, 50% to half the learned speed, and 100% to the learned speed. This is the default Speed Master fader mode.

Changing the fader must update the same Speed Group A–E used by Cuelist Chasers and command-line speed controls. Feedback on every surface must remain synchronized.

## Group Master controls

The Group Master fader is fixed to the assigned Group's intensity master and cannot be reassigned. Its available buttons are:

- **Select**, which selects the live Group reference;
- **Select dereferenced**, which selects the Group's current fixture members as individual fixtures rather than retaining a live Group reference; and
- **Flash**, which temporarily brings the Group Master to full without moving its stored fader level.

These are also the immediate top, middle, and bottom defaults, respectively.

## Grand Master controls

The Grand Master fader is fixed to the global Grand Master. Its top, middle, and bottom defaults are **Blackout**, **Pause Dynamics**, and **Flash**, respectively. Blackout toggles global blackout, Pause Dynamics pauses/resumes effect and Dynamics phase without deleting their configuration, and Flash temporarily brings the Grand Master to full without changing its stored level.

## Programmer Fade and Cue Fade controls

For Programmer Fade and Cue Fade assignments, the fader controls the corresponding time master. Their immediate top, middle, and bottom defaults are **Double**, **Half**, and **Off**. Double and Half scale the current time, while Off sets the time to zero. Their value ranges and units must match the existing dedicated Programmer Fade and Cue Fade controls.

Selecting any playback type immediately replaces the draft layout with that type's defaults: Cue List uses Go Minus, Go Plus, Flash, and Master; Group Master uses Select, Select dereferenced, Flash, and Master; Speed Master uses Double, Half, Learn, and Learned-speed percentage; the two Fade masters use Double, Half, Off, and their corresponding time fader; Grand Master uses Blackout, Pause Dynamics, Flash, and its fixed master fader.

## Persistence and feedback

The assignment target, button mappings, fader mode, color, Flash/Swap release mode, Protect from Swap, and full-control automatic-Off setting are show-persisted and revision-checked. A successful update refreshes hardware LEDs, software labels/colors, button feedback, and fader feedback from authoritative state. Invalid targets, incompatible mappings, and stale revisions are rejected atomically without partially changing a playback.

Legacy shows without these fields load deterministic type defaults. Saving a migrated show writes the new fields without changing the referenced Cuelist/Group data or current playback output.
