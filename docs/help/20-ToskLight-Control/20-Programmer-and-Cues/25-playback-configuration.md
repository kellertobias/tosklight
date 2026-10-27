# Playback configuration

Press `[^SET]` with no source object, then choose a playback button or touch area, to open Playback Configuration. The command-line equivalent is `[^SET][PBK] <playback-number> [ENT]`. A regular `[SET]` edits the value or object chosen next; it does not assign. Playback Configuration has **Function**, **Behavior**, and **Layout** tabs.

## Choose the function

Choose **Cue List**, **Macro**, **Timecode**, **Group Master**, **Speed Master**, or **Special** on the left, then its target on the right. **Dynamic** is also available when a stored-target Dynamic can be assigned. Name and color sit below the lists.

Choose the red **None** function to clear the assignment without deleting its target. This is a draft until **Apply**; Close leaves the Playback unchanged. Apply stays disabled while the draft matches its saved configuration.

## Configure behavior

Cuelist behavior settings are shared by every Playback assigned to that Cuelist:

- **When fader reaches zero** turns the Cuelist Off at Master zero. Raising it resumes the same Cue only when that zero gesture caused the Off. X-fade endpoints and Temp do not trigger it.
- **When Flash is released** turns it Off only if Flash started an otherwise Off Cuelist. Releasing Flash leaves an intentionally On Cuelist On.
- **When Flash or Swap is released** chooses whether temporary release removes all temporary values or retains non-intensity state.
- **Turn off when other playbacks take full control** requires every value to be replaced by normal full-level Playbacks. Partial takeover, Flash, and Temp do not count.
- **Protect from Swap** keeps this Playback at its current level while another Swap is held.

## Configure controls

Assign each available button, fader, or push encoder in **Layout**. The choice dialog explains each function. Button actions are grouped into Step Control, Permanent State, Temporary State, and Selection. Specialized masters and faders have their corresponding choices.

Choose **Empty Button** beside Close to leave one button unassigned. **Apply** saves the changed draft; Close discards it. Macro and Timecode assignments use buttons only, without a fader value.

Choosing a different playback type immediately loads its standard layout. Cue List uses Go Minus, Go Plus, Flash, and Master. Group Master uses Select, Select dereferenced, Flash, and Master. Speed Master uses Double, Half, Learn, and Learned-speed percentage. Programmer Fade and Cue Fade use Double, Half, and Off above their time fader; Off sets the time to zero. Grand Master uses Blackout, Pause Dynamics, and Flash above its fixed master fader.

The desk timing strip exposes **Programmer Fade**, **Cue Fade**, and **Release** together on touch and attached control surfaces. Release is the authoritative fallback for Cues whose Out Fade is linked to Release; changing it updates those linked Cues without rewriting their remembered explicit Out Fade values.

For a direct Group Master assignment, press `[^SET]`, touch the source Group tile, then touch the destination Playback. The visible command line remains armed with the explicit Group until that destination completes the assignment.

The [command reference](01-command-line.md#assigning-objects-to-playbacks) gives source and destination forms. Continue with [playback operation and pages](10-cues-and-playbacks.md).

All surfaces use page.position addresses, with pages 1-99 and positions 1-1000. An undotted command uses the visibly selected surface's effective page. [Playback surfaces](14-playback-surfaces.md) covers layouts and page sources. Macro and Timecode assignments provide button actions without a fader value.
