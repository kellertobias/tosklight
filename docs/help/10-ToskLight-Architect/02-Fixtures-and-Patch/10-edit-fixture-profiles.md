# Create and revise fixture profiles

**Create fixture** opens a blank profile with one mode named **Default** and one editable main head. **Edit as new revision** opens the same editor with the chosen revision. The title bar contains **Identity**, **Simulation**, **Geometry**, **Modes**, **Save fixture**, and Close; the Modes tab also adds **Add mode** at the top right. There is no footer Cancel action.

Closing an unchanged editor is immediate. Closing a changed editor through Close, Escape, or the backdrop asks whether to **Stay** or **Discard changes**. Saving an existing profile first asks to **Save and create revision**. A failed or stale save keeps the editor open and explains the problem.

### Identity

Identity covers who the fixture is: manufacturer, full and short names, fixture type, notes, stage icon, photograph, and optional visualizer GLB model. Notes, photograph, and visualizer are shown side by side; drag the visualizer preview to inspect the GLB from another angle and scroll to zoom. Manufacturer remains free text. Use its lookup button to search the unique desk-library manufacturers with the shared full-text keyboard and fill the field without saving the editor.

### Simulation

Simulation is what the Stage needs in order to draw the fixture. **Body** chooses the shape it is
drawn as, **Physical** is how
the lantern is built: dimensions, weight, power consumption, connectors, light source, colour
rendering index, and lens. **Optics** is what comes out of it: colour temperature, luminous output,
beam angle, relative output, edge, field uniformity, and light-source dimensions.

Colour temperature, luminous output, and beam angle describe the light rather than the lantern, so
they are part of Optics. A profile written before that keeps working untouched: the values are read
from where they used to be stored and belong to Optics from then on, including in the profile
revision a patched show has already embedded.

Leave any optics field empty to use the normal appearance for the fixture type.

Every physical figure is entered in a fixed unit and precision:

| Field | Unit | Precision |
| --- | --- | --- |
| Width, Height, Depth | millimetres (mm) | whole numbers |
| Weight | kilograms (kg) | up to two decimal places |
| Power consumption | watts (W) | whole numbers |
| Sharpness, Uniformity | percent (%), 0 to 100 | one decimal place, always shown as e.g. `85.0` |

A figure beyond its precision (`420.5` mm, `24.555` kg) is kept on screen and named under the
field, and **Save fixture** lists it instead of saving. Nothing is rounded for you. Every shipped
fixture is stored at this precision. A profile imported from before these rules, with a dimension
such as `498.2` mm, still loads and patches unchanged; its off-precision figures are named when you
open it in the editor and must be corrected before that revision can be saved.

**Generic body** names one of the bodies ToskLight ships (the full list is in the
[Model Catalogue](../../99-Appendix/01-model-catalogue.md)) so a PAR 64 long nose is drawn as one
rather than as whatever its declared type suggests. Press the field to open the body picker, which
shows a picture of every body grouped by kind and can be searched by name. Leave it on *Guess from
the fixture type* and
the body is inferred from the declared type and
the channels the mode has, which is right for most fixtures and cannot tell a PAR 64 from a PAR 16
or a two-cell blinder from an eight. A fixture that ships its own visualizer model is drawn with
that model, whatever the body says.

**Mounting** is what holds the lantern up, and it is what ToskLight Architect rigs it by: drag a
lamp near a truss and the pipe line of its clip lands on the chord the clip reaches. **Hangs by**
is a hook clamp over a pipe, a yoke or bracket bolted down, or nothing at all for a fixture that
stands (a hazer, a floor can) which is then never picked up by a truss. The rest is six figures
in whole millimetres from the centre of the body: **Pipe across**, **deep** and **up** put the bar
where it ends up, and **Clip width**, **depth** and **height** say how big the hardware around it
is, which is how near a pipe has to come to catch it. The clip is taken to hang directly under the
pipe, as a hook clamp does.

Shipped fixture profiles include mounting data. Fill this in for a model you
imported yourself, where only you know where the clamp is. The figures are read against the
fixture's declared Physical size, so a lamp placed larger or smaller in the plan keeps its clamp in
proportion; a fixture with no declared size takes the clip as it stands.

### Modes and heads

Modes have stable identities, names, notes, and complete channel configuration. Each row in the full-width Modes list edits that mode's name and notes directly and summarizes its heads, logical channels, and splits. Add modes from the title bar; reorder them with drag-and-drop. Removing a mode asks for confirmation first, because its heads, channels, and functions go with it; the final mode cannot be removed. **Edit channels** opens the nested tabs in this order: **Heads**, **Channels**, **Control actions**, **Color**, and **Emitters & Motion**.

The path under each editor window's title (for example *Acme Orbit › Modes › Default › Channels*) shows where you are among the nested windows. Press an earlier step to close every window below
it.

Every head has a stable identity and an optional master/shared designation. Heads describe logical emitters, not patch blocks: one head may own channels in several independently patched splits. At most one head is master/shared. A head that still owns channels cannot be removed until those channels are reassigned or removed.

A split is an independently patchable address block configured in Channels. Its footprint follows the slots its channels take up, and every physical channel belongs to one split; each split gets its own optional universe and address in Show Patch. An unpatched split remains selectable, programmable, and visible but emits no DMX.

![Nested fixture mode editor with Heads, Channels, Color, and Geometry tabs](../../assets/screenshots/workflows/fixture-library-mode-editor.png)

### Channels

Channels are arranged by split, one row per DMX slot, in the order the manufacturer's chart lists
them. Every cell shows its value; press it to open the list or number keypad that changes it. Each
row has these columns:

