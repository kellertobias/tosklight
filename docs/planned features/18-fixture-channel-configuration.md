# Fixture Channel Configuration

Replace the current per-mode fixture definitions and compact channel syntax with a desk-wide, revisioned fixture profile containing generic metadata, ordered modes, heads, independently patchable splits, semantic channels, color systems, control actions, and 3D geometry.

This feature includes the complete persistence, migration, editor, patching, DMX-resolution, programmer-control, generated-preset, and visualization behavior described below. Already-patched shows must remain insulated from later fixture-library revisions.

## Fixture profile and revisions

A fixture profile is the atomic revision unit. It contains:

- manufacturer;
- fixture name and short name;
- fixture type;
- notes;
- fixture photograph, stage icon, and GLB model asset;
- physical width, height, depth, weight, and power consumption in watts; and
- an ordered list of modes.

Creating a fixture creates revision 1. Editing an existing fixture never overwrites it. Saving an edit asks whether the operator wants to save the fixture and create a new revision. The server, not the client, assigns the next revision and rejects a revision conflict. The complete fixture, including every mode, is versioned atomically.

The desk-wide fixture library stores fixture profiles independently of show files. A patched show embeds a portable snapshot of its selected fixture revision and mode, so editing or deleting a library entry does not alter an existing show.

## Shared Create and Edit fixture modal

Create and Edit use the same modal and layout. Create opens an empty profile with one mode named **Default** and one editable main head. Edit opens the selected fixture revision with its current data.

The modal title bar contains the **Generic** and **Modes** tabs. **Save fixture** is a button on the right side of the title bar. There is no footer Cancel button.

- Saving a new fixture stores it immediately.
- Saving an edited fixture opens the new-revision confirmation before anything is stored.
- Closing an unchanged modal closes it immediately.
- Closing a changed modal through the title-bar close control, Escape, or the backdrop asks whether to **Stay** or **Discard changes**.
- Failed and conflicting saves keep the editor open and show an actionable error.

### Generic tab

The Generic tab is divided into Identity, Physical, and Notes and picture sections.

Identity contains manufacturer, fixture name, fixture short name, fixture type, and fixture icon. Manufacturer remains a normal text input so a new manufacturer can be entered. A dedicated magnifying-glass button opens a manufacturer lookup modal. The lookup contains a search field, the shared full-text keyboard, and a case-insensitive list of the unique manufacturers already present in the fixture library. Selecting a manufacturer fills the field without saving or closing the fixture editor.

Physical contains width, height, and depth in millimetres, weight in kilograms, and power consumption in watts.

Notes and picture contains free-form fixture notes and a fixture photograph with preview, replace, and remove actions. The photograph is distinct from the stage icon and from the optional GLB model used by the visualizer.

### Modes tab

Each mode has a stable identity, name, notes, and an **Edit channels** action. Modes can be added, removed, and reordered by touch-capable drag and drop. Explicit move controls provide the same behavior for keyboard and accessibility users. The final mode cannot be removed.

Deleting a mode affects only the unsaved editor draft. Saving that deletion as an edit creates a new fixture revision; older revisions and shows using them remain intact.

## Mode editor

**Edit channels** opens a nested mode editor. Its tabs appear in this order: **Heads**, **Channels**, **Color**, and **Geometry**.

### Heads

Heads have stable identities, a name, an optional master/shared designation, and exactly one split number. At most one head in a mode can be the master/shared head. Multiple heads may use the same split number.

Heads can be added, removed, and reordered. A head that still owns channels cannot be removed until those channels are removed or reassigned.

A split is an independently patchable address block within one logical fixture. Every distinct split number in a mode receives its own optional universe and address when the fixture is patched. An unpatched split remains part of the fixture: its heads remain selectable, programmable, and visible, but that split emits no DMX.

### Channels

When a mode has multiple splits, the Channels tab shows one accordion section per split. Exactly one section is open at all times. With only one split, the table is shown directly without an accordion wrapper.

Each split contains an ordered table with one row per logical channel. Rows can be added, removed, and reordered through the same touch, keyboard, and accessibility paths used for modes and heads.

