# Programming and Visualization Windows

## Preset pool

The Preset pool recalls reusable attribute values into the programmer. Intensity, Color, Position, and Beam presets contain only attributes from that family. A Mixed preset can contain any combination of attributes. A preset does not execute a Cue by itself. Tap a populated tile to apply it to the current fixture selection. With Record armed, an empty tile stores a new preset and a populated tile offers overwrite or merge.

Each tile shows its number, title, family, icon or artwork, and how many fixture-value entries it contains. The pool provides at least 200 numbered positions and grows to include the highest stored preset number. Filtering to one family disables tiles from the other families instead of deleting or renumbering them.

**Pane configuration:**

- **Preset family** selects Mixed, Intensity, Color, Position, or Beam for this pane only. Mixed is a family for presets containing any combination of attributes; it is not an aggregate list of the other families. Two Preset panes can therefore show different families.
- **Type colors / Individual colors** chooses the color source for this pane. Type mode uses the configured Preset-family default. Individual mode shows only an explicit button color and leaves uncolored tiles grey.
- **Show group shortcuts** adds the Group strip below the pool so fixtures can be selected before a preset is recalled or recorded.

The full Presets window additionally exposes family buttons and pool-color settings in its header. With Set armed, tapping a preset opens its local button presentation settings: title, icon, and button color. Those presentation choices are stored in desk data, scoped to the active show identity, and do not change the stored preset values.

![Preset pool pane](../../assets/screenshots/panes/presets.png)

![Preset pool settings](../../assets/screenshots/panes/presets-settings.png)

## Group pool

The Group pool stores ordered fixture selections. Order matters for operations such as value spreading, and an intentionally stored empty Group remains different from an unused pool position. Tap a populated Group to select its current ordered members and place that Group reference on the command line, ready for an operation such as **DIV**. Record plus an empty tile stores the current selection; recording over an existing Group offers overwrite or merge.

A populated tile shows its number, name, member count, and status information such as missing members, portable stored attributes, unsupported values, or whether it is derived or frozen. A selected Group is highlighted. Hold a populated tile to open its operational controls: adjust its Group master, select the live or frozen membership, refresh a frozen snapshot, detach a derived Group, replace membership, or undo the latest membership/programming change.

The Group master limits the intensity of members when that Group is assigned to a playback fader. It does not rewrite their programmer or Cue values. Missing fixture IDs are reported and skipped; they do not turn the Group into an empty Group.

**Pane configuration:** **Type colors / Individual colors** chooses the shared Group default or explicit item colors for this pane. Its membership and Group-master controls are content operations, not pane-layout settings. The common size and removal settings also apply.

![Group pool pane](../../assets/screenshots/panes/groups.png)

## Fixture sheet

The Fixture sheet is the detailed programming-state inspection and selection table. Each value cell shows the winning ordinary static base before Dynamic/FAT modulation. A running Dynamic keeps its icon and pool number beside the affected member but does not replace the base with its continuously sampled value. Open **DMX output** to inspect the changing resolved DMX result. While Preload is active, the Fixture sheet adds the pending base and Dynamic identity for comparison. Activating a row selects that fixture or logical head. A compact pane shows every eligible row in Fixture ID order and scrolls when the rows exceed the available pane height.

Venue fixtures, `visual_only` profiles, and complete fixture IDs beginning `0.` remain in Show Patch, Stage, selection, MVR, and the portable show but never appear as Fixture sheet rows. These independent scenery checks run before active-only, Cuelist, ordering, and head filters. An authoritative selection containing only hidden scenery therefore selects no unrelated Fixture sheet row.

Simple fixtures use one row. Multi-head fixtures can expose a `.0` master row for shared parameters and `.1`, `.2`, and following logical-head rows for the individual heads.

During PREV/NEXT stepping, every row in the remembered base selection remains visibly selected with a subdued patterned treatment, while the actual current fixture or head uses the prominent selected treatment and a stronger left marker. This distinction does not rely on color alone and remains visible whether HIGH is off or on. PREV and NEXT move the prominent marker without hiding the base; ALL restores ordinary complete-selection styling, and an external selection replaces both indications. Multi-head state appears on the actual subhead rows. When subheads are hidden, the visible master row retains a contained-base or contained-step indication so the active state is not hidden.

