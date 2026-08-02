# Show Patch Table, Fixture Encoders, and Light-Source Appearance

## Status

**Doing.** Claimed on 2026-08-02 after completing Plan 02. This is the sole active implementation
contract for the Show Patch table consolidation, patch-specific encoder groups, and installed
light-source/gel visualization contract.

This is the third item in the current [Next plan order](README.md), after Group/Dynamics spatial
mapping and the Fixture Sheet improvements. Add the normal `## Result` and verification evidence
before moving it to `Done` in a semantic commit.

## Progress

- [x] Claimed from `docs/plans/Next` in numeric order.
- [x] Audit the current table, patch selection, encoder, persistence, API, catalog, and visualizer
  ownership boundaries plus compatibility and acceptance seams.
- [ ] Implement the exact shared sixteen-column table and physical-instance editing geometry.
- [ ] Implement portable installed appearance, typed sparse mutations, compatibility, and catalog
  import infrastructure.
- [ ] Implement Location/Visualization encoders and shared Stage/Viz appearance evaluation.
- [ ] Update help, screenshots, human scenarios, and focused automated acceptance coverage.
- [ ] Run focused checks, required major suites, migration/recovery proof, and real desktop/Viz
  verification.

## Implementation decisions

- The plan's required focused operator review of the final generic gel names, numbers, display RGB,
  and visualizer RGB remains open. Work may advance on independent table, model, API, encoder,
  import, and renderer contracts, but the shipped catalog entries will not be implemented or
  represented as accepted until that review occurs.
- The current table has 21 columns rather than the 19 stated in the problem description; the exact
  required 16-column target remains unambiguous and is the implementation authority.
- Preserve the existing flat `bracket_angle` as the canonical installed bracket value and interpret
  the existing optional `shaper_angle` as the installed shaper-module rotation. Add the source,
  explicit CCT, gel assignment, and four static shaper-element angles alongside them with compatible
  defaults instead of introducing a duplicate nested authority for already-valid angle fields.
- Introduce one exact patch-local physical instance for encoder/edit targeting. Keep any ordered
  multi-row range selection as a separate table-operation concept; a removed multi-patch clears the
  exact target and never silently retargets its parent.
- Extend the authoritative Patch projection and shared renderer inputs. Catalog lookup and CCT/gel
  evaluation must not become browser-local or Viz-only authority, and live shaper attributes win per
  supported physical component rather than being added to a competing static value.
- Repair modern reference-only MVR merge retention in this plan: current merge code attempts a
  legacy inline-fixture decode and can silently default existing policies and angles before the new
  appearance fields are considered.

## Implementation verification

- The first table slice replaces the current 21-header model with the exact 16 headers, stacks
  product/mode plus manufacturer, Masters, and Pan/Tilt in shared cells, combines MIB display, adds
  the Light source default presentation, and removes the retired Bracket/Shaper table columns.
  Primary and multi-patch rows now use the same 16-cell grid. Multi-patch Fixture ID and Name are
  literal `—`, its stored name remains in the row's accessible identity, its actual split patch is
  in the Patch cell, and shared logical values remain visibly identified. Desktop typecheck and all
  35 focused Show Patch control tests pass, including an exact header/cell-count regression; the
  production desktop frontend build also passes.
- Patch now owns a separate exact physical-instance selection keyed by fixture ID plus optional
  multi-patch instance ID. Primary and multi-patch row clicks set that identity independently of
  logical Programmer selection; the patch parameter surface resolves only that exact instance.
  Authoritative fixture reconciliation retains existing roots/copies, clears removed fixtures or
  copies, and never falls back from a stale copy to its parent. Desktop typecheck and 44 focused
  tests pass across the Show Patch table, exact root/copy mutations, loading/stale behavior, and
  selection reconciliation.
