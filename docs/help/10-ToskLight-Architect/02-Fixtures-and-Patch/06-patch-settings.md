# Inspect and configure the patch

The patch lists fixture IDs, modes, addresses, master policy, installed appearance, placement, and layers. Location and rotation describe the installed object; they are separate from Programmer values.

**Fixtures**, **Points**, **Media Servers**, and **Tracking** are tabs at the top right of Show Patch, next to **⚙** Settings. The tabs and **⚙** stay in the same place on all four views; a narrow window shortens the Fixtures actions instead. Points, Media Servers and Tracking each scroll as one page with the same inner margins as Settings, so their last controls stay reachable on a short display. **⚙** opens the Show Patch Settings on the page for the current view: **Columns** for the Fixtures table (also from Points, which has no settings of its own), **Media Servers** for the thumbnail cache, and **Tracking** for the PosiStageNet source.

## Sort and choose columns

Click a column header in Show Patch to order the table by that column, for example **Fixture ID**
or **Patch**; click it again to reverse the order. An arrow marks the column the table is ordered
by. **Patch** orders by universe, then address. Unpatched fixtures and fixtures without a fixture
ID always come last, and fixtures with the same value keep Fixture ID order. Shift ranges follow
the order the table shows. Show Patch opens ordered by Fixture ID.

Choose which columns Show Patch draws from its settings: the gear in the Show Patch header, or
**Pane Settings → Show Patch** for a Show Patch pane. Switch a column to **Hidden** to leave it out;
every column is visible until you hide it. Each pane keeps its own choice, and the full-screen
Show Patch keeps one for the desk. The last visible column cannot be hidden.

## Physical instance settings

The combined **MIB** cell shows **Off** or one non-negative delay in seconds. `0 s` means Move in Black is enabled with no delay and is distinct from Off. One SET action writes enabled state and millisecond delay atomically; fractional seconds are accepted when representable.

The two-row **Masters** cell shows Group Masters and Grand Master participation. Its one editor chooses **Not controlled**, **Group Master**, **Grand Master**, or **Both** and atomically applies the eligible policies. A fixture is eligible when its mode has a level channel (Intensity, Volume) or a channel that follows the virtual intensity: masters scale the level parameters before DMX, and other channels follow them only through the virtual intensity. Ignoring a master is an intentional live-output exception: the fixture's levels remain live while that master is reduced. It does not bypass Blackout, output-route disable, hazardous-fixture safety, or emergency suppression. Multi-patch rows show this logical-fixture value as shared.

The two-row **Pan / Tilt** cell shows each physical instance's inversion. Its one editor chooses **None**, **Invert Pan**, **Invert Tilt**, or **Invert Both** without changing Programmer, Preset, Cue, or tracking values. Inverted maps 0% to the high endpoint and 100% to the low endpoint while leaving 50% unchanged. The main fixture and each multi-patch instance can use different values. Inapplicable axes stay visibly unavailable and retain their dormant value.

The **Light source** cell shows the selected installed source, effective color temperature and luminous output above its gel/filter. SET opens one physical-instance editor for Profile default or a typed lamp source, whole-kelvin CCT from 1,000-25,000 K, and an optional positive lumen output; leaving output empty inherits the embedded fixture profile. Gel selection opens its own preset grid. Choosing a gel closes that grid and returns to the Light source editor with the selection applied. Open white and a named custom color remain available there as well. Every installation starts with the **Generic gels** catalog (G00–G15), which can be edited like any other installation-owned catalog. Catalog CSV import uses exactly `number,name,display_rgb,visualizer_rgb` and presents additions, replacements, conflicts, and row-specific errors before explicit confirmation. Assigning a catalog entry embeds its number, name, display color, and separate visualizer color in the show, so another desk can render the look without that installation catalog. Fixtures without light-emitting geometry leave this cell unavailable.

Use **Open Stage Renderer** in the Show Patch title bar to open the dedicated PreViz Renderer window. It follows the authoritative show patch and live output as the patch changes; there is no embedded preview overlay. Selection in Show Patch remains normal desk selection and does not add hidden output values.

Configure the installation-wide identification look under **Desk Setup > Preferences > Highlight**. See [Configure Highlight](../../20-ToskLight-Control/10-Show-Setup/06-highlight-look.md) for the settings and compatibility choices.

Older portable shows may still contain raw per-fixture Highlight override maps. ToskLight preserves those maps losslessly and continues evaluating them while the installation is in **LegacyRaw** or **NeedsReview** compatibility mode. They remain hidden from ordinary Patch editing; changing a fixture mode retains only overrides whose stable channel identities and resolutions are still compatible. Choosing **Use semantic Highlight Look** is the explicit decision to stop evaluating the legacy raw maps. It does not delete or rewrite them, add new maps to the show, or store transient Highlight state.

Use [Position and Color calibration](13-installed-calibration.md) for measurements belonging to one physical unit.
