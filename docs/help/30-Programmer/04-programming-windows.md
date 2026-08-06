# Programming Windows

Use the window that gives the clearest view of the current task; all of them operate on the same user programmer.

| Window | Primary use |
| --- | --- |
| Stage | Spatial selection, 2D/3D visualization, and Preload following. |
| Fixtures | Fixture/head rows, attributes, source ownership, ordering, and active/Cuelist filters. |
| Channels | Channel-oriented value and source inspection. |
| Groups | Reusable ordered selections, references, projection, and phase ranking. |
| Presets | Mixed, Intensity, Color, Position, and Beam pools. |
| Dynamics | Numbered Dynamic pool, Lanes, Phase, Speed, targets, and running state. |
| Cuelists / Cues | Cue content, order, timing, triggers, tracking, and execution. |
| DMX | Final universe output and diagnostic overrides. |

Selections made in Stage, Fixtures, Groups, Presets, or the command line are the same actual programmer selection. While using [Highlight and Step Through](02-selecting-and-setting-values.md#highlight-and-step-through), PREV and NEXT replace that actual selection with one item, ALL restores the current membership of the remembered live source, and any ordinary selection from one of these windows becomes the new complete step basis. Preset, encoder, dialog, and other value changes use the actual selection but do not reset the basis. HIGH remains independent and follows whatever is actually selected.

In the Dynamics Speed view, choose **Loop** to repeat until Off or **One-shot** to run one complete effective cycle and stop automatically. Run Mode is separate from Start now, Join sync now, and Next boundary, which determine when and where the cycle begins. A completed one-shot does not restart merely because its Cue, Programmer, or Playback value remains active; trigger it again with a new deliberate activation.

The Dynamics editor separates target ordering from phase distribution. **Projection** places
fixtures in a 2D plane and nothing else; **Phase** turns that plane into the one-dimensional order
each lamp takes its phase from.

**Projection** starts from the saved live Group's mapping. **Inherit** follows the Group;
**Override** enables the local controls and a **Projection** dropdown choosing Planar, Cylindrical,
or Spherical. Each kind offers only the values it uses, and a simplified Stage view shows the centre
point, the axis, and where the spread starts. Changes are saved as they are made, so there is no
Apply button; if the Dynamic changed elsewhere, authority reloads and your values are kept. Frozen
or targetless Dynamics instead show **Selection order (no Group mapping)**. On the encoders,
projection takes two pages: the kind and its position, then its orientation.

**Phase** owns the ordering — **Inherit**, Linear, Grid, Radial, Radar, or Random — alongside
offset, span, anchors, blocks, repeats, wings, and uniform/per-lane phase behavior. **Inherit**
takes the Group's ordering and appears only for a Dynamic bound to a live Group, because anything
else has nothing to inherit from. **Random** ignores fixture positions and projection.

With an empty programmer selection, the first ordinary tap on a populated Preset selects every
fixture or logical head for which that Preset stores a value; it does not recall values. Tap the
Preset again to recall it onto that selection. The selection is immediately shared with Stage,
Fixtures, the command line, OSC, and attached controls. Unpatched fixtures remain selectable.
Targets that no longer exist are skipped and reported without substituting another fixture. An
empty Preset slot remains inactive unless a recording workflow is armed. Record/Store, Update, and
Set keep priority over this selection shortcut.

## Return Position fixtures home

Open **Position → Special Dialog** and press **Return Home** beside the relative-position controls to return the current ordered selection to its fixture-profile Position defaults. Each selected logical head uses its own Pan and Tilt defaults; a missing default falls back independently to 50%. Fixtures without the corresponding Position attribute are skipped. With no selection, Return Home is disabled and never addresses every moving light in the show.

Return Home is one normal programmer gesture. It follows Programmer Fade and the current Blind, Preview, or Preload mode, and one **UND** restores the preceding programmer values. Record or Update the result when it should become show data. Return Home itself does not edit fixture-profile defaults or save values into a Cue or Preset.

## Run fixture control actions

Open **Control → Special Dialog** to run the selected fixtures' authored control actions. The familiar Lamp, Reset, and Fan buttons apply only where a fixture profile provides the matching action; the Fixture controls row exposes every authored action by its profile name, including Custom actions. Momentary actions remain active only while held, timed actions release on the fixture profile's timer, and latched actions toggle on and off.

Control actions are live fixture overrides, not recordable encoder values. Use **Generate portable presets** in the same dialog when fixed or indexed fixture functions should become portable Preset choices for the selected fixtures.

## Align a Color range

Open **Color → Special Dialog** to apply the picker's chosen color uniformly to the current selection. To create a range, hold Shift on the normal keyboard or attached hardware while pressing a start point, drag to the end point, and release. The preview line and endpoint markers show the active range without covering the picker.

The first selected fixture receives the start color, the last receives the end color, and intermediate fixtures receive equal steps in the current selection order. Horizontal hue follows the visible drag direction directly rather than wrapping around the color wheel. Every step uses the displayed Brightness. Reversing the selection reverses which fixtures receive the steps; fixtures or logical heads without compatible RGB or CMY Color attributes are skipped without changing the spacing.

The complete range lands once on release as one normal Programmer Fade and Undo gesture. Leaving or cancelling the pointer gesture applies nothing. Blind, Preview, Preload, Record, and Update use the same programmer behavior as other Color edits.

The Fixture Sheet is also the on-desk step-state view: remembered-base rows remain subtly selected while the actual step is prominent, including on multi-head rows and master rows shown while subheads are hidden. The indication remains with HIGH off. The command bar replaces its DMX-rate text with `Highlight` while HIGH is active but adds no separate status panel; neither does the hardware simulator.

Pane settings are local to that pane. A Stage pane can follow Preload while another shows live output; a Preset pane can remain on Position while another shows Color.

See [Channel Faders](05-channel-faders.md) for the current Channels workflow. Use Dynamics for
animated Programmer values and Dynamic Playback assignments; use a Live or Preload-following Stage
window to inspect their authoritative output.