- The stacked policy cells now have one SET target each. Masters exposes only the applicable
  choices from Not controlled, Group Master, Grand Master, and Both; Pan/Tilt exposes only the
  applicable choices from None, Invert Pan, Invert Tilt, and Invert Both for the addressed root or
  multi-patch. Each Apply writes both underlying flags in one patch mutation while preserving
  dormant inapplicable values. MIB now accepts one value, `Off` or non-negative decimal seconds;
  `0` remains enabled at zero delay, fractional values use millisecond rounding, and invalid,
  non-finite, non-decimal, or unsafe-overflow input is rejected visibly. Desktop typecheck and 50
  focused table/editor/parser tests pass.

## Current behavior and problem

The current Show Patch table has nineteen columns. Manufacturer, Product / mode, Group Masters,
Grand Master, Invert Pan, Invert Tilt, MIB, MIB Delay, and all six transform axes are separate
columns. This makes each row excessively wide and makes related two-state settings look unrelated.

Multi-patch rows currently place shared master information into columns that visually belong to
fixture identity, use a wide multi-patch name cell, and can make the patch address appear displaced
from the **Patch** column. The table must instead keep every row on the same column grid.

The current patch-specific encoder surface exposes the six position/rotation values in one unnamed
family. It does not offer the required installed-fixture visualization controls. Light source and
color temperature currently exist only as fixture-profile physical defaults, while a show-patched
physical lamp or multi-patch instance cannot record its installed lamp source or gel.

## Goals

1. Reduce horizontal table width by stacking related values into literal two-row cells.
2. Keep primary and multi-patch rows aligned under exactly the same column headers.
3. Replace paired Boolean editors with one explicit four-state choice where applicable.
4. Replace separate MIB enabled/delay editing with one operator value: **Off** or a non-negative
   delay.
5. Give the selected physical patch instance two exact six-encoder groups: **Location** and
   **Visualization**.
6. Store installed light-source, color-temperature, and gel appearance per physical instance and
   use it consistently in every visualizer.

## Required table columns

The fixture table has these columns in this exact order:

1. **Type**
2. **Fixture ID**
3. **Name**
4. **Fixture / mode**
5. **Patch**
6. **Masters**
7. **Pan / Tilt**
8. **MIB**
9. **Light source**
10. **Location X**
11. **Location Y**
12. **Location Z**
13. **Rotation X**
14. **Rotation Y**
15. **Rotation Z**
16. **Layer**

Do not retain separate Manufacturer, Product / mode, Group Masters, Grand Master, Invert Pan,
Invert Tilt, or MIB Delay columns. The six transform columns remain in the table; the encoder
groups are an additional editing surface, not a reason to hide the table values.

All headers and data cells stay on one fixed column model. Do not use row-specific `colSpan`, wide
identity cells, or content-driven column insertion for multi-patches. Horizontal scrolling may
remain when the available viewport is narrower than the table, but the consolidated table should
not recreate the removed width through oversized minimum widths.

## Stacked cell contracts

### Fixture / mode

Manufacturer, product, and mode occupy one column and exactly two visible text rows:

- first row: `<product/model name> · <mode name>`;
- second row: `<manufacturer>`.

The first row is the SET target for changing the fixture mode through the established compatible
manufacturer/product family workflow. The second row is read-only profile identity. Long values
truncate within this one column with full text available to accessible name/title; they do not
widen a multi-patch row or create a third visible row.

The separate **Name** column remains the operator's show-specific fixture name. It is not combined
with manufacturer/product identity.

### Masters

Group Masters and Grand Master occupy one column and two rows:

- first row: **Group Masters** with its effective **Controlled** or **Not controlled** state;
- second row: **Grand Master** with its effective **Controlled** or **Not controlled** state.

SET on the cell opens one four-choice editor:

1. **Not controlled** — neither Group Masters nor Grand Master applies;
2. **Group Master** — Group Masters apply, Grand Master does not;
3. **Grand Master** — Grand Master applies, Group Masters do not; and
4. **Both** — both master families apply.

The editor maps atomically to the existing independent `group_masters_enabled` and
`grand_master_enabled` policies. It does not change profile-level master eligibility. A fixture
without eligible intensity for one family shows that line unavailable and cannot be offered an
impossible combined choice. The warning that a fixture may remain live while a master is reduced
remains visible for every choice that disables an applicable master.

