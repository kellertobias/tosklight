# Playback Configuration Implementation Contract

This document defines the planned playback-assignment and control-layout feature. Its behavioral acceptance scenarios are specified in [`docs/testing/07-playback-configuration.md`](../testing/07-playback-configuration.md). The feature does not exist yet; implement this contract before enabling those scenarios as Playwright coverage.

## Opening Playback Configuration

`[SET]` followed by touching any part of a playback opens one **Playback Configuration** modal for that playback. Valid targets include every physical or simulated button, the fader track or handle, and the playback representation in the software. While Set is armed, the touched control identifies the playback only:

- a button must not execute its assigned action;
- a fader touch or drag must not change output or the stored fader level; and
- touching an empty playback must still open its configuration.

All entry surfaces address the same page/playback identity and open the same persisted configuration. Cancel changes nothing.

## Modal structure

The modal has two tabs:

1. **Playback Function** chooses what the playback represents and exposes settings belonging to that assignment.
2. **Playback Layout** assigns the available physical/software buttons and fader for that playback's actual hardware topology.

The modal also exposes a compact playback color palette and **Clear Playback**. The palette should contain approximately 16 prominent, high-contrast colors; a free-form color picker is not required. The chosen color persists and drives both the background LED color of the playback's hardware buttons and the corresponding software playback representation.

**Clear Playback** removes the page/playback assignment and its playback-specific layout, color, and behavior settings, returning the slot to the normal empty defaults. It stops and releases that playback contribution, but it must not delete the assigned Cuelist, Group, or other referenced show object from its pool. Confirmation is required when clearing a non-empty or active playback.

## Assignable playback functions

Playback Function offers these assignment families:

- **Cuelist**, with a Cuelist selector;
- **Group Master**, with a Group selector;
- **Speed Master / Speed Group**, with a selector for Speed Group A, B, C, D, or E;
- **Programmer Fade**;
- **Cue Fade**; and
- **Grand Master**.

“Speed Master” is the playback function controlling one of the existing Speed Groups A–E; do not create a second duplicate persisted speed-object family unless a future requirement gives it distinct semantics.

Changing the function immediately changes the choices offered by Playback Layout. An incompatible prior button or fader assignment must be replaced by that function's defaults rather than retained invisibly. The configuration model records only controls physically present on that playback: a one-button playback has one assignment, a two-button playback has two, and a faderless playback has no fader mode.

## Cuelist controls

The default three-button Cuelist layout is:

| Control | Default |
| --- | --- |
| Top button | **Go minus** |
| Middle button | **Go plus** |
| Bottom button | **Flash** |
| Fader | **Master** |

Each available Cuelist button may instead be assigned any of:

- **Go plus**: advance to the next Cue using configured timing.
- **Go minus**: return to the previous Cue using configured timing.
- **Fast forward**: advance to the next Cue with all fades and delays bypassed for that transition.
- **Fast rewind**: return to the previous Cue with all fades and delays bypassed for that transition.
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

A Cuelist playback has a persisted **Flash release** setting:

- **Release all** switches/releases the flashed playback and removes every attribute contributed by the Flash when the button is released.
- **Release intensity only** leaves that playback active at zero intensity while retaining its applicable non-intensity state such as color and position according to normal tracking and arbitration. This is an intentional persistent post-release state, not a hidden temporary entry.

Each Cuelist playback also has a persisted **Switch Cuelist off when fully overwritten** setting, enabled by default. A normal non-temporary playback may automatically switch it Off only when every active attribute address it contributes has been overwritten. Partial overwrite, Flash, Temp button, and Temp fader never satisfy this condition. The full arbitration contract remains covered by MERGE-003.

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
- **Flash**, which temporarily brings the Group Master to full without moving its stored fader level; and
- **Select dereferenced**, which selects the Group's current fixture members as individual fixtures rather than retaining a live Group reference.

## Grand Master controls

The Grand Master fader is fixed to the global Grand Master. Its available buttons are **Blackout**, **Flash**, and **Pause Dynamics**. Blackout toggles global blackout, Flash temporarily brings the Grand Master to full without changing its stored level, and Pause Dynamics pauses/resumes effect and Dynamics phase without deleting their configuration.

## Programmer Fade and Cue Fade controls

For Programmer Fade and Cue Fade assignments, the fader controls the corresponding time master. Buttons are disabled and have no assignable or executable action. Their value ranges and units must match the existing dedicated Programmer Fade and Cue Fade controls.

## Persistence and feedback

The assignment target, button mappings, fader mode, color, Flash release mode, Protect from Swap, and fully-overwritten automatic-Off setting are show-persisted and revision-checked. A successful update refreshes hardware LEDs, software labels/colors, button feedback, and fader feedback from authoritative state. Invalid targets, incompatible mappings, and stale revisions are rejected atomically without partially changing a playback.

Legacy shows without these fields load deterministic type defaults. Saving a migrated show writes the new fields without changing the referenced Cuelist/Group data or current playback output.
