# Fixture Types and GDTF

The fixture library is desk-wide and persists independently of show files. Open **Desk Setup > Shows & recovery > Open Fixture Library** to launch its modal and search, import, create, revise, and inspect complete fixture profiles. Library search follows the shared [search-bar layout](../30-Windows/01-desk-interface-and-windows.md#search-bars) and filters automatically with every typed character. Its optional Options dialog selects the fixture type. A profile is one revisioned fixture containing Generic information and an ordered set of modes; a patched show embeds the selected profile revision and mode so later library edits or deletion cannot change that show.

![Fixture-library manufacturers, modes, footprint, heads, and revision](../../assets/screenshots/workflows/fixture-library.png)

The shipped library includes separate conventional **Dimmer PAR Can**, **Dimmer Profile**, and **Dimmer Fresnel** fixture profiles, each with 8-bit and 16-bit dimmer modes. Shipped control fixtures leave body geometry to the renderer-owned default models, so the desk, demo show, Visualizer, and CAD all choose the same PAR, elongated profile, Fresnel with barn doors, moving-head, strip, laser, or effect body from fixture semantics. Portable visual-only Venue and Rigging objects keep their exact GLB geometry because a truss, stage deck, curtain, railing, or mirror ball cannot be represented by a generic lamp body. Choose the fixture profile for the physical lantern rather than treating these appearances as modes of one Dimmer profile.

## Transferable fixture packages

ToskLight has no fixture definitions compiled into the application. Every fixture supplied with the desk is an ordinary `.toskfixture` package, loaded through the same package reader used by **Import fixture**. You can export it, move it to another desk, keep it with a test, unpack and edit it, or replace it with a corrected package without rebuilding ToskLight.

Select a fixture and choose **Export fixture** to download its complete immutable revision. On another desk, choose **Import fixture** and select that file. A package keeps the stable fixture, mode, head, channel, function, split, and geometry IDs. Importing identical content is a no-op; importing changed content with the same fixture ID and manufacturer/name creates the next local revision. Reusing an existing ID for a different fixture family is rejected.

The shipped package directory currently provides an operator-focused Generic family and these manufacturer profiles with complete ordered mode lists:

- **Generic ACL** — a compact 200 mm-long, 80 mm-diameter conventional ACL lamp with aligned lens and beam geometry. **Blinder** provides the seven requested two-, four-, and eight-lamp one-, two-, and four-channel groupings. Each dimmer channel owns a non-master logical head and the corresponding physical emitters. **Fogger** provides Fog, Fan/Fog, and Fog/Fan modes; **Hazer** provides both two-channel orderings.
- **Generic RGBW, RGBWA, and RGBWAUV LED** — one canonical RGB-first emitter order with an 8-bit dimmer first, an 8-bit dimmer last, or a virtual dimmer. **Generic RGBCCT LED** provides the six useful placements of the RGB block, cold white, and warm white (`RGBCW`, `RGBWC`, `CRGBW`, `CWRGB`, `WRGBC`, and `WCRGB`), each with those same three dimmer choices. The library deliberately avoids factorial permutations of individual RGB emitters that do not represent normal fixture personalities.
- **Generic rare-capability references** — **Endless Pan Tilt** retains endless 16-bit axis representation on the canonical Pan and Tilt controls; **Beam Size and Edge** keeps Zoom independent from Softness; **Media Positioning** provides independent media-layer X and Y axes; **Flame Jet** supplies a ToskLight-authored single-nozzle demonstration; and **Kabuki Curtain** maps one raw slot to Reset, Hold, and a latched Release through its portable physics script. These are explicit transferable reference personalities: match their documented channel order to the device rather than treating them as manufacturer profiles.
- **Venue** visual-only profiles — 1 × 1 m, 2 × 1 m, and 1 × 0.5 m stage elements; correctly rising stage stairs; 1 m, 2 m, 3 m, 5 m, and 6 m curtains; **Disco Ball 50 cm**; and **Crowd Area**. One-, two-, three-, and four-point truss and pipe profiles use the separate **Rigging** type. Crowd Area supplies all nine Sitting, Standing still, and Dancing × Sparse, Medium, and Dense modes and stores independent width and depth with the show. The conventional scenery archives include portable photographs and metre-authored GLB geometry; the desk displays its built-in Venue or Rigging type icon. Crowd Area is rendered procedurally from its portable crowd contract.
- **ToskLight** product and Visualizer profiles — **Audio Player** is an Internal fixture: one independently programmable Audio service voice with a regular fixture ID and no DMX address. It is addressed through the canonical Media attributes — Media Folder, Media File, Play mode, and Volume — so the Media encoder group and the Media pane control it exactly like any other media source. Play mode carries transport and repeat together: a looping mode repeats the file, a once mode plays it through, and Stop and Pause hold the voice silent. Stop is the patched default. Play mode names every mode it can be in — Loop, Reverse, Bounce, the Once and Reverse once end states, their tempo-synced counterparts, Stop, and Pause — so the encoder and the Media pane show the mode by name instead of a percentage, and each mode can be chosen directly or generated as a preset. Media Server play mode is named the same way. Shows patched before this change keep their stored Audio Folder/File, Transport, Repeat, and Volume attributes and continue to play. **Media Server** provides two complete personalities: 118 slots for two layers and 352 slots for eight layers. Each 39-slot layer is an independently programmable logical head; the trailing 40-slot output block belongs to the shared master head and exposes Output, Geometry, Mask position, Shapers, and Colour. Existing shows retain their embedded 75-slot legacy or 89/323-slot mask-position snapshots, while new patches use the complete master personality. **Visualizer Camera** keeps the stable 17-slot X/Y/Z, Yaw/Pitch/Roll, and Zoom wire contract, while **Visualizer Laser** provides the packaged demo laser and its scan program. This manufacturer is reserved for implemented ToskLight-owned product fixtures; planned further Visualizer fixtures do not appear until their capabilities exist.

- **JB-Lighting JBLED A7** — Standard and Compressed RGB personalities in 8-bit and 16-bit color,
  with the complete shutter-effect table and a documented open shutter as the safe home and
  Highlight look.
- **Martin MAC 250 Entour** — 16 Bit and 16 Bit Extended.
- **High End Systems Trackspot** — the classic seven-channel mirror scanner in low- and high-resolution DMX personalities.
- **Showtec Sunstrip Active DMX** — ten independently controlled tungsten lamps.
- **Showtec Sunstrip LED RGB 42206** — ten independently controlled RGB pixels.
- **ROBE Robin DLS Profile**, **Robin 600X LEDWash**, **Robin LEDBeam 150**, **Robin 300 LEDWash**, and **Robin DLF Wash** — every documented manufacturer personality. The 600X and 300 zone modes expose their three concentric RGBW zones as logical heads.
- **Claypaky Sharpy**, **ETC Source Four LED Series 2 Lustr**, **CHAUVET Professional COLORado 1 Solo**, and **GLP JDC1**. JDC1 SPix modes expose all twelve RGB plate pixels and twelve white beam segments as logical heads.

The Source Four LED Series 2 configuration can independently enable Strobe, Fan Control, and Plus Seven. Its package contains the canonical console personalities, including the common fully enabled and Plus Seven variants, instead of multiplying every fixture-menu option permutation into a separate mode.

Channel order, footprints, fine-byte slots, safe defaults, and physical ranges come from the corresponding manufacturer DMX charts. Shipped packages are not privileged or reserved: after loading, they are normal desk-library profiles. When a newer shipped package is installed, ToskLight updates it only if its last package-installed revision is still current. An operator-created later revision is preserved and reported instead of being overwritten.

Package format, asset restrictions, JSON structure, generated projections, and archive validation
are fixture-author documentation. For an operator, the important rule is simple: export a working
fixture as the safest template, keep its identity when correcting that fixture, and import the
result as a new immutable revision. If an import contains an unknown attribute, map it to a
compatible desk attribute or create a custom one before importing; existing revisions remain
readable and exportable.

## Import GDTF

Choose **Import GDTF** and select a `.gdtf` archive. ToskLight normalizes the supported modes, channels, physical information, emitters, capabilities, geometry, and model into a fixture profile and retains the original GDTF bytes beside every resulting immutable revision. MVR export can therefore use the retained source instead of reconstructing an archive from lossy normalized data.

GDTF interchange is preserved in both directions through the show workflow: import the original `.gdtf` into the Fixture Library, then export an MVR to carry the retained GDTF archives for the fixtures in that rig. ToskLight does not reconstruct a new standalone GDTF archive from edited normalized profile data.

The same canonical-attribute preflight applies before a newly normalized GDTF profile is stored. An
import or migration error leaves the original data untouched and appears in the open import dialog
or as an actionable warning in the Fixture Library. Do not delete the source row until the warning
has been investigated or the fixture has been recovered.

![Import every mode from a local GDTF archive](../../assets/screenshots/workflows/fixture-library-import.png)

## Create or edit a fixture profile

**Create fixture** opens a blank profile with one mode named **Default** and one editable main head. **Edit as new revision** opens the same editor with the chosen revision. The title bar contains **Generic**, **Modes**, **Save fixture**, and Close; the Modes tab also adds **Add mode** at the top right. There is no footer Cancel action.

Closing an unchanged editor is immediate. Closing a changed editor through Close, Escape, or the backdrop asks whether to **Stay** or **Discard changes**. Saving an existing profile first asks to **Save and create revision**. A failed or stale save keeps the editor open and explains the problem.

### Generic

Generic information includes manufacturer, full and short names, fixture type, notes, stage icon, photograph, optional visualizer GLB model, dimensions, weight, power consumption, color temperature, luminous output, and beam angle. Notes, photograph, and visualizer are shown side by side; drag the visualizer preview to inspect the GLB from another angle and scroll to zoom. Manufacturer remains free text. Use its lookup button to search the unique desk-library manufacturers with the shared full-text keyboard and fill the field without saving the editor.

### Optics

Optics optionally refine the Stage appearance. Leave them empty to use the normal appearance for
the fixture type; set relative output, edge, field uniformity, and light-source dimensions only
when the fixture needs a different result.

### Modes and heads

Modes have stable identities, names, notes, and complete channel configuration. Each row in the full-width Modes list edits that mode's name and notes directly and summarizes its heads, logical channels, and splits. Add modes from the title bar; remove and reorder them with drag-and-drop or the explicit move buttons. The final mode cannot be removed. **Edit channels** opens the nested tabs in this order: **Heads**, **Channels**, **Color**, and **Geometry**.

Every head has a stable identity and an optional master/shared designation. Heads describe logical emitters, not patch blocks: one head may own channels in several independently patched splits. At most one head is master/shared. A head that still owns channels cannot be removed until those channels are reassigned or removed.

A split is an independently patchable address block configured in Channels. Give each split its footprint there and assign every physical channel to a split; each split gets its own optional universe and address in Show Patch. An unpatched split remains selectable, programmable, and visible but emits no DMX.

![Nested fixture mode editor with Heads, Channels, Color, and Geometry tabs](../../assets/screenshots/workflows/fixture-library-mode-editor.png)

### Channels

Channels are arranged by split and use the selected attribute registry. Set the physical resolution,
default and Highlight values, and any continuous, indexed, or control functions. The editor blocks
invalid footprints and overlapping component slots. Choose a safe home and Highlight look, then
verify them on the real fixture. Detailed raw-value, color-system, and control-action authoring is
fixture-developer documentation.

### Color

Configure the fixture's additive, subtractive, or wheel color system, then use **Generate portable
presets** in **Control → Special Dialog** when fixed or indexed choices should be added to the show.

### Geometry

Choose a suitable fixture geometry and use the preview to confirm the Stage appearance. Detailed
model hierarchy, emitter, pivot, and projection authoring is fixture-developer documentation.

## Revisions and compatibility

The server assigns revision numbers atomically and rejects concurrent edits. Open **Revision history** to inspect immutable revisions, edit an older revision as a new one, or delete an unused revision. Deletion warns when a patched show embeds that revision; the show's snapshot remains intact even if deletion is confirmed.

Legacy library entries migrate through an explicit schema-v1 reader. Compatible modes are combined only when their fixture-family metadata agrees; conflicts remain separate and produce a visible warning. Installations that predate transferable packages remove only the historical code-owned catalog rows, then load the equivalent `.toskfixture` files as ordinary profiles. User-authored profiles are never claimed by manufacturer or model name.

During legacy or GDTF migration, intensity, RGB/RGBW/additive, CMY/subtractive, and identifiable Open/White wheel channels receive the same deterministic physical Highlight defaults; unmatched wheel, Position, and unrelated channels retain their source defaults. Existing authored schema-v2 Highlight raw values are preserved exactly. A patched fixture without a per-instance Highlight override map inherits those values from its embedded profile revision. Later desk-library edits therefore do not silently change the Highlight Look already stored with a show.

![Create a complete revisioned fixture profile](../../assets/screenshots/workflows/fixture-library-create.png)