Master participation belongs to the logical fixture. A multi-patch row shows both shared states in
the one **Masters** cell with a clear shared indicator; it does not place those values in Fixture ID
or Name and cannot edit them as instance-local data.

### Pan / Tilt

Invert Pan and Invert Tilt occupy one column and two rows:

- first row: **Invert Pan** with **Normal** or **Inverted**;
- second row: **Invert Tilt** with **Normal** or **Inverted**.

SET on the cell opens one four-choice editor:

1. **None**;
2. **Invert Pan**;
3. **Invert Tilt**; and
4. **Invert Both**.

The choice atomically maps to the two existing flags. Inapplicable axes remain visibly unavailable
and are never mutated merely because the other axis changes. The primary row edits its physical
instance; each multi-patch row edits that multi-patch's independent physical Pan/Tilt inversion.
The completed Plan 22 output order, raw profile inversion composition, logical programming, and
master-participation ownership remain unchanged.

### MIB

Move in Black and MIB Delay occupy one **MIB** column. The cell and editor expose one value:

- **Off** means Move in Black disabled; or
- a non-negative delay in seconds means Move in Black enabled with that delay.

`0 s` is valid and explicitly means **On with zero delay**; it is distinct from **Off**. Positive
decimal seconds are accepted and converted to integer milliseconds using the existing rounding
rule. Do not add an arbitrary product maximum: accept any finite non-negative value representable
by the persisted millisecond field, and reject overflow or non-finite input visibly.

The editor atomically sets `move_in_black_enabled` plus `move_in_black_delay_millis`. The table
shows **Off** or the formatted delay such as `0 s`, `0.5 s`, or `12 s`; it never needs a second MIB
column. MIB remains logical-fixture behavior shared by every physical multi-patch output, so a
multi-patch row shows the shared value rather than an independent editor.

## Multi-patch row geometry

A multi-patch row uses the same sixteen table cells as its parent row.

- **Type** contains only the compact branch/tree marker.
- **Fixture ID** contains exactly an em dash (`—`).
- **Name** contains exactly an em dash (`—`).
- Those two dash cells use the normal column widths and padding. They must not be wider, merged, or
  replaced by the current wide multi-patch name block.
- The stored multi-patch name remains available for accessible labels, edit-dialog identity, API
  diagnostics, and future paperwork, but it does not occupy the visible Name column in this table.
- **Fixture / mode** shows the same two-row product/mode and manufacturer identity as the parent,
  with a subdued shared treatment.
- **Patch** contains that multi-patch instance's actual split/single patch position and edit target.
  It must never appear under Masters, identity, or an empty spacer column.
- **Masters** and **MIB** show their two-row/shared logical-fixture values.
- **Pan / Tilt**, transforms, and **Light source** show/edit the selected physical multi-patch
  instance's values.
- **Layer** shows the effective parent layer as shared unless a later plan introduces per-instance
  layer ownership.

Unpatched instances show the established dash/unpatched representation inside **Patch**. Split
patches remain grouped inside that same cell. Multi-patch creation, removal, selection, and address
conflict validation retain their existing behavior.

## Patch-local physical selection

The Show Patch surface needs an explicit selected physical row:

```rust
struct SelectedPatchInstance {
    fixture_id: FixtureId,
    multipatch_instance_id: Option<Uuid>,
}
```

Clicking a primary fixture row selects the primary physical instance. Clicking a multi-patch row
selects that exact multi-patch instance for patch encoders and physical-instance cell editing.
Ordinary fixture programming selection may still resolve to the shared logical fixture/head set,
but it must not erase or ambiguously replace the patch-local physical identity.

The current `selectedFixture`-only editor state is insufficient for instance-local Location,
Visualization, Pan/Tilt, and Light source edits. Every such mutation must carry both fixture and
optional multi-patch identity. A stale or removed selected instance disables the encoder surface
and clears/reconciles the patch-local selection without retargeting the parent silently.

## Patch encoder groups