- **Attribute** opens a picker for the encoder group, activation group, and attribute. Choose **Static** for a slot without a Programmer control.
- **Level** selects **Coarse**, **Fine**, **Ultra**, or **Extreme**. To create 16-bit Pan, assign Pan to two slots and set the second to Fine. It becomes Pan's second byte and reads *↳ Pan*. A Fine slot needs a matching Coarse slot on the same head. Returning it to Coarse creates a separate channel. Adding or removing a byte retains the meaning of defaults, Highlight, and function values.
- **Default** and **Highlight** open a number keypad for raw values.
- **Mapping** edits the physical minimum, maximum, and unit. The function table sits below the range. Use **Details** for behavior-specific fields, such as fixed-value labels or angular motion.
- **Invert** and **Snap** control inversion and whether the value jumps instead of fading.
- **Masters** shows how each channel follows the Cue, Group, Grand Master, and Blackout. Level channels follow those masters before conversion to DMX and read *Level*. Other channels follow virtual intensity only when configured to do so.

Every light-emitting head without a physical Intensity channel has a virtual dimmer. Program, record, and master it like physical intensity. Its color emitters start on **Follow** under **React to Virtual Intensity**; emitters on a head with a physical dimmer start on **Ignore**. CMY flags also start on Ignore.

**Inverse** makes the channel full at zero virtual intensity and zero at full virtual intensity. The table marks Follow as *VI* and Inverse as *−VI*. A Cue without an Intensity value for that head cannot dim it from its fader.

The table is the tab's only scrolling area: it scrolls up, down, and sideways under a fixed column
header, while the mode editor's title bar and split headers stay in place. In a channel's
**Mapping** window the function table likewise scrolls on its own below the physical range.

Only the Coarse row carries these settings; a further byte shows a dash. Drag a row by the handle
beside its slot number, or use its move buttons. The red bin removes a slot after asking, and the
slots after it move up.

**Add split** and **Add channel** sit in the mode editor's title bar while **Channels** is open; a
new channel goes into the split that is open. With more than one split, each has a one-line header
with its name and how many channels it holds (press it to open that split) and its own move and
remove buttons. A split's footprint follows its slots, so it is never typed. Typed control actions
have their own **Control actions** tab, with **Add control action** in the title bar. The editor blocks invalid
footprints and overlapping slots. Choose a safe home and Highlight look, then verify them on the
real fixture. Detailed raw-value, color-system, and control-action authoring is fixture-developer
documentation.

A **Static output** channel has no Programmer control. It transmits its authored Default raw value
during normal output and its separately authored Highlight raw value while Highlight is active, so
a normally fixed control slot can still switch a fixture display or identification mode for
Highlight. A per-fixture Highlight override may replace that Highlight raw value.

### Emitters & Motion

A mode says which of its heads owns which of the fixture's emitters, and which attribute moves each
of its moving parts; that is the whole of what a personality says about geometry. An emitter no
head owns is not lit in that mode: which is how one personality gives every ring of a wash a head
of its own and another drives them all together, without either describing the lantern twice.

Under **Moving parts**, press a part to choose which of this mode's channels drives it. Only
attributes the mode's channels carry are offered, each once with the slots that carry it, plus
**Not driven** to leave the part resting where it is drawn. A template's pan arm and tilt head start out
driven by Pan and Tilt in every mode.

## Geometry

Geometry belongs to the fixture, not to one of its personalities: a moving head has the same yoke
whichever mode it is patched in. It is edited once, in the profile's own **Geometry** tab, and every
mode binds its heads to the emitters described there.

A profile written before this carried a whole graph on each mode. Reading one moves the graph to
the fixture and leaves the head each emitter named behind as that mode's binding, so nothing an
operator authored is lost and no show needs repatching. Modes are compared by what they describe
rather than by the identifiers they were written with, and a mode that adds parts the others leave
out (a wash whose zone personalities drive four heads where its plain ones drive one) is read as
that personality driving more of the same lantern.

Where a profile's modes genuinely describe different geometry the profile is left exactly as it
was.

Choose a suitable fixture geometry and use the preview to confirm the lamp's shape. Detailed
model hierarchy, emitter, pivot, and projection authoring is fixture-developer documentation.

The Geometry tab fills the editor window: the part and emitter list, the selected part's
properties, and the live 3D preview sit side by side, and each scrolls on its own. The preview
shows the lamp itself (its parts, their transforms and the emitter faces) framed on a dark-blue
background; it draws no beam, so check how the light falls on Stage. A part's
properties are split into tabs: **Generic** (name, parent part, GLB node binding, and **Remove
part**), **Translation**, **Rotation**, **Scale**, **Pivot**, and **Animate**.

**Animate** makes a part move and describes how: **Motion kind**, **Motion axis**, the physical
minimum and maximum, and its speeds. It does not name the attribute that drives the part; each mode
chooses that under **Emitters & Motion**.

An axis that moves (a yoke's pan, a head's tilt) can also declare how fast it actually travels:
**Top speed**, **Acceleration**, and **Deceleration**, in degrees per second for a rotation and in
millimetres for a translation. A real lantern does not arrive instantly, and two
fixtures given the same position at the same moment do not arrive together. Leave the figures
empty and the axis moves as fast as it is told to, which is how every fixture behaved before.

## Save a revision

The server assigns revision numbers atomically and rejects concurrent edits. Open **Revision history** to inspect immutable revisions, edit an older revision as a new one, or delete an unused revision. Deletion warns when a patched show embeds that revision; the show's snapshot remains intact even if deletion is confirmed.

See [Color models](12-fixture-color-models.md), [Physical channel mapping](11-physical-channel-mapping.md), and [Installed calibration](13-installed-calibration.md) for measured behavior.