### Fixture sheet columns

| Column | What it shows |
| --- | --- |
| **ID** | The fixture number. Multi-head targets add `.0` for the master and `.1` onward for logical heads. |
| **Icon** | The fixture profile's stage icon. This column normally appears between ID and Name. |
| **Name / type** | Operator name with optional manufacturer/mode. An assigned Group-master badge shows its effective fader/Flash state and explicitly reports when live Highlight output bypasses it. |
| **Patch** | The fixture's universe and address, or **Unpatched**. This dedicated column is off by default. |
| **Intensity** | The ordinary base level as a meter and percentage. During Preload, an arrow shows the pending base percentage. |
| **Color** | An RGB swatch and label. Every swatch has the same thin light-grey boundary so black, dark, bright, absent, and mixed colors remain distinct from the table without changing the resolved fill. During Preload, a second swatch identifies the pending color. Fixtures without color parameters show the neutral fallback. |
| **Position** | A position glyph and pan/tilt values. Fixtures without position parameters show a dash. During Preload, the pending pan/tilt values appear below. |
| **Beam** | Authoritative semantic Beam bases such as an open state or indexed gobo. |
| **Shapers** | Authoritative framing/shaper member bases. |
| **Focus** | Authoritative focus, edge, frost, or zoom member bases. |
| **Control** | Authoritative semantic fixture-control bases without exposing raw DMX. |
| **Media** | Distinct Media Folder/File and, where supported, Mask Folder/File base pairs. |

The **Columns** settings independently control ID, Icon, Name, Patch address, Intensity, Color, Position, Beam, Shapers, Focus, Control, and Media. At least one column remains visible. Existing saved **Dimmer** visibility migrates to **Intensity**; newly available group columns do not turn themselves on in an existing layout. Source styling distinguishes current Programmer, Playback, and profile-default bases.

Freeze state appears in the Name cell as a blue snowflake status that is separate from selection,
Highlight, Group-Master limitation, and patch state. `❄ FREEZE` is a full-output Freeze;
`❄ FREEZE · Intensity · Color` (with the applicable family names) is a partial Freeze. A master row
shown without its frozen subheads adds `INSIDE` so the state is not hidden by the head filter.

A column only reports attributes the lantern actually carries. A fixture without colour or without Position shows **—** in that column with no colour swatch and no position crosshair, so a frost-only or dimmer-only lantern is never given a preview it cannot honour. Where the lantern does carry the group and nothing drives it, the column shows the profile home value: physical white for colour and centre for absolute Pan and Tilt.

**Pane configuration:** **Fixture Sheet → Compact mode** has exactly **Off**, **Icon only**, and **Text only**. Off keeps the detailed 43 px presentation. Icon only uses deterministic 32 px rows, retains graphical base/Preload summaries, and removes ordinary value text. Text only uses the same 32 px rows, retains concise semantic base/Preload text, and removes decorative value graphics. Both compact modes keep Dynamic identities, source ownership, Group-master/Highlight status, fixture type, selection, and step markers. Configured columns are never dropped at a breakpoint; a small pane scrolls horizontally when the selected set still cannot fit. Each pane, the full built-in, and each fixed external Fixture Sheet persist their own desk-local mode, defaulting to Off without changing portable show data.

**Show active fixtures only** limits that pane to fixtures carrying Programmer values; this is useful beside a Cuelist Pool while recording looks. **Show group shortcuts** adds the Group strip. The common size and removal controls also apply. In the full Fixture Sheet window, open **Fixture Sheet** settings and use **View** for Compact mode, fixture heads, ordering, and filters; **Columns** for visible data and optional Name details; and **Groups** for the Group strip. **Included heads** defaults to **All**. Choose **No sub heads** to show only master rows with the fixture's bare ID, or **No master heads** to show only subhead rows without indentation. There is no per-row expand or collapse button. These full-sheet choices persist with the desk layout. Ordering can use Fixture ID or put active programmer fixtures first; filters can show only active fixtures or limit membership to one Cuelist.

![Fixture sheet pane](../../assets/screenshots/panes/fixtures.png)