When Show Patch is the active built-in and one physical row is selected, replace the current
unnamed **Fixture position** surface with two explicit encoder-group tabs. Use the normal shared
six-slot encoder surface and preserve software/hardware interaction parity, fine/coarse behavior,
Set Value, touch sizing, numbered slots, and disabled-slot geometry.

### Location

**Location** contains exactly:

| Slot | Encoder | Unit |
| ---: | --- | --- |
| 1 | Location X | m |
| 2 | Location Y | m |
| 3 | Location Z | m |
| 4 | Rotation X | ° |
| 5 | Rotation Y | ° |
| 6 | Rotation Z | ° |

Locations persist as integer millimetres and display/edit metres. Rotations persist/display in
degrees. Relative turns change only the addressed axis; Set Value opens exact absolute entry.
Primary and multi-patch instances mutate their own existing transform fields through the same
revisioned patch service.

### Visualization

**Visualization** contains exactly:

| Slot | Encoder | Unit |
| ---: | --- | --- |
| 1 | Bracket | ° |
| 2 | Shaper 1 Angle | ° |
| 3 | Shaper 2 Angle | ° |
| 4 | Shaper 3 Angle | ° |
| 5 | Shaper 4 Angle | ° |
| 6 | Shaper Module Rotation | ° |

These are installed physical visualization values, not Programmer attributes or DMX output. Store
finite degrees normalized to `[-180, 180)`, display whole degrees by default, use 1° fine steps and
10° coarse steps, and permit exact decimal Set Value entry.

**Bracket** is the installed mounting/yoke bracket angle used by compatible fixture geometry.
Shaper angles describe the four installed physical shutter/barn-door elements and **Shaper Module
Rotation** rotates their shared module. Profiles/models expose which semantic visualization roles
they support. Unsupported encoders remain present and disabled; a missing role is not synthesized
by mutating arbitrary GLB nodes.

If the same physical shaper is controlled by a live DMX attribute, the live resolved attribute is
authoritative in visualization and the corresponding static patch encoder is unavailable. The
patch values are for installed/static geometry and must not compete with or overwrite
`shaper.blade.*.angle`, `shaper.rotation`, Presets, Cues, or Programmer data.

## Installed light-source appearance

### Per-instance model

Add portable show-patch visualization data to the primary physical fixture and every multi-patch
instance:

```rust
struct InstalledFixtureAppearance {
    light_source: InstalledLightSource,
    color_temperature_kelvin: Option<u32>,
    gel: GelAssignment,
    bracket_degrees: f32,
    shaper_angles_degrees: [f32; 4],
    shaper_module_rotation_degrees: f32,
}

enum InstalledLightSource {
    ProfileDefault,
    Tungsten,
    Halogen,
    Discharge,
    Led,
    Fluorescent,
    Arc,
    Other { label: String },
}

enum GelAssignment {
    OpenWhite,
    BuiltIn {
        id: String,
        embedded_fallback: GelDefinitionSnapshot,
    },
    Custom {
        name: String,
        color_srgb: String,
        note: Option<String>,
    },
}
```

Naming may follow repository conventions, but the semantics above are required. A built-in gel
snapshot contains stable identity, catalog number, display name, display sRGB color, and a separate
visualizer sRGB color so a show remains portable if another desk lacks that catalog revision. The
display color is optimized for recognizing the entry in the operator UI; the visualizer color is
the approximation applied to emitted light and may deliberately differ. The first version does not
require spectral curves or measured transmission.

### Gel catalogs and CSV import

Do not bundle Rosco, LEE, or another manufacturer's gel catalog without explicit redistribution
rights. Ship one small manufacturer-neutral catalog of approximately sixteen plainly named colors,
including **Open white** and common choices such as red, green, blue, amber, cyan, magenta, and
violet. The final generic names, numbers, and two RGB values receive one focused operator review
before implementation; they must not imitate or imply a third-party catalog.

Operators can import additional catalogs from CSV. The required columns are exactly:

```text
number,name,display_rgb,visualizer_rgb
```