The first column displays the channel's primary DMX slot. Primary slots are derived from row order by choosing the next slot that has not been reserved as a secondary byte. Dedicated Fine, Third byte, and Fourth byte columns contain explicit DMX slot numbers for 16-, 24-, and 32-bit channels. Those reserved slots do not appear as separate table rows and are skipped when primary slots are calculated. Reordering rows recalculates their primary slots but does not silently change their explicitly assigned secondary slots.

Saving is blocked when component slots are duplicated, outside the split footprint, invalid for the selected resolution, or cause the split to exceed 512 DMX slots.

Each channel configures:

- its head and semantic attribute;
- its primary and optional secondary byte positions;
- raw-DMX default and highlight values at the selected resolution;
- physical minimum, physical maximum, and unit for continuous mappings;
- invert;
- snap, meaning output changes never fade;
- reacts to virtual intensity;
- reacts to sequence, group, and grand masters; and
- one or more prioritized channel functions.

Default, highlight, function ranges, and fixed values are entered as the exact raw DMX integers used by fixture manuals. Continuous functions additionally expose their physical-unit range. Values are stored without losing 16-, 24-, or 32-bit precision.

**Static** is a channel behavior rather than a programmer-controlled attribute. A static channel outputs its default value unless its fixture or head is highlighted, in which case it outputs its highlight value.

The attribute selector uses a canonical registry shared with the programmer. It includes intensity; red, green, blue, cyan, magenta, yellow, amber, white, and UV emitters; color wheels 1 and 2; pan; tilt; the existing position, beam, focus, and control attributes; and custom attributes. The registry supplies stable identifiers, operator labels, attribute families, value types, and default physical units.

### Multi-function channels and fixed values

A physical DMX channel can expose several ordered, non-overlapping functions. A function declares its full-resolution DMX range, semantic attribute, fixed priority, and one of these behaviors:

- continuous mapping with a physical range;
- named fixed value;
- indexed color or gobo slots; or
- a control or macro function.

Only explicitly active programming claims a function. When several programmed attributes target functions on the same physical channel, the configured highest-priority function owns the output. Releasing it reveals the next active function or the channel default. Defaults do not permanently claim a function. New continuous functions default to priority 0, indexed or fixed functions to priority 100, and safety/control functions to priority 200; the fixture author can change the priority.

This mechanism supports combined dimmer/shutter/strobe channels without pretending that all functions can output simultaneously.

Named color and gobo slots use stable cross-fixture semantic identifiers in addition to fixture-local labels and DMX ranges. They always appear in direct programmer pickers. An explicit action can create portable show presets from them; patching a fixture does not automatically add preset objects to the show.

Multi-channel commands use typed control actions rather than unrelated fixed values. Each named action is configured as latched, momentary, or timed pulse and atomically assigns active and inactive values to all participating channels. This supports lamp commands, resets, and mutually exclusive controls such as projector-screen up and down.

### Runtime behavior

The engine applies a channel's default only when no explicit source owns it. Highlight replaces the output with the configured highlight value for the highlighted fixture or head, including static channels. Snap channels bypass programmer, cue, move-in-black, and other output transitions.

Programmer values retain LTP semantics. Function priority is applied only after normal source resolution when several semantic attributes compete for one physical channel.

**Reacts to virtual intensity** multiplies the channel output by the physical or abstract intensity of its head. **Reacts to master** opts non-intensity channels into the same applicable sequence-master, group-master, and grand-master scaling paths as intensity. Master scaling must be applied exactly once; unrelated sequence or group masters do not affect the channel.

## Color systems

The Color tab configures color abstraction per head. The programmer and presets continue to use an abstract XYZ color so different fixture implementations can produce a consistent requested color.

Additive systems bind each emitter to a channel, measured XYZ or xyY output, maximum level, and response curve. Subtractive systems describe typed CMY transforms. Discrete color or gel wheels use indexed slots with stable color identities and optional measured XYZ values.

The mixer uses bounded, non-negative emitter optimization and deterministic gamut clipping. When calibration is unavailable, it falls back to direct RGB or CMY mapping. UV and other non-visible emitters do not participate in normal visible-color matching unless explicitly programmed. Expert direct-emitter control remains available.

