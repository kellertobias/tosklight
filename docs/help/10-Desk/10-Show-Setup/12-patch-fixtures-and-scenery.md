# Patch Fixtures and Scenery

The patch connects logical fixture IDs to fixture-library modes and physical DMX addresses. Open **Show > Show Patch**. Patch is a full built-in workflow and is not one of the addable desk panes.

![Show Patch table, layers, fixture modes, addresses, and transforms](../../assets/screenshots/workflows/show-patch.png)

## Patching Fixtures

After you have selected the fixture type you want, you can patch your fixtures with entering `<amount> [AT] <universe>.<address>`. This patches the amount of fixtures starting at the selected address with the offset of the amount of channels in the selected mode of the fixture. You can chain multiple of these before pressing [ENT] if you want them across multiple addresses or universes.

The patch refuses addresses outside 1-512 and detects overlaps across primary and multi-patch instances. Unpatched fixtures remain valid show fixtures and can still be selected and programmed; they simply produce no routed DMX until addressed.

For a multi-split profile mode, the placement dialog shows **Independent split patches** with one optional `universe.address` field per split and its separate footprint. Clear a field to leave only that split unpatched. Batch placement advances every patched split by its own footprint, validates all split ranges independently, and rejects overlap between splits, fixtures, and multi-patch instances. The Patch table presents every split as its own `S<number> <universe>.<address>` or `—` target.

To repatch, select a split target and press `[SET]`, or press `[SET]` first and then touch it.
Enter or choose the new `universe.address`; the Desk validates every split and applies the confirmed
patch together. **Clear address · Unpatch** removes only the selected split. Closing leaves the
current patch unchanged, and any collision or invalid address is reported without a partial patch.

The Patch table identifies the fixture, mode, address, master policy, Pan/Tilt inversion, MIB,
light source, placement, and layer. Location and rotation describe the installed object for Stage;
they are not Programmer values or DMX channels.