![Fixture sheet Icon-only compact mode](../../assets/screenshots/panes/fixtures-icon-only.png)

![Fixture sheet Text-only compact mode](../../assets/screenshots/panes/fixtures-text-only.png)

![Fixture sheet pane settings](../../assets/screenshots/panes/fixtures-settings.png)

![Full Fixture Sheet view controls](../../assets/screenshots/workflows/fixture-sheet-settings-view.png)

![Full Fixture Sheet column controls](../../assets/screenshots/workflows/fixture-sheet-settings-columns.png)

## Media

The Media window is available from **Shift + Stage** in Built-ins and from **Open Window**. Without a configured CITP/MSEX connection it says that no CITP Media Server is available and still shows the complete 0–255 Content and Mask folder/file configuration ranges. Endpoint setup remains under **Show Patch > Media Servers**. When a server connects, its advertised names, thumbnails, status, and controls reconcile into the same stable configuration surface without hiding unadvertised values.

Choose the physical master in **Server**, then choose one of its advertised logical layers. A layer touch replaces the desk's shared fixture selection with that exact logical head, so the Programmer, OSC, attached controls, and other panes continue to describe one authoritative selection. The Program output and layer thumbnails use the preview-source and layer identities advertised by the server; source number, layer number, and fixture sub-ID are not assumed to be interchangeable. Loading, stale, failed, unsupported, and offline states are shown beside the preview rather than drawn into the program image.

The folder and file pools separate browsing from live output. Touching a folder changes only the local draft and loads its files; it does not change Programmer values or DMX. Touching a file commits that folder and file together as one Programmer operation and one Undo step. It never exposes the newly browsed folder with the old live file. Media encoders remain immediate and do not use this staged touch workflow.

**Media / Mask** always selects the corresponding complete numeric folder/file range. Advertised mask names and thumbnails reconcile into those slots; without that advertisement the values remain configurable through the patched layer's Programmer attributes and are identified as not advertised. Server-specific secondary controls still appear only when the fixture and connection advertise them. Library administration and connection setup stay outside this window.

A saved Media pane keeps its selected server, layer, Media/Mask choice, content section, and secondary-region visibility as desk-local state. A disconnect, missing patch, or temporarily unsupported capability does not delete that pane or silently select another server. The pane explains the unavailable state and resumes the same stable identities when they return.

**Pane configuration:** the selected server and layer, browser choice, main content section, and secondary-region visibility belong to this pane. Common size and removal controls also apply.

## Stage

The Stage is the spatial selection and visualization surface. In 2D it shows fixture symbols, position, color, intensity, and direction. In 3D it renders patched fixtures, multi-patch physical instances, beams, and visual-only Venue fixtures. Tap or marquee fixtures to select them; Shift extends a range and Control/Command toggles fixtures. **Follow Preload** changes the Stage from live output to the pending Preload visualization.

**Pane configuration:**

- **Stage view** selects 2D or 3D for this pane.
- **Follow Preload** makes this pane a dedicated preview surface while another Stage pane can remain live.
- **Show group shortcuts** adds the Group strip.
- The common size and removal controls apply per pane.

Drag to orbit, scroll to zoom, use the middle button to pan, and use the secondary button to slide
the view. **Follow Preload** shows the pending values where present; a preloaded Dynamic remains
live because it is a running function. **2D** is the rig plan, **3D** is the model-and-aim view,
and **3D Viz** adds the rendered light picture. A clearly reported unavailable Stage never affects
programming, playback, or DMX output.

### Stage is a selection and viewing surface

Only the full Stage window exposes **Select fixtures** and **Navigate**. A Stage pane reflects the global mode, but it does not contain the controls that switch it.

With **Navigate** active, the desk's ordinary encoder bank replaces the Programmer attributes. On
the renderer camera, **Position** provides X, Y, Z, and Zoom; **Direction** provides Pan and Tilt.
The software encoders use the same tap, drag, wheel, arrow-key, and direct-value interactions as
the Programmer encoders. Attached encoders turn in fine steps, press-turn in coarse steps, and
open the same direct-value editor when pressed.