- `number` is the catalog-local operator identifier and is preserved as text.
- `name` is the operator-facing color name.
- `display_rgb` and `visualizer_rgb` are canonical `#RRGGBB` sRGB values.
- The import action asks for a catalog name; the four-column CSV does not need to repeat it per row.
- Empty required fields, invalid colors, duplicate numbers within one catalog, duplicate catalog
  identities, malformed quoting, or unreadable encoding reject the import with row-specific errors
  and no partial catalog mutation.
- Import is an explicit preview-and-confirm transaction showing additions, replacements, conflicts,
  and invalid rows. It must not silently reinterpret a changed CSV on application startup.

Imported catalogs are installation-owned library data, not automatically embedded wholesale in a
show. Assigning one entry embeds its number, name, display color, and visualizer color as the
portable fallback snapshot. Another desk therefore renders the assigned appearance even without
the source catalog and may later reconcile it by stable catalog/entry identity.

Profile defaults remain immutable fixture-library metadata. **Profile default** resolves the
profile's `physical.light_source` and `physical.color_temperature_kelvin`. An instance override
never edits the embedded profile snapshot or library. New primary fixtures default to Profile
default plus Open white. Creating a multi-patch copies the primary instance's current appearance;
the copy is independent afterward so physical copies may use different lamps, CCTs, gels,
brackets, or shapers.

### Light source column and editor

The **Light source** cell uses two compact rows:

- first row: selected source plus effective color temperature, for example
  `Tungsten · 3,200 K` or `Profile default · 6,500 K`;
- second row: gel/filter name, or **Open white**.

SET on the cell opens one editor for the selected physical instance. It contains:

- light-source selection, including Profile default and the typed/common source choices;
- an editable color temperature in kelvin;
- gel choice: Open white, a built-in gel/filter, or Custom color;
- built-in/imported catalog search with catalog, number, name, and display color;
- a CSV catalog import action with preview, validation, and visible errors; and
- custom gel name, color picker/value, and optional note.

Color temperature accepts whole kelvin values from 1,000 K through 25,000 K. Profile default may
show an unknown CCT; choosing an explicit source without a profile/default temperature requires an
explicit CCT before Apply. Gel selection is available for conventional and color-capable fixtures
because it represents a physical filter on the installed output. Fixtures with no light-emitting
geometry show the cell unavailable rather than storing misleading source data.

The editor is one revisioned patch-object intent. Apply is enabled only when the normalized draft
differs; Close discards the draft. Changing light source does not silently clear an explicitly
selected gel, and choosing Open white clears only the gel assignment.

## Visualizer evaluation

Built-in Stage, the standalone Viz renderer/editor, help previews, and future paperwork consumers
must read the same installed appearance from the API scene/configuration projection. Do not create
a browser-local gel table or a second Viz-only fixture assignment.

For rendering:

1. resolve profile or explicit source and effective CCT;
2. convert CCT to a deterministic linear-RGB white-point approximation;
3. resolve live emitted fixture color, or white for a conventional intensity-only lamp;
4. multiply by the source/CCT tint and gel visualizer color in linear space;
5. apply resolved intensity and the existing beam/material pipeline; and
6. apply bracket/shaper physical geometry to supported semantic nodes.

This appearance affects fixture emissive surfaces, beams, and visible spill consistently. It does
not alter Art-Net/sACN/DMX output, Programmer values, fixture color calibration, Group activation,
or Cue storage. The initial sRGB gel approximation does not claim spectral accuracy or lumen loss;
future spectral/transmission data can refine rendering without changing assignment identity.

The API scene/configuration carries appearance changes and authoritative revisions. Live values
remain sourced only from the active real output provider under the standalone Viz contract.

## Persistence and backward compatibility

This plan does not declare a pre-v1 compatibility break. Follow `docs/acceptance-criteria.md`.

- Existing shows keep the current four master/inversion Booleans and MIB enabled/delay fields;
  table editors project them into combined choices without requiring a destructive schema merge.
- Existing fixtures without installed appearance decode as Profile default, profile CCT when
  present, Open white, and zero bracket/shaper angles.
- Existing multi-patches receive the same defaults without changing patch addresses, transform,
  inversion, or logical membership.
