# Groups and Presets

Groups store ordered fixture selections. Presets store reusable attribute values.

## Record and select Groups

1. Select fixtures in the required order.
2. Press **REC**, then a Group tile or `[GRP] <number> [ENT]`.
3. Touch the Group later to select its live membership. Double-press freezes its current members
   as an ordinary fixture selection.

Normal recording merges into an existing Group, preserving its member order. **REC +** explicitly
merges; **REC −** subtracts selected members. Choose one-off **Smart** for Merge/Overwrite, or
`RECORD SMART GROUP 22` to overwrite Group 22. Stored empty Groups remain valid; absent Groups in
a range are skipped.

The lower-left count is grey with no members selected, selected/total for a partial selection,
and the white total when all are selected. This follows fixture selection as well as Group references.

![Group member-count states](../../assets/screenshots/panes/groups-tile-counts.png)

A live reference follows later membership changes. Dereference it when you need a frozen list.
Derived Groups preserve their ordering rule and source relationship; see exact syntax in the
[Command Line Reference](01-command-line.md#dereferencing-groups).

![Source Group, derived Group and frozen snapshot markers](../../assets/screenshots/panes/groups-tile-markers.png)

### Configure ordering and phase

Hold a Group or enter `[SET] [GRP] <number> [ENT]` for settings. **General** sets name, icon and
colour. **Projection** maps Stage positions onto a plane; **Phase** orders that plane as Grid,
Radial or Radar. Its preview shades first-to-last ranks. Fixtures sharing a rank receive the same
point in a THRU spread; fixtures without a valid position use individual fallback ranks.

Grid direction 0° ranks columns, 90° ranks rows, and 45° ranks diagonals. Projection may be Planar,
Cylindrical or Spherical. Use its position/direction/rotation controls and the preview to align
it with the rig; cylindrical/spherical direction uses azimuth and elevation.

Referenced Groups inherit mappings until you edit Projection or Phase locally. Membership remains
live. Mixed source mappings use visible source order until you assign one. Fixture positions
remain in Stage; a Dynamic may use its own Projection override.

A Group value reaches only members supporting that attribute. Unsupported members retain their
other values. To create a **Group Master**, assign the explicit Group to a Playback; it is not a
Group setting. Playbacks targeting that same Group share its master, while different Groups
remain independent HTP contributors.

## Record and recall Presets

Preset families have separate local numbers:

| Address prefix | Family | Stored attributes |
| --- | --- | --- |
| `0.` | Mixed | Attributes chosen by the operator. |
| `1.` | Intensity | Intensity, Shutter and Strobe. |
| `2.` | Color | Colour values. |
| `3.` | Position | Position values. |
| `4.` | Beam | Beam/Focus functions, including Zoom, Iris, Gobo, Prism and Frost. |

For example, `2.1` means Color Preset 1, not a globally numbered Preset.

Build values, press **REC**, then touch a Preset. With normal Smart recording, an existing Preset
merges (**REC MRG**); an empty slot records (**REC**). Merge retains other values and replaces
matching ones. For complete replacement, choose one-off Smart then **Overwrite**, or enter
`RECORD SMART 2.1`. Saved alternative Record defaults still apply.

Select fixtures and touch a Preset to recall compatible values. Recording recalled values into a
Cue retains their Preset links. Updating the source changes linked Cue values, even during playback.
A direct edit before recording makes that attribute independent. Clear the Programmer to check
playback alone.

Moving a Preset retains links; copying creates a separate source. Deleting a linked source uses
the Cue's recorded fallback. Creating another Preset at the old number does not take over the link.
Cuelist View identifies missing sources and the recovery action.

### Read the tile

A Preset's lower-left number is the count of fixtures currently showing its values after all live
sources are resolved. Overridden or merely stored values do not count; no matches shows 0.
Unpatched fixtures still belong to the show's Preset targets.

![Active counts, segmented colour previews and an explicitly chosen icon](../../assets/screenshots/panes/presets-color-details.png)

Automatic Color previews show the requested colour at full brightness, independently of Intensity.
Several colours appear as segments, up to six; no averaging hides the spread. UV-only requests
appear violet and unknown Direct appearance hatched grey.

Position previews show up to ten distinct aims: Pan/Tilt for angles, or Stage X/upstage for targets.
They retain the spread's proportions and extremes. Mixed angle/target Presets show the majority kind.

![Automatic position previews for a fan, audience grid and centre target](../../assets/screenshots/panes/presets-position-details.png)

A chosen icon or button colour replaces the automatic preview. **Automatic icon** restores it.
Intensity and Beam Presets use plain tiles. These previews describe stored data; viewing them
never recalls a Preset.

A Color Intent Preset holding one shared colour is **universal**, so it can apply to fixtures that
were not selected when recorded. Different stored colours remain fixture-specific. See
[Color Intent](05-color-intent.md#universal-color-presets).