Positions are edited in **Show Patch**: physical patch and multi-patch placement provides every fixture's location and rotation, with **Open Stage Renderer** for visual feedback while patching. Add a truss, platform, curtain, or other scenery object from the **Venue** manufacturer in **Show Patch**; these visual-only fixtures receive `0.x` fixture IDs and no DMX address.

A Stage pane stores its own view and side independently, so a Live pane and a **Follow Preload**
pane can look at the rig from different places.

**Render quality** applies to the 3D Viz view and is a ladder of what is in the beam, each tier
adding to the one below it:

- **Draft** — the light cones, and nothing in them.
- **Standard** — and the gobos, so a projected pattern is a pattern rather than a plain cone.
- **High** — and the fall-off: a feathered field edge, the light dropping away across the pool, and
  shadows where a beam meets something opaque.
- **Ultra** — and the haze itself, drifting and uneven, so a beam through it varies along its
  length instead of running through a uniform slab.

On a fresh desk, **Environment brightness** starts at 5%, leaving an unlit rig just visible without
flattening the fixtures' output. Changing it is persisted with the desk layout; an existing saved
value, including zero, remains authoritative when that layout is reopened.

**Floor grid** lays a dark reference grid of lines on the ground plane, a metre apart, with the
centre lines drawn stronger. It is lines rather than a surface: it takes no light and hides nothing
under it.

**Background** is the colour behind the rig — the room rather than the show — and applies to every
Stage view. It is very dark and slightly blue by default.

Exposure is fixed. The Stage does not adapt to how much light the rig is producing the way an eye
does, because a desk has to answer "how bright is this" with the same picture every time: taking a
rig down has to look like taking a rig down, across the whole of the fader rather than the bottom
of it. The **Exposure** trim is the operator's own multiplier over that.

Stage receives authoritative Live and Preload output from the engine. A disconnected view freezes its last coherent state and reconnects without blocking Programmer, Playback, command handling, or physical output.

![Stage pane](../../assets/screenshots/panes/stage.png)

![Stage pane settings](../../assets/screenshots/panes/stage-settings.png)

![Full Stage window](../../assets/screenshots/workflows/stage-window-2d.png)

## Visualization

The Visualization pane monitors raw DMX or resolved fixture attributes without changing output.
Add rows and text, number, bar, or graph widgets in **Pane Settings → Visualization**. Configure the
source, range, scale, and display style; unavailable sources are identified rather than shown as
live values.

## Channels

The Channels pane is a direct programming bank ordered by Fixture ID. In the default **Intensity only** mode, it assigns one fader to each fixture. Faders fill each page from left to right across the first row and then continue on the second row. Each fader is labelled **Fixture _ID_**, followed by **Intensity**, and shows the resolved percentage. Moving it writes an intensity value into the programmer; tapping its card selects the fixture. A disabled fader replaces the attribute label with its current reason, such as **Empty position**, **Programmer values are loading**, or **Preload control is unavailable**. The reason disappears as soon as that fader becomes available.

In **Pane Settings → Channels**, choose **Intensity only** or **All channels** for that pane. **All channels** keeps fixtures grouped in Fixture ID order and shows each fixture's attributes in profile-authored order. Each fader identifies the fixture and attribute it controls. The full Channels window offers the same display choice in **Channel Settings**, along with the number of faders per row.

The full Channels window has previous/next controls and a page picker with at least eight channel pages. The compact pane hides that header, so it remains on channels 1-20 and cannot change pages from inside the pane. Use the full window when access beyond the first bank is required.

**Pane configuration:** only the common size and removal controls.

![Channels pane](../../assets/screenshots/panes/channels.png)

## Dynamics

The Dynamics pool holds numbered animated values. Tap a tile to toggle it for the current ordered
selection, or Shift-click to edit it; use **SET** followed by a Playback to assign it. The editor
shows **Lanes**, **Phase**, and **Speed** for attribute curves, target spreading, and BPM/Speed
Group control. **Take Selection** stores a target (a single Group stays live; other selections are
frozen); a stored target takes precedence over any active selection. **Clear Selection** removes
it. Use a Stage pane for Live or Follow Preload visualization.

**Pane configuration:** only common size and removal controls.

![Dynamics editor](../../assets/screenshots/panes/dynamics.png)