- A migration may materialize appearance fields only when the show is next saved; loading must not
  rewrite the original merely to display defaults.
- Copy, partial-show import, full-show import, duplicate, MVR merge, demo generation, and fixture
  replacement preserve/remap the appearance with the same physical instance as transforms and
  inversion.
- Changing fixture profile/mode retains installed overrides. Profile default resolves against the
  newly embedded revision; explicit source/CCT/gel values remain explicit.
- Unknown source/gel IDs retain embedded display/color data and show an actionable unavailable
  catalog state instead of being deleted or remapped.
- Malformed appearance data preserves the original show and follows startup recovery; invalid one-
  object edits are rejected without changing the last valid patch revision.

Representative legacy fixtures, multi-patches, profile-default CCT values, visual-only objects, and
old inline profile snapshots need migration regression coverage through real server startup.

## API, event, and mutation ownership

Follow `docs/engineering/api-rules.md` and retain the existing authoritative Patch application
boundary:

- table and encoder edits are typed sparse object intents, never whole-patch resubmission;
- every physical-instance edit includes fixture ID, optional multi-patch instance ID, expected
  fixture/patch/show revision, and request ID;
- combined Masters, Pan/Tilt, and MIB editors commit their underlying paired fields atomically;
- transform, visualization, and light-source changes update only the addressed physical instance;
- tolerate and log unknown fields without logging values;
- stale/replayed requests reconcile through the existing idempotent outcome path;
- one accepted edit creates one patch revision/audit entry and one targeted patch event; and
- Show Patch edits remain outside Programmer Undo.

The frontend may preview a light color or shaper pose, but persisted state and every renderer
reconcile from the authoritative patch projection. Generated Rust/TypeScript declarations and JSON
schemas are regenerated from source rather than edited by hand.

## Validation and edge cases

- Empty/unpatched fixtures retain table policy, transforms, visualization, and light-source data;
  only physical DMX output is suppressed.
- A fixture without eligible master/axis capability shows the corresponding stacked line
  unavailable and preserves dormant compatible data through mode changes.
- `MIB Off` and `MIB 0 s` remain distinct through UI, wire, persistence, engine, and reopen.
- MIB numeric entry rejects negative, non-finite, and millisecond-overflow values atomically.
- Multi-patch Fixture ID and Name cells remain exactly `—` at narrow and wide viewport sizes.
- Split-patch controls and address-conflict checking remain wholly inside **Patch**.
- Selecting a multi-patch targets its physical encoder values without duplicating the logical
  fixture in Programmer selection.
- Removing a selected multi-patch clears/reconciles the physical selection and never edits the
  parent by accident.
- Source labels are trimmed, bounded, and non-empty for Other/Custom values.
- CCT must be an integer within 1,000–25,000 K when explicit.
- Gel display and visualizer colors use validated canonical sRGB hex values; unknown built-ins and
  imported entries retain both fallback values.
- Open white plus a warm CCT remains warm; a red gel on a conventional lamp renders red as intensity
  rises and returns dark at zero.
- Different multi-patch instances of one logical fixture may show different gels and transforms
  while receiving the same live intensity/programming value.
- Live DMX shaper attributes win over static appearance for the same physical component.
- Visual-only scenery without an emitter cannot acquire a misleading light source.

## Verification plan

### Focused model and application tests

- combined Masters four-state mapping, capability restrictions, warnings, and atomic mutation;
- combined Pan/Tilt four-state mapping for primary and multi-patch instances;
- MIB Off versus 0/decimal/large delay parsing and overflow rejection;
- patch-local primary/multi-patch selection and stale-instance behavior;
- Location and Visualization encoder ordering, units, fine/coarse steps, Set Value, disabled slots,
  and software/hardware parity;
- appearance defaults, independent multi-patch copies, profile/mode replacement, and import/copy;
- generic/imported/custom/open-white gel round-trip, CSV import rejection/atomicity, and unknown
  catalog fallback; and
- deterministic CCT/gel color calculation plus live-shaper precedence.

### Table and visual tests

