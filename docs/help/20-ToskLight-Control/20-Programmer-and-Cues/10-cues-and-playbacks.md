# Playback operation and pages

## Quick summary

- A **Cuelist** stores the sequence; a **playback** is the button/fader control assigned to run it.
- **GO** runs the next Cue. **Pause/Resume** holds and continues the current transition; GO also resumes a paused transition.
- **In** timing belongs to the Cue you enter; **Out** timing belongs to the Cue you leave and applies to decreasing or released Intensity.
- **Cue Fade** supplies the sequence's fallback fade. **Programmer Fade** controls programming transitions and Preload GO. **Release** supplies Out Fade when that cell is linked to Release.
- Press **CLR twice** before checking Cue output: first clears selection, second clears Programmer values. Check the selected surface's effective page before using an undotted address.

A Cuelist contains ordered Cues. A playback is an operator control assigned to a Cuelist, Group, or specialized master.

![Cuelist and assigned playback controls](../../assets/screenshots/cuelist-playback.png)

## Assign controls

Open [Playback configuration](25-playback-configuration.md) to assign a Cuelist, Group Master, Speed Master, or special control and choose button/fader behavior.

## Manage playback pages

Touch the current **Page** control to open **Playback pages** and select an existing page. The keyboard button at the right of each page opens the full-text keyboard to rename that page without selecting it. **Add new page** creates and selects the next numbered page. When the last page already contains an assigned playback, the Next Page control also creates and selects a new empty page automatically; it remains disabled while the last page is empty so the desk does not accumulate unused pages.

To rename the current page, press **SET** and then touch the **Page** control. Enter the new name and choose **Rename Page**. Page names and assignments are stored with the show, while each independently paged surface retains its own current-page position. [Playback surfaces](14-playback-surfaces.md) explains Own, Pinned, and Follow page sources.

## Run Cues

Desk Settings → Preferences → Defaults supplies the initial Cuelist Auto-off choices and **Start after first recording** for newly created playback/Cuelist/first-Cue transactions. All three defaults are Off for compatibility. Changing a Desk default does not rewrite existing Cuelists. When first-record start is enabled, the new playback starts on its recorded Cue at 100% and the recorded Programmer values clear while fixture selection remains; recording into an existing empty Cuelist or adding later Cues never auto-starts.

GO advances to the next Cue and applies its tracking state with configured timing. Increasing Intensity uses the incoming Cue's In delay/fade; decreasing or released Intensity uses the outgoing Cue's Out delay/fade. For Cue 1 → Cue 2, edit Cue 2's In cells and Cue 1's Out cells. Stored per-value timing takes priority unless Force Cue Timing is enabled; see [Programming Cues](17-cue-timing.md#incoming-and-outgoing-timing). The two directions may overlap or leave a black gap. If GO, GO minus, or Go To interrupts a running fade, the new transition starts from the currently resolved output rather than jumping to a stored Cue endpoint. GO minus reconstructs the previous Cue rather than relying on programmer residue. A playback button configured as **Pause** freezes a transition and changes to **Resume** while paused; pressing it again continues the same Cue without advancing. GO also continues a paused transition. Release removes the playback's ownership and permits lower-priority sources to become visible.

**Flash** and **Temp** play the Cuelist with its Cues' own timing, as GO does: the Cue fades in with its In Fade, and Follow and Time triggers advance the list while it is held. Temp works like a Flash that toggles: one press starts it and the next ends it.

A Cuelist that no longer holds any parameter turns itself off. Build the last Cue to release everything the list set, and once those releases have faded out the playback goes Off by itself. A Flash or Temp holding it ends with it, so the Temp button reads off again and its next press starts the list afresh. This applies to lists that actually held values. It does not apply to a list whose Cue has not set anything yet, a chaser, a paused list, or a list with Group values or running Dynamics.

The active playback is an explicit operator selection. Running another playback must not silently steal that selection. Cuelist View shows current/next state, Cue detail, and playback configuration.

In the Cuelist Pool, touch a Cuelist's preview image to open that actual image at a larger size;
Close, Escape, or the backdrop returns to the pool. **Cuelist Settings → Renumber Cues** assigns
the entered **First new Cue number** to the first Cue, then numbers every later Cue consecutively
without changing their order, names, values, timing, or stable identities. Choose title-bar
**Renumber** to apply the change or Close to cancel it.

On the touch playback surface, the whole playback is a Record target. On an attached desk, only the topmost assigned playback button is a Record target. If the playback has one button, that is the topmost button; if it has no buttons, use its visible on-screen section. The physical fader is explicitly not a Record target, so it can still be moved while recording. The touched target's normal action is suppressed while Record is armed. Outside Record, playback buttons and the fader remain independent controls. Selection retains the playback's explicit identity when pages change, even when another page uses the same slot.

The visible layout never invents controls the playback does not have. A one-button playback exposes one button; a two-button playback exposes two. Increasing its footprint can reveal additional configured controls, but the compact footprint shows only the controls that exist.

For Jump counts and restart policies, see [Playback loops and restart](33-playback-loops-and-restart.md).
