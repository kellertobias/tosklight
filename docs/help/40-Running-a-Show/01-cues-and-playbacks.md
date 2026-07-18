# Cues and Playbacks

A Cuelist contains ordered Cues. A playback is an operator control assigned to a Cuelist, Group, or specialized master.

![Cuelist and assigned playback controls](../assets/screenshots/cuelist-playback.png)

## Assign controls

Arm **SET**, then choose a playback button or fader. Playback Configuration has three edge-to-edge tabs: **Function**, **Behavior**, and **Layout**. Function uses two scrollable Fixture Library-style lists: choose Cue List, Group Master, Speed Master, or Special on the left, then choose its specific target from the visible list on the right. Playback name and color are in the compact full-width row below both lists. Behavior's **When Flash or Swap is released** toggle chooses whether release removes all temporary values or leaves the Cue List active at zero intensity with tracked color and position. **Turn off when other playbacks take full control** acts only after normal full-level playbacks replace every value; partial takeover, Flash, and Temp do not count. **Protect from Swap** keeps the playback at its current level while another Swap is held. Layout assigns each available button and fader through a choice dialog that explains the selected function. Button functions are grouped into Step Control, Permanent State, Temporary State, and Selection; specialized masters and faders use corresponding control groups. Choose **Empty Button** beside the dialog Close button to leave that physical button unassigned. Choose title-bar **Apply** to save a changed draft, or Close to discard it; Apply remains disabled when nothing differs from the opened configuration.

Choosing a different playback type immediately loads its standard layout. Cue List uses Go Minus, Go Plus, Flash, and Master. Group Master uses Select, Select dereferenced, Flash, and Master. Speed Master uses Double, Half, Learn, and Learned-speed percentage. Programmer Fade and Cue Fade use Double, Half, and Off above their time fader; Off sets the time to zero. Grand Master uses Blackout, Pause Dynamics, and Flash above its fixed master fader.

Choose the red **None** function and then **Apply** to clear that playback without deleting its referenced Cue List or Group. Selecting None only previews the clear; Close keeps the playback unchanged. Assignment persists in the show and remains page-aware.

## Manage playback pages

Touch the current **Page** control to open **Playback pages** and select an existing page. The keyboard button at the right of each page opens the full-text keyboard to rename that page without selecting it. **Add new page** creates and selects the next numbered page. When the last page already contains an assigned playback, the Next Page control also creates and selects a new empty page automatically; it remains disabled while the last page is empty so the desk does not accumulate unused pages.

To rename the current page, press **SET** and then touch the **Page** control. Enter the new name and choose **Rename Page**. Page names and assignments are stored with the show, while each desk or independently paged screen retains its own current-page position.

## Run Cues

GO advances to the next Cue and applies its tracking state with configured timing. GO minus reconstructs the previous Cue rather than relying on programmer residue. A playback button configured as **Pause** freezes a transition and changes to **Resume** while paused; pressing it again continues the same Cue without advancing. GO also continues a paused transition. Release removes the playback's ownership and permits lower-priority sources to become visible.

The active playback is an explicit operator selection. Running another playback must not silently steal that selection. Cuelist View shows current/next state, Cue detail, and playback configuration.

In the hardware-connected layout, touch any descriptive area of an assigned playback card to select that concrete playback. Cuelist cards also open that playback in Cuelist View; when **REC** is armed, the whole card becomes the Record target, including its playback buttons and fader, and the touched control's normal action is suppressed. Group playback cards select the playback and its Group without opening Cuelist View. Outside Record, the labeled playback buttons and fader remain independent controls: operating them does not also select the card. Selection retains the playback's explicit identity when pages change, even when another page uses the same slot.

## Restart behavior

First and Continue policies determine how a Cuelist starts after release or restart. Looping and chaser modes change end-of-list behavior. Test the exact production policy after a real server/app restart.