- exact sixteen headers and absence of the retired separate headers;
- two-row Fixture/mode, Masters, Pan/Tilt, MIB, and Light source content;
- multi-patch dash-only Fixture ID/Name cells, fixed alignment, and Patch-column address;
- narrow/wide viewport screenshots with realistic ACL multi-patches and long manufacturer/mode
  labels;
- accessible names for truncated values, shared values, and unavailable states; and
- built-in Stage plus standalone Viz rendering of warm/cool sources, Open white, red gel, custom
  gel, different multi-patch gels, bracket, and four shaper/module angles.

### Persistence/API acceptance

- representative legacy show startup and migration defaults;
- sparse typed mutation, request replay, stale revision, show guard, tolerant unknown fields, and
  targeted event projection;
- partial/full import and MVR merge retain appearance with the physical instance;
- malformed data preserves original files and exposes recovery; and
- new-show initialization plus save/reopen preserve every field.

Run focused checks first, then proportionate repository gates:

```sh
npm run test:unit
npm run test:e2e-api
npm run test:e2e -- tests/<focused-show-patch-layout-spec>.spec.ts
npm run test:e2e-ui
npm run manual
npm run open
curl -fsS http://127.0.0.1:5000/api/v2/readiness
```

Inspect `.artifacts/runtime/light-data/light-headless.log` first for runtime failures. Storybook and
frontend builds are visual/design evidence, not proof of persistence, authoritative patch mutation,
DMX non-interference, or renderer behavior.

## Acceptance criteria

- [ ] The Show Patch table has the exact sixteen-column model and no retired separate columns.
- [ ] Fixture/model plus mode is row one and manufacturer row two in one Fixture / mode column.
- [ ] Group Masters and Grand Master share one two-row Masters cell and one four-state editor.
- [ ] Invert Pan and Invert Tilt share one two-row Pan / Tilt cell and one None/Pan/Tilt/Both editor.
- [ ] MIB is one column/editor where Off differs from every non-negative delay, including 0 s.
- [ ] Multi-patch Fixture ID and Name cells contain only `—`, keep normal widths, and never merge.
- [ ] Every multi-patch address/position appears in the Patch column.
- [ ] Selecting a primary or multi-patch row establishes an exact patch-local physical instance.
- [ ] Location encoders are exactly X/Y/Z and Rotation X/Y/Z in slots 1–6.
- [ ] Visualization encoders are exactly Bracket, Shaper 1–4 Angle, and Shaper Module Rotation in
      slots 1–6.
- [ ] Unsupported/static-versus-live visualization controls are explicit and never mutate DMX
      shaper attributes.
- [ ] Light source is one table column/editor covering source, explicit/profile CCT, and
      Open-white/built-in/custom gel selection.
- [ ] The shipped catalog is manufacturer-neutral; CSV import accepts exactly number, name,
      display RGB, and visualizer RGB with atomic preview/validation.
- [ ] Installed appearance is portable per physical instance; multi-patches may differ.
- [ ] Built-in and standalone visualizers render the same authoritative CCT/gel and physical
      bracket/shaper appearance without changing DMX or Programmer values.
- [ ] Legacy shows, imports, profile/mode changes, malformed recovery, and unknown gels meet the
      compatibility contract.
- [ ] Exact table geometry, software/hardware encoders, API, persistence, and real visualizer
      behavior have focused acceptance evidence before completion.

## Explicit non-goals

- Changing the established XYZ coordinate convention or table transform ownership.
- Making master participation, MIB, or Group Master runtime per multi-patch instance.
- Changing Plan 22 master/inversion output semantics or HTP arbitration.
- Turning bracket/shaper visualization fields into Programmer attributes or DMX channels.
- Overriding live controllable shapers with static patch metadata.
- Editing shared fixture-profile physical defaults from Show Patch.
- Spectrally accurate gel curves, measured transmission/lumen loss, inventory, fixture schedules,
  or gel paperwork in this first implementation.
- A second Viz-only scene, gel catalog, or local renderer authority.
- Per-instance layers, mounting/parent hierarchy, cable planning, or rigging calculations.
- Removing the six transform columns from the Show Patch table.
