# Fixture Library

The fixture library is desk-wide and persists independently of show files. Open **Desk Setup > Shows & recovery > Open Fixture Library** to launch its modal and search, import, create, revise, and inspect complete fixture profiles. Library search follows the shared [search-bar layout](../01-application-layout.md#search-bars) and filters automatically with every typed character. Its optional Options dialog selects the fixture type. A profile is one revisioned fixture containing Generic information and an ordered set of modes; a patched show embeds the selected profile revision and mode so later library edits or deletion cannot change that show.

![Fixture-library manufacturers, modes, footprint, heads, and revision](../assets/screenshots/workflows/fixture-library.png)

The shipped library includes separate conventional **Dimmer PAR Can**, **Dimmer Profile**, and **Dimmer Fresnel** fixture profiles, each with 8-bit and 16-bit dimmer modes. Their transferred GLB models use a PAR housing with a square gel frame, an elongated ellipsoidal-profile housing, and a Fresnel housing with four external barn doors respectively. Choose the fixture profile for the physical lantern rather than treating these appearances as modes of one Dimmer profile.

## Transferable fixture packages

ToskLight has no fixture definitions compiled into the application. Every fixture supplied with the desk is an ordinary `.toskfixture` package, loaded through the same package reader used by **Import fixture**. You can export it, move it to another desk, keep it with a test, unpack and edit it, or replace it with a corrected package without rebuilding ToskLight.

Select a fixture and choose **Export fixture** to download its complete immutable revision. On another desk, choose **Import fixture** and select that file. A package keeps the stable fixture, mode, head, channel, function, split, and geometry IDs. Importing identical content is a no-op; importing changed content with the same fixture ID and manufacturer/name creates the next local revision. Reusing an existing ID for a different fixture family is rejected.

The shipped package directory currently provides the exhaustive Generic family and these manufacturer profiles with complete ordered mode lists:

- **Generic Blinder** — the seven requested two-, four-, and eight-lamp one-, two-, and four-channel groupings. Each dimmer channel owns a non-master logical head and the corresponding physical emitters. **Fogger** provides Fog, Fan/Fog, and Fog/Fan modes; **Hazer** provides both two-channel orderings.
- **Venue** visual-only profiles — 1 × 1 m, 2 × 1 m, and 1 × 0.5 m stage elements; correctly rising stage stairs; one-, two-, three-, and four-point truss; and 1 m, 2 m, 3 m, 5 m, and 6 m curtains. Their modes select leg height, target stage height, truss/pipe length, or curtain height. Every archive includes portable icon, photograph, and metre-authored GLB geometry.

- **JB-Lighting JBLED A7** — Standard and Compressed RGB personalities in 8-bit and 16-bit color.
- **Martin MAC 250 Entour** — 16 Bit and 16 Bit Extended.
- **High End Systems Trackspot** — the classic seven-channel mirror scanner in low- and high-resolution DMX personalities.
- **Showtec Sunstrip Active DMX** — ten independently controlled tungsten lamps.
- **Showtec Sunstrip LED RGB 42206** — ten independently controlled RGB pixels.
- **ROBE Robin DLS Profile**, **Robin 600X LEDWash**, **Robin LEDBeam 150**, **Robin 300 LEDWash**, and **Robin DLF Wash** — every documented manufacturer personality. The 600X and 300 zone modes expose their three concentric RGBW zones as logical heads.
- **Claypaky Sharpy**, **ETC Source Four LED Series 2 Lustr**, **CHAUVET Professional COLORado 1 Solo**, and **GLP JDC1**. JDC1 SPix modes expose all twelve RGB plate pixels and twelve white beam segments as logical heads.

The Source Four LED Series 2 configuration can independently enable Strobe, Fan Control, and Plus Seven. Its package contains the canonical console personalities, including the common fully enabled and Plus Seven variants, instead of multiplying every fixture-menu option permutation into a separate mode.

Channel order, footprints, fine-byte slots, safe defaults, and physical ranges come from the corresponding manufacturer DMX charts. Shipped packages are not privileged or reserved: after loading, they are normal desk-library profiles. When a newer shipped package is installed, ToskLight updates it only if its last package-installed revision is still current. An operator-created later revision is preserved and reported instead of being overwritten.

### Package layout

A `.toskfixture` file is a ZIP archive with this portable layout:

```text
fixture.json
assets/photograph.png    optional PNG, JPEG, or WebP
assets/icon.png          optional PNG, JPEG, or WebP stage icon
assets/model.glb         optional self-contained glTF Binary 2.0 model
```

`fixture.json` is UTF-8 JSON. The outer document is deliberately small and can be produced with a normal text editor or an AI fixture-building workflow:

```json
{
  "$schema": "https://tosklight.app/schemas/fixture-package-v1.json",
  "format": "tosklight.fixture",
  "format_version": 1,
  "profile": {
    "schema_version": 2,
    "id": "a-stable-uuid",
    "revision": 1,
    "manufacturer": "Example",
    "name": "Example Profile",
    "patch_policy": "dmx",
    "photograph_asset": "assets/photograph.png",
    "stage_icon_asset": "assets/icon.png",
    "model_asset": "assets/model.glb",
    "model_units": "metres",
    "modes": []
  }
}
```

The `profile` is the same schema-v2 fixture profile edited by the Fixture Library and embedded in patched shows. A DMX profile uses `"patch_policy": "dmx"`, a 1–512 slot split footprint, and its channels. A scenic object uses `"patch_policy": "visual_only"`, a zero-footprint split, no channels/color/control actions, and geometry; the desk then guarantees that it cannot receive a universe, address, or direct-control endpoint. `"model_units": "metres"` preserves authored GLB dimensions exactly, while the backward-compatible `"auto"` value normalizes a conventional lamp model to its profile dimensions. Use an exported package as the safest complete template. Asset fields are either `null` or relative paths under `assets/`. Do not use absolute paths, parent paths, data URLs, external GLB textures, or network URLs inside a package. The package must contain exactly the referenced files and no unreferenced extras.

To author one manually, export a similar fixture, rename `.toskfixture` to `.zip`, unpack it, edit `fixture.json`, add or replace assets, ZIP `fixture.json` and `assets/` at the archive root, then restore the `.toskfixture` extension. Keep existing UUIDs when correcting the same fixture; generate new UUIDs for a genuinely different fixture, mode, head, channel, or function. Never derive identity from display text or DMX row position.

For safety, import rejects unsafe or duplicate paths, symbolic links, unsupported compression, undeclared files, invalid raster data, non-self-contained GLBs, archives over 64 MiB compressed or 128 MiB expanded, more than 32 entries, and manifests over 64 MiB. The supported MIME type is `application/vnd.tosklight.fixture+zip`.

## Import GDTF

Choose **Import GDTF** and select a `.gdtf` archive. ToskLight normalizes the supported modes, channels, physical information, emitters, capabilities, geometry, and model into a fixture profile and retains the original GDTF bytes beside every resulting immutable revision. MVR export can therefore use the retained source instead of reconstructing an archive from lossy normalized data.

An import or migration error leaves the original data untouched and appears as an actionable warning in the Fixture Library. Do not delete the source row until the warning has been investigated or the fixture has been recovered.

![Import every mode from a local GDTF archive](../assets/screenshots/workflows/fixture-library-import.png)

## Create or edit a fixture profile

**Create fixture** opens a blank profile with one mode named **Default** and one editable main head. **Edit as new revision** opens the same editor with the chosen revision. The title bar contains **Generic**, **Modes**, **Save fixture**, and Close; there is no footer Cancel action.

Closing an unchanged editor is immediate. Closing a changed editor through Close, Escape, or the backdrop asks whether to **Stay** or **Discard changes**. Saving an existing profile first asks to **Save and create revision**. A failed or stale save keeps the editor open and explains the problem.

### Generic

Generic information includes manufacturer, full and short names, fixture type, notes, stage icon, photograph, optional visualizer GLB model, dimensions, weight, and power consumption. Manufacturer remains free text. Use its lookup button to search the unique desk-library manufacturers with the shared full-text keyboard and fill the field without saving the editor.

### Modes and heads

Modes have stable identities, names, notes, and complete channel configuration. Add, remove, and reorder modes with drag-and-drop or the explicit move buttons; the final mode cannot be removed. **Edit channels** opens the nested tabs in this order: **Heads**, **Channels**, **Color**, and **Geometry**.

Every head has a stable identity, one split, and an optional master/shared designation. At most one head is master/shared. Several heads may share a split. A head that still owns channels cannot be removed until those channels are reassigned or removed.

A split is an independently patchable address block. Give each split its footprint here; each gets its own optional universe and address in Show Patch. An unpatched split remains selectable, programmable, and visible but emits no DMX.

![Nested fixture mode editor with Heads, Channels, Color, and Geometry tabs](../assets/screenshots/workflows/fixture-library-mode-editor.png)

### Channels

For multi-split modes, Channels shows one accordion per split and keeps exactly one open. A single-split mode shows its table directly. Rows support touch drag-and-drop and explicit keyboard/accessibility move controls.

The primary DMX slot is derived from row order. Fine, Third byte, and Fourth byte contain explicit component slots for 16-, 24-, and 32-bit channels; reserved component slots are skipped when later primary slots are calculated. Default, Highlight, function ranges, and fixed values are exact raw integers at the selected resolution. Saving is blocked when slots overlap, exceed 512, do not fit the resolution, or lie outside the split footprint.

**Highlight raw** defines the profile-level identification look used while that channel's fixture or logical head is highlighted. A newly derived default uses full conventional intensity and physical white: direct RGB/RGBW white endpoints, calibrated additive or subtractive D65 white, zero CMY filtration, and the midpoint of a discrete wheel slot explicitly named Open, White, Clear, or No Color. Inversion is included when choosing a raw endpoint. If no white wheel slot can be identified, that channel keeps its safe default instead of using an arbitrary maximum. Set any required shutter-open channel deliberately, and leave Position and unrelated or hazardous functions at an appropriate safe/default raw value. Validate the complete look on the real fixture; Highlight raw is physical output configuration, not a normalized programmer value.

Changing a newly added channel's attribute, additive/subtractive calibration, or discrete-wheel Open/White slot recalculates its semantic Highlight default only while the field still contains the previous automatic value. This lets an untouched wheel channel move from its safe default to the Open/White midpoint when that slot is defined. Once an operator enters an exact Highlight raw value, later channel or Color-tab edits preserve it. Existing schema-v2 revisions are likewise never renormalized on load or save.

Each channel chooses its head and canonical attribute and can configure physical range/unit, invert, snap, virtual-intensity reaction, sequence/group/grand-master reactions, and prioritized functions. **Static** channels normally output their default and use their Highlight value only while identified. Snap channels bypass programmer, Cue, Move in Black, and safety transitions.

A physical channel may contain ordered continuous, fixed, indexed-color/gobo, or control functions. Only an explicitly programmed function claims it; the highest configured priority wins, and releasing it reveals the next claim or channel default. Typed control actions can atomically set several channels and be latched, momentary, or timed. Fixed and indexed functions appear in the direct programmer picker.

### Color

Color remains an abstract XYZ request across fixtures. Additive systems bind measured XYZ or xyY emitters, maximum level, response curve, and visible-color participation. Subtractive systems bind CMY channels. Discrete wheels store portable semantic color IDs, local labels, DMX ranges, and optional measured color. The engine uses bounded non-negative mixing and deterministic gamut clipping, with direct RGB or CMY fallback when calibration is unavailable. UV and other non-visible emitters participate only when explicitly programmed.

Portable presets are never created merely by patching. Use the explicit **Generate portable presets** action for the selected fixtures to add stable fixed/indexed semantic choices to the show.

### Geometry

Start with **Fixed fixture**, **Moving head**, **Bar**, **Matrix**, or **Shared-pan multi-head**, then edit the generated hierarchy. Parts have parents, transforms, pivots, optional GLB-node bindings, and attribute-driven rotation or translation. Emitters attach to any part and define logical head, origin, orientation, beam and field angles, feather, focus, and point/matrix/ring/strip/explicit-pixel layout.

The editor preview and Stage visualizer use the configured graph, multiple emitters, resolved motion attributes, and the same resolved color used for output. This supports a shared pan ancestor with independent tilt children and multiple offset beam sources instead of assuming one hard-coded beam.

## Revisions and compatibility

The server assigns revision numbers atomically and rejects concurrent edits. Open **Revision history** to inspect immutable revisions, edit an older revision as a new one, or delete an unused revision. Deletion warns when a patched show embeds that revision; the show's snapshot remains intact even if deletion is confirmed.

Legacy library entries migrate through an explicit schema-v1 reader. Compatible modes are combined only when their fixture-family metadata agrees; conflicts remain separate and produce a visible warning. Installations that predate transferable packages remove only the historical code-owned catalog rows, then load the equivalent `.toskfixture` files as ordinary profiles. User-authored profiles are never claimed by manufacturer or model name.

During legacy or GDTF migration, intensity, RGB/RGBW/additive, CMY/subtractive, and identifiable Open/White wheel channels receive the same deterministic physical Highlight defaults; unmatched wheel, Position, and unrelated channels retain their source defaults. Existing authored schema-v2 Highlight raw values are preserved exactly. A patched fixture without a per-instance Highlight override map inherits those values from its embedded profile revision. Later desk-library edits therefore do not silently change the Highlight Look already stored with a show.

![Create a complete revisioned fixture profile](../assets/screenshots/workflows/fixture-library-create.png)