Use **+ Add fixture** to search by type, manufacturer, fixture family, and mode, then check the footprint and physical details before placement. Search sits in the Add Fixture title bar and filters automatically with every typed character; no Search-button confirmation is required. It follows the shared [search-bar layout](../30-Windows/01-desk-interface-and-windows.md#search-bars), and its optional Options dialog selects the fixture type. Clearing the query restores all fixtures. Manufacturer and fixture names align left, while type/mode counts and detail values align right for quick scanning. In the placement dialog, **Start fixture ID** is a regular number field alongside **Count** and the **Address**/**Empty** choice. Choose **Empty** to add every requested fixture unpatched: the fixtures retain their IDs, profiles, modes, layers, positions, and programming surfaces but send no DMX until patched later. Switching back to **Address** restores the normal footprint preview and collision validation. A batch starting at ID 100 receives 100, 101, 102, and so on; any ID already used in the show is skipped while the requested fixture count is preserved. **Cancel**, **Add fixtures**, and Close remain together in the placement title bar, with Add directly beside Close. Closing or cancelling after changing placement values asks for confirmation with **Yes, close** and **Stay in Add Fixture**; staying preserves every entered value.

The manufacturer column is ordered **All manufacturers**, **Generic**, **Venue**, then the actual manufacturers alphabetically. Venue profiles are scenic objects rather than DMX fixtures. Their placement dialog assigns fixture IDs from the reserved `0.x` range, beginning at `0.1`, and asks for name, count, and mode but has no Address field or universe grid. The Patch, MIB, MIB Delay, and Highlight cells show that no DMX patch applies. They remain ordinary transferable show objects with editable location, rotation, and layer. They appear in Show Patch and Stage but are excluded from the Fixture Sheet, which contains programmable fixtures only. The same exclusion applies independently to every `visual_only` profile and every complete fixture ID beginning `0.` so imported or legacy scenery cannot leak into the programming table.

Click a column header in Show Patch to order the table by that column, for example **Fixture ID**
or **Patch**; click it again to reverse the order. An arrow marks the column the table is ordered
by. **Patch** orders by universe, then address. Unpatched fixtures and fixtures without a fixture
ID always come last, and fixtures with the same value keep Fixture ID order. Shift ranges follow
the order the table shows. Show Patch opens ordered by Fixture ID.

Choose which columns Show Patch draws from its settings: the gear in the Show Patch header, or
**Pane Settings → Show Patch** for a Show Patch pane. Switch a column to **Hidden** to leave it out;
every column is visible until you hide it. Each pane keeps its own choice, and the full-screen
Show Patch keeps one for the desk. The last visible column cannot be hidden.

## Importing a CSV Patch List

Open the top-right **⚙ Settings** in Show Patch and press **Import CSV** in the Settings title to add a whole fixture list from a spreadsheet, paperwork export, or another desk. Every CSV row becomes one fixture. The import only adds fixtures; it never changes or removes fixtures already in the show. From **Media Servers** or **Tracking**, the same action switches to **Fixtures** first. In a Show Patch pane, **Import CSV** sits in the title of that pane's **Pane Settings**.

1. **Columns.** Choose the file. Comma, semicolon, and tab separated files are detected automatically, and quoted cells may contain the separator. Choose whether the first row holds **Column names** or **Fixture data**, and whether X / Y / Z are in **Metres** or **Millimetres**. Above every column, assign **Patch**, **Fixture ID**, **Fixture Name**, **Fixture Type**, **Manufacturer**, **Mode**, **X**, **Y**, **Z**, **RotX**, **RotY**, **RotZ**, or **Ignore**. Recognised column names are suggested; each field can be assigned to only one column, so choosing it again moves it.
2. **Fixture types.** Rows are grouped by manufacturer, fixture type, and mode. A group uses a library fixture directly when exactly one library mode has the same manufacturer, fixture name, and mode name, ignoring only letter case and repeated spaces. Without a Manufacturer column, the Fixture Type may contain both, such as `Martin MAC Aura`. Without a Mode column, only a fixture with a single mode matches. Every other group stops in the wizard: choose the manufacturer, fixture, and mode it should use, or **Skip these rows**. The wizard moves to the next unmatched group after each choice and preselects a likely fixture, which you still confirm. Exact matches can be changed here too.
3. **Review.** Check every row's fixture ID, name, library fixture, patch, location, and rotation, then press **Import**. All importable rows are added together in one Patch change on the layer currently selected in Show Patch (**Default** when All fixtures is shown).

Cell rules:

- **Patch** accepts `universe.address`, `universe/address`, or one absolute address, where `513` means `2.1`. An empty cell or `-` imports the fixture unpatched. Only the first split of a multi-split mode is patched from the CSV.
- **Fixture ID** must be a whole number that is not used in the show or earlier in the file. Venue profiles use `0.x`. An empty cell takes the lowest free ID.
- **Fixture Name** defaults to the fixture name followed by its ID.
- **X / Y / Z** and **RotX / RotY / RotZ** default to 0; rotation is in degrees. A decimal comma is accepted.
- **Address conflicts** with the show or an earlier row are imported unpatched by default; choose **Skip row** to leave them out instead.

Rows with an unusable cell are listed as **Not imported** with the reason and never block the other rows. Closing after choosing a file asks for confirmation.

## Show Patch views

**Fixtures**, **Media Servers**, and **Tracking** are tabs at the top right of Show Patch, next to **⚙** Settings. The tabs and **⚙** stay in the same place on all three views; a narrow window shortens the Fixtures actions instead. Media Servers and Tracking each scroll as one page with the same inner margins as Settings, so their last controls stay reachable on a short display. **⚙** opens the Show Patch Settings on the page for the current view: **Columns** for the Fixtures table, **Media Servers** for the thumbnail cache, and **Tracking** for the PosiStageNet source.

## Patched Media Servers

**Show Patch > Media Servers** opens with a table of every patched Media Server, one row each, ordered by fixture number. A server stays in the table while its network control is off; only its connection is paused.

| Column | Meaning |
| --- | --- |
| **Type** | **ToskLight Media** for a ToskLight Pixel Media Server, **CITP media server** for any other CITP-capable media server, or **No network control** when the profile offers none. |
| **Protocol** | **CITP** connects the desk to the server; **Off** stops the desk from talking to it. Only protocols the fixture profile supports are offered. |
| **IP address** and **Port** | Where the server listens for CITP. The address must be a literal IPv4 or IPv6 address, and the port must be 1–65535; the row explains a wrong value and keeps **Apply** disabled until it is fixed. |
| **Status** | The desk's own connection state: **Connected**, **Offline**, **Not checked**, **Checking…**, or **Off**. Below it, whether the last discovery **Found** the address on the network. An offline row shows the server's reason and what to check. |
| **Actions** | **Apply** stores the row's protocol and address. **Check connection** asks that server once whether it answers and updates **Status**. **Refresh Thumbnails** reconnects and fetches that server's media thumbnails. **Start live preview** shows the server's output under the row. |

The status follows the desk as it changes: when a connection attempt succeeds or fails, every open Show Patch updates without refreshing. When you apply a new address, the row checks the server once at that address. Each row works on its own; refreshing or applying one server never disables another row or clears its message.

**⚙ > Media Servers > Clear Thumbnail Cache** drops every Media Server thumbnail the desk has cached, for example after replacing media on a server. Live previews and the patch stay as they are; use **Refresh Thumbnails** on a row to fetch its thumbnails again.

## Discovering a ToskLight Pixel Media Server

Open **Show Patch > Media Servers** to discover ToskLight Pixel Media servers on the local network. **Refresh Discovery**, in its own group at the top right of the window beside the view tabs, repeats the search without restarting either application. Each output is shown by its Media Server and output name. Below the name are the server's IP address, its type (**ToskLight Media**), its CITP port, whether it is **Online**, and the desk's connection to it (**Connected**, **Offline**, **Not checked**, or **Not connected (not patched)**). Next comes the output's current configuration:

- the suggested DMX `universe.address`
- the **2 layers** or **8 layers** personality
- the protocol and universe it listens to (for example **listens to sACN 101**)
- whether its tempo follows the Playback BPM channel or a desk Speed Group

The suggested desk universe is the one whose output route (**Setup › Outputs**) sends what the server listens to. When no desk route sends it, the card says so. A discovered output remains explicitly **Not patched** until you choose an action; discovery alone never changes the show.

The status beside each output tells you what to do next:

- **Patched** – a desk fixture controls this output with the matching personality, at the server's address, and a desk output route delivers that universe to the server.
- **Mode differs** – the desk fixture uses the other personality. Every layer after the first would be misaddressed, so the card explains the difference, and **Patch suggested** switches the desk fixture to the output's personality.
- **Endpoint differs** – the desk fixture controls the server at another IP address or CITP port than discovery found. **Patch suggested** updates the desk endpoint.
- **Address differs** – the desk sends the fixture's universe or address somewhere other than where the output listens. The card names both sides. **Patch suggested** moves the desk patch to the server; **Patch address** moves both.
- **Not received** – no desk output route sends the fixture's universe onto the network, or the fixture is unpatched, so the server receives nothing. Add an output route under **Setup › Outputs**, or choose **Patch address**.
- **Needs update** – the Media Server reports a personality the desk cannot patch, or is too old to describe its outputs in the current format (for example, it still offers the retired channel layouts). The patch actions stay disabled. Update ToskLight Media, or choose 2 or 8 layers under its **Settings > Network & DMX**, then **Refresh Discovery**.
- **Unavailable** – the server answered discovery but not its configuration API. Check that it is running and reachable on port 8080, then refresh.

**Patch suggested** creates the matching ToskLight Pixel fixture, or updates the existing one, at the desk universe and address that reach the output. It also sets the fixture's CITP endpoint to the discovered server. It never changes the Media Server. The normal Patch footprint and collision checks run before the desk accepts it; a refusal is shown on the card and says that the Media Server was not changed.

**Patch address** lets you choose another desk universe and address. Before anything changes, the desk looks up how it sends that universe. When no enabled network output route sends it, nothing changes and the card asks you to add the route. Otherwise:

1. The desk patch is validated and stored.
2. The selected Media Server output moves to that address. Its DMX input also moves to the route's protocol and universe, for example **Art-Net 1** when desk universe 1 is sent as Art-Net universe 1. When a universe is sent both ways, the server keeps the protocol it already uses.
3. The card confirms both sides, or says that the server listens there only after its next restart.

If the Media Server answers with a different address than requested, the card says so rather than reporting success. If the remote update fails, the desk restores its previous patch (or removes the newly created fixture) and asks you to **Refresh Discovery** to confirm the server's address. If restoring also fails, the card says that the desk and the server may differ. It never silently reports mismatched addresses as patched. When the Media Server refuses the change, the message says why and what to do: a value it rejected (such as an 8-layer block that no longer fits the universe) names that value, a change it could not save asks you to check free space and write access for its configuration folder, and an output it no longer has asks you to refresh discovery.

Native Media controls (effects and text) act on the output the fixture was patched to. If that output has since been removed from the Media Server, the desk reports it instead of controlling a different screen; refresh discovery and patch the output again.

The Media Server may report that its DMX input change needs a restart; Show Patch keeps that state visible. An unreachable server or discovery failure does not disable the ordinary fixture-library and manual Media Server patch workflows.

Pixel also publishes GDTF and desk-native personality downloads in its management interface under
**DMX > Connect to Console**. Use those files when patching Pixel from MagicQ,
grandMA2, grandMA3, or another GDTF-compatible desk. The Layer and Master personalities come from
the same canonical channel layout as Pixel's DMX receiver; patch every Layer consecutively and one
Master immediately after them.

The placement dialog shows all 512 addresses of the selected universe as a scrollable grid of square touch targets. Existing fixture ranges have a gray outline and translucent gray fill labeled with fixture ID and name. Every fixture requested by **Count** appears as its own blue proposed range, arranged consecutively by default; a range turns red if it overlaps an existing or proposed fixture. Grab any blue range with a mouse or touch to move that fixture independently, or select one and tap a free address. The batch is created from the individual displayed addresses, and every footprint remains inside its universe.

The combined **MIB** cell shows **Off** or one non-negative delay in seconds. `0 s` means Move in Black is enabled with no delay and is distinct from Off. One SET action writes enabled state and millisecond delay atomically; fractional seconds are accepted when representable.

The two-row **Masters** cell shows Group Masters and Grand Master participation. Its one editor chooses **Not controlled**, **Group Master**, **Grand Master**, or **Both** and atomically applies the eligible policies. Ignoring a master is an intentional live-output exception: eligible intensity channels can remain live while that master is reduced. It does not bypass Blackout, output-route disable, hazardous-fixture safety, or emergency suppression. Multi-patch rows show this logical-fixture value as shared.

The two-row **Pan / Tilt** cell shows each physical instance's inversion. Its one editor chooses **None**, **Invert Pan**, **Invert Tilt**, or **Invert Both** without changing Programmer, Preset, Cue, or tracking values. Inverted maps 0% to the high endpoint and 100% to the low endpoint while leaving 50% unchanged. The main fixture and each multi-patch instance can use different values. Inapplicable axes stay visibly unavailable and retain their dormant value.

The **Light source** cell shows the selected installed source, effective color temperature and luminous output above its gel/filter. SET opens one physical-instance editor for Profile default or a typed lamp source, whole-kelvin CCT from 1,000–25,000 K, and an optional positive lumen output; leaving output empty inherits the embedded fixture profile. Gel selection opens its own preset grid. Choosing a gel closes that grid and returns to the Light source editor with the selection applied. Open white and a named custom color remain available there as well. Every installation starts with the **Generic gels** catalog (G00–G15), which can be edited like any other installation-owned catalog. Catalog CSV import uses exactly `number,name,display_rgb,visualizer_rgb` and presents additions, replacements, conflicts, and row-specific errors before explicit confirmation. Assigning a catalog entry embeds its number, name, display color, and separate visualizer color in the show, so another desk can render the look without that installation catalog. Fixtures without light-emitting geometry leave this cell unavailable.

Use **Open Stage Renderer** in the Show Patch title bar to open the dedicated PreViz Renderer window. It follows the authoritative show patch and live output as the patch changes; there is no embedded preview overlay. Selection in Show Patch remains normal desk selection and does not add hidden output values.

The ordinary Patch table does not edit Highlight values. Configure one installation-wide semantic look under **Desk Setup > Programmer > Highlight Look**; the fixture profile translates it into the fixture's authored functions, color system, physical ranges, and exact DMX.

Older portable shows may still contain raw per-fixture Highlight override maps. ToskLight preserves those maps losslessly and continues evaluating them while the installation is in **LegacyRaw** or **NeedsReview** compatibility mode. They remain hidden from ordinary Patch editing; changing a fixture mode retains only overrides whose stable channel identities and resolutions are still compatible. Choosing **Use semantic Highlight Look** is the explicit decision to stop evaluating the legacy raw maps. It does not delete or rewrite them, add new maps to the show, or store transient Highlight state.

![Fixture-library browser used while patching](../../assets/screenshots/workflows/patch-add-fixture.png)

## Multi-patch

Multi-patch gives one logical fixture additional physical output instances. Use it when several physical units must always share the same logical programming. Every instance uses the same embedded fixture profile and values but has its own per-split universe/address assignments, optional stage position, and physical Pan/Tilt inversion. The same independent footprint and overlap checks apply to every instance. Do not use multi-patch for separately selectable heads; use a multi-head fixture definition instead.

Repatch a multi-patch instance through the same **Multi-patch Address** screen used for a fixture address. It shows all 512 slots of the selected universe, supports selecting a free start slot by touch, and allows dragging the pending footprint to another address. **Set Address** and Close remain together in the title bar; only the edited instance is excluded from the occupied-address display, so the fixture's primary patch and its sibling multi-patches remain protected from overlap.

Venue objects have no multi-patch: **+ Add multi-patch** is unavailable while one is selected. Place each span of a truss run, each deck, stair, pipe, or curtain as its own Venue object, so each has its own `0.x` ID, placement, and size. A show saved with multi-patch copies of a Venue object opens with every copy turned into a Venue object of its own, keeping its name, placement, and size and numbered after the show's existing `0.x` objects.

> [!danger] Missing graphic
> Add a comparison diagram showing one logical fixture with several physical multi-patch instances beside one fixture with independently selectable logical heads.

## Multi Head Fixtures

Multi-Head Fixtures are lamps that have more than one individually controllable light source. Good examples are LED strips with individual controllable segments, LED PAR-Bars with 4 individually controllable heads, etc.

Every of these heads acts like a single fixture, but they are grouped together and patched together.

You give a multi-head fixture one fixture ID, such as 100. Its master uses sub-address `100.0`, while its individually controllable heads automatically receive `100.1`, `100.2`, and so on.

For a ten-head Sunstrip with shared tilt, `100 [ENT]` selects `100.0` followed by `100.1` through `100.10`. Use `100.0 [ENT]` when you want only the master and its shared tilt parameters.

Bare fixture ranges intentionally select controllable heads without their masters: `100 [THRU] 110 [ENT]` expands to the child heads of fixtures 100 through 110. To select the shared masters instead, use `100.0 [THRU] 110.0 [ENT]`.

In the fixture sheet, a multi-head fixture appears as separate `.0`, `.1`, `.2`, and subsequent rows. There is no additional aggregate row.

## Patch check

After patching, inspect **DMX > Universe** for footprint and channel ownership, then set a safe test value and verify the real output. Save a named revision before a large repatch.