The same resolved color drives DMX output, direct programmer controls, presets, and visualization.

## Geometry and beam visualization

The Geometry tab provides templates for fixed fixtures, conventional moving heads, bars and matrices, and shared-pan multi-head fixtures. A template creates an editable hierarchy; it does not restrict later configuration.

The editor contains a parts-and-emitters tree, property forms, and a live 3D preview. Geometry nodes have stable identities, a parent, base transform, pivot, optional GLB-node binding, and optional attribute-driven rotation or translation with an axis and physical range.

Emitter nodes attach to any geometry part and configure:

- logical head;
- origin and orientation;
- beam and field angle;
- feather and focus; and
- point, matrix, ring, strip, or explicit-pixel source layout.

The hierarchy must support a fixed chassis, a pan-driven arm, tilt-driven child heads, and offset beam sources. A shared pan ancestor with multiple tilt children supports fixtures with common pan and independently controlled heads. Multiple emitters may belong to one logical head, allowing fixed multi-emitter lamps, separated RGB sources, ring and strip sources, and conventional lamps with gel scrollers.

The stage visualizer consumes this geometry graph and the same resolved attribute and color values used for output. It must no longer assume one root, one beam, or hard-coded pan and tilt geometry.

## Patch behavior

The patch workflow continues to select a fixture first and a mode second. Single-split fixtures preserve the current placement and command-line behavior.

For a multi-split mode, placement and patch editing show one optional universe/address assignment per split. Patch-table editing, desk **SET** handling, keyboard control, and attached hardware paths address the selected split consistently. Multi-patch instances carry the same per-split assignment structure.

Each split footprint is calculated and validated independently. Overlap checking covers every patched split and multipatch instance. Clearing a split address unpatches only that split; it does not delete the fixture, its heads, or its programming.

## Persistence and migration

Add an explicit schema-v1-to-v2 reader for fixture-library rows and definitions embedded in shows. Do not rely on a schema-version literal change alone.

- A legacy definition converts to a fixture profile with one mode, one default split, and stable head mappings derived from the existing head indices.
- Legacy universe and address fields map to the default split. Legacy multipatch instances migrate to per-split assignments.
- Existing library modes are combined into one profile only when their current manufacturer/model family key and fixture-level metadata agree. Conflicting records remain separate profiles and produce a visible migration warning instead of losing data.
- Retain the original JSON and any retained GDTF source bytes. A failed migration must leave them untouched, keep application startup available, and expose an actionable recovery error.
- Built-in Generic fixtures are regenerated as reserved-source profiles. Catalog upgrades must not delete user-authored fixtures whose manufacturer is also `Generic`.

Successful migration and fresh-library initialization must both be verified through the real server startup path. Existing shows must remain loadable without consulting the current desk fixture library.

## Acceptance coverage

Add focused unit, API, component, Playwright, and desktop coverage for:

- fixture-profile creation, atomic revisions, and revision conflicts;
- create/edit layout parity, title-bar save, edit confirmation, and every dirty-close path;
- manufacturer lookup, search, selection, and shared keyboard input;
- mode, head, and channel add/remove/reorder behavior;
- optional exclusive master heads and one-split-per-head validation;
- the single-open split accordion and the unwrapped one-split table;
- primary-slot derivation, reserved fine/third/fourth-byte positions, raw multi-byte encoding, and validation errors;
- defaults, highlight, static, snap, invert, virtual intensity, and master reactions;
- prioritized multi-function ownership, fixed/indexed values, typed atomic actions, and opt-in preset generation;
- calibrated additive color, CMY fallback, discrete wheels, gamut clipping, and visualization color parity;
- geometry templates, hierarchical transforms, common-pan multi-heads, multiple emitters, and beam characteristics;
- independent split patching, unpatched splits, overlap detection, multipatch, and control-surface parity; and
- representative legacy fixture-library databases and embedded show definitions, including startup recovery on failed migration.

Update the fixture-library and patch help, both testing coverage indexes, and the fixture-library screenshots when implementation changes become operator-visible. Verification should progress through focused unit and UI tests, migration startup tests, relevant API and Playwright scenarios, manual generation, desktop smoke, and the authoritative `./build open` path.
