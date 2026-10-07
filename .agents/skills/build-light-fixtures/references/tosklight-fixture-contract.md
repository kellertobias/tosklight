# ToskLight transferable fixture-package contract

Re-check the implementation because schema and verification seams can evolve.

## Package ownership

All shipped and operator-transferred fixtures use `assets/fixture-library/*.toskfixture`. Fixture definitions must not be added to Rust or TypeScript catalogs. A `.toskfixture` is a ZIP containing one root `fixture.json` plus only the assets referenced by that manifest.

The wrapper is:

```json
{
  "$schema": "https://tosklight.app/schemas/fixture-package-v1.json",
  "format": "tosklight.fixture",
  "format_version": 1,
  "profile": {}
}
```

An optional `projection_assets` set is revision-owned beside `model_asset` and contains exactly
top, left, right, front, and back SVGs at `assets/projections/<view>.svg`. It records the source GLB
SHA-256, generator/version, pose-contract version, physical millimetre `viewBox`, fixture origin,
page orientation, and deterministic pose. SVGs use only opaque closed move/line paths. Scripts,
events, CSS, text/fonts, images, links, external resources, transforms, animation, filters, and
environment-dependent references are invalid. Raster output is derived from this canonical SVG.
Generation writes a separate package and never mutates an installed library revision.

The profile is schema v3 and must have `reserved_source: null` or omit catalog ownership. Schema-v2 profiles migrate to explicit identity mappings when read. Optional assets are relative paths under `assets/`: photograph and stage icon in PNG/JPEG/WebP, a self-contained GLB 2.0 model, and — for a laser only — a UTF-8 JavaScript scan engine at `assets/scan.js`, referenced from `profile.laser.scan_script_asset` and capped at 256 KiB. A fixture with a gobo wheel declares it as `profile.gobos`, one entry per slot with the open slot counted as zero: `{ "wheel": 1, "slot": 3, "name": "Breakup", "artwork_asset": "assets/gobo-3.png" }`. The artwork is a PNG/JPEG/WebP mask where light passes through white, at most 2048 pixels square and 1 MiB, resampled once on the way to a renderer; colour in it is ignored, because a gobo takes the colour of whatever the fixture puts through it. A declared wheel also decides how many slots the gobo channel is divided into, so a fixture with five gobos and an open slot no longer inherits a guess. A slot may name itself and carry no artwork, and a profile that declares no wheel keeps the visualizer's own drawn patterns. Imports preserve the stable profile ID. Changed content for the same manufacturer/name becomes a new local revision; an ID collision with a different family is invalid.

Startup reads the same archives through `FixtureLibrary::load_fixture_package_directory`. Package updates apply only while the last package-installed revision is current. A later operator revision is preserved. Patched shows remain insulated by their embedded profile snapshot.

## Schema relationships

- `FixtureProfile` owns fixture-wide identity, physical facts, optics, assets, modes, and safety policy.
- `FixtureProfile.optics` describes what the fixture's light looks like: relative `output`, `sharpness` and `uniformity` as `0..=1` fractions, and a `light_source` with a `round`/`oval`/`rectangular` form and width and height in millimetres. Every field is optional, and an omitted field is derived from the declared `fixture_type` rather than guessed per fixture. Declare a figure only when it is measured or documented; leaving the block empty is the correct answer for a fixture whose manual says nothing about its optics.
- `FixtureProfile.laser` is present only on a laser and describes what its channels cannot: the scan engine as source text plus the scanner it drives — `scan_angle_degrees` (and an optional `scan_angle_y_degrees`), `points_per_second`, `divergence_milliradians`, `aperture_millimetres`, and `optical_power_milliwatts`. Every field is optional and an omitted one is derived from the declared `fixture_type`, exactly as `optics` works. The script is the exception in kind rather than in optionality: a laser without one is rendered dark and diagnosed, because a laser's output is decided by a pattern bank inside the fixture and inventing one would misrepresent the product. Do not add a `laser` block to a fixture that is not a laser.
- `FixtureProfile.gobos` describes the fixture's gobo wheels, and belongs on any fixture that has one. A gobo channel says which slot is in the beam; nothing in the channel model can say what is etched on the glass, so the wheel is declared here and the artwork travels with the package. Slots need not be contiguous, the open slot need not be declared, and the wheel is as long as its highest slot. Declaring a wheel with no artwork is still worth doing: it divides the channel into the slots the fixture actually has. Each entry has a one-based `wheel` (1 through 8), corresponding to `gobo.N`; omission means wheel 1 for existing packages. Slots are 0 through 63 and unique within a wheel, so two wheels may use the same slot number. Package export keeps first-wheel artwork under `assets/gobo-<slot>.*` and uses `assets/gobo-wheel-<wheel>-<slot>.*` for later wheels.
- `FixtureProfile.prisms` declares visual representations selected by `prism.N`: `{ "wheel": 2, "slot": 1, "representation": "linear", "facets": 5, "spread_degrees": 8 }`. The one-based wheel is 1 through 8 and defaults to 1. Slots 1 through 63 are unique within a wheel; zero is the open beam and must not carry a prism. `representation` is `radial` or `linear`, `facets` is the number of beam copies (2 through 16), and finite `spread_degrees` is the angle from the beam axis to the outermost copy (0 through 45). Rotating a prism rotates this arrangement. Omission or an empty list preserves the generic legacy prism appearance. This is a visual approximation, not a lens prescription.
- `FixtureMode` owns independent splits, logical heads, ordered physical channels, color systems, control actions, and geometry.
- `FixtureSplit.number` is an independently patchable address block.
- `FixtureChannel.head_id` selects its logical head and `FixtureChannel.split` selects its patch block. One head may own channels in several splits. Row order derives primary slots per split; `secondary_slots` reserves fine and higher bytes.
- U8 has zero secondary slots, U16 one, U24 two, and U32 three.
- A physical channel retains a fixture-facing attribute, maps it to one canonical semantic attribute with an explicit normalized transform, and has non-overlapping channel functions. Function arbitration uses configured priority.
- Multi-cell emitters need separate logical heads when independently programmable. Fixture-wide dimmer, shutter, macro, and movement controls stay on the master/shared head.
- A mode geometry graph may be empty when the packaged GLB or a broad device-type fallback supplies the Stage representation.
- `HeadColorSystem` drives Color Intent, which shows one device-independent colour on every fixture. A head may carry several systems (a CMY engine and a colour wheel); the continuous engine is used first and a steady wheel slot only when it cannot come within Δu′v′ 0.02. Each system carries `calibration: { status, revision, source }`: `measured` only for colorimeter or spectrometer data, `nominal` for datasheet or typical values (the reading of a profile that omits the block), `uncalibrated` when the data cannot promise a colour. Increment `revision` whenever the system's colour data changes. Additive emitters carry their full-drive `xyz`, `maximum_level` and `response_curve`. A `subtractive` system may carry `filters: { open_xyz, cyan_xyz, magenta_xyz, yellow_xyz }`, the measured beam open and with each flag fully in; omit it rather than invent it. A `discrete_wheel` slot carries `measured_xyz` when measured and `steady: false` for a split, scroll, rotation or effect position (unset reads its name: split, half, scroll, rotation, rainbow, effect and similar are not steady). Heads without authored systems still resolve from their channel attributes and are reported uncalibrated, so author a system only from real data.

## Intent-programming physical foundation

The optional `ChannelFunction.physical_mapping` and `FixtureMode.color_physical` blocks are
additive authoring data. During the foundation increment they do not replace the current live
mapping or `HeadColorSystem` resolver. Preserve existing supported runtime metadata as well.
Do not claim a fixture matches physically just because these fields validate.

- Physical mapping retains directed endpoints and optional monotone raw/physical samples at the
  channel's full 8/16/24/32-bit width. Keep exact byte order, function domain, unit and provenance.
  Zoom values are full opening degrees with an explicit beam/field convention. Focus remains
  normalized 0–100%; do not fabricate distance from a focus channel.
- A version-1 `color_physical` model has one ordered path per modeled logical head. Each path owns
  the complete native Color channel set, including parked and currently unmodeled mechanisms.
  Bind emitters and filters to stable channel and function IDs, never display names or raw slot
  indices. Missing or foreign references are invalid. Shared controls must belong to an explicitly
  shared head; this does not establish independent solvability for every dependent head.
- Sources are unknown, fixed or additive. Additive emitters retain full-drive XYZ and/or source
  spectrum, visible/UV/IR band, native direction, maximum drive, response and provenance.
  A label such as RGBW is a topology hint, not measured emitter chromaticity.
- Filters are in physical beam order. Store transmission spectra only when known, over explicit
  disjoint native intervals. Source spectra are linear relative spectral power per nanometre on
  a common path scale; filter spectra are dimensionless transmission in 0–1. Retain wavelength
  coverage and gaps. Do not multiply isolated XYZ wheel-slot or CMY colors to predict stacked
  filters. Missing source spectra or filter transmission remains unknown.
- Whole-path measurements identify every owned native control exactly once, including open or
  parked channels. They describe that complete recipe only. A zero XYZ measurement is black,
  not a missing value. Service/reset functions cannot be native Color recipes.
- Quality is explicit: unknown, estimated, manufacturer or measured. Manufacturer and measured
  entries require evidence. Imported nominal colors do not become measurements. Retain imported
  spectra and recipe measurements even when the simple editor does not author individual samples.
- Stable path/emitter/filter IDs join the existing identity rules. The native layout signature
  distinguishes recipe compatibility from the full appearance/profile digest; do not recreate
  IDs during calibration updates or move old recipes onto different channel functions.
- Installed pan/tilt `position_calibration` belongs to the show patch root or its specific physical
  multi-patch copy, never to the transferable library fixture. It stores unwrapped zero offsets,
  quality, source and revision, separately from mounting pose and Pan/Tilt inversion.

Normal semantic Color presets/cues will store the requested intent. Fixture metadata must enable
the destination resolver to write every destination Color channel, including additional White,
CMY or wheel channels after a fixture replacement. Explicit Direct recipes retain their separate
compatibility and best-effort portable behavior; they are not the default recording format.

## Identity rules

- Generate UUIDs once for the profile, modes, heads, channels, functions, and geometry parts and retain them. UUID v4 is acceptable. Splits use their positive `number` and have no UUID.
- Do not regenerate identity from display wording, incidental row numbers, package revision, or archive filename.
- Keep existing UUIDs when correcting the same semantic object. New physical products or genuinely different semantic objects receive new UUIDs.
- Never use manufacturer text or fixture name as ownership. Never set historical `builtin:*` reserved-source markers.

## Manual transcription checklist

For every mode, capture exact mode name and footprint; every slot; coarse/fine grouping and byte order; functions and wheels; defaults, safe and Highlight values; physical ranges and units; emitters and color semantics; head ownership; dimensions, weight and power; safety policy; geometry pivots, emitters and beam layout; and source URL/manual revision. Record manual title, revision, firmware applicability, and URLs in `profile.notes` until structured provenance exists.

Represent documented unused slots as static channels. Mark unknown facts unknown. If only a third-party source exists, identify that limitation.

## Package and runtime verification

- Validate every archive with `cargo run -p light-fixture --bin fixture-package -- validate assets/fixture-library/*.toskfixture`; write/export round trips must retain normalized content and stable IDs.
- Assert exact profile/mode inventory, slot coverage, resolution bytes, logical heads, safe/Highlight values, and GLB/icon presence where required.
- Start `light-headless` with `--fixture-package-dir "$PWD/assets/fixture-library"` and verify `/api/v2/fixture-library/profiles` plus the typed actions at `/api/v2/fixture-library`.
- Start twice against the same temporary data directory to prove idempotence.
- Verify a later operator revision is not overwritten by a changed startup package.

A packaged scan script must be an ES module exporting `scan(input)` and returning control points with `x`/`y` deflections in `-1..=1`, `r`/`g`/`b` in `0..=1`, and an `amount` percentage of the scan. Verify it by loading the package and rendering rather than by reading it: `docs/help/45-Visualizer/05-lasers.md` is the operator-facing contract, and the engine's own rules — the ILDA colour-arrives-at-the-point convention, dwell as brightness, the sandbox and the per-frame time budget — are in `crates/viz/laser`.

GLB is optional unless exact manufacturer appearance is requested or the broad device-type fallback is inadequate. When supplied, verify useful non-collapsed bounds, intended node bindings, pivots, emitter ownership, and finite non-zero scales for visible parts.


## Explicit physical geometry and Position calibration

For the intention-programming contract, use optional `GeometryGraph.physical_contract` version 1
and `FixtureMode.position_physical` version 1. Their presence is explicit authoring, not an automatic
claim that existing artwork is measured. Geometry is local mm, right-handed Y-up, beam −Y and
neutral `Rx*Ry*Rz`; motion composes a local axis-angle rotation after the neutral rotation.
The optionality belongs to the whole contract; inside it bracket state is explicitly unknown,
fixed, or a parent-frame hinge attached to an existing body node. Physical ancestry is rigid.
See the equations and TL-555 section in `docs/plans/fixture-independent-programming.md`.

Bind Position by exact node/channel/function IDs, explicit Pan/Tilt role and the function's
absolute-degree or degree/second motion metadata. Preserve multi-turn endpoints and fine ordering.
Shared cross-head motors require a shared channel head. Do not infer exact physical bindings from
attribute names or silently lift/rekey a graph after its physical contract has been authored.

Installed axis overrides and Color observations belong to each show-patch physical instance.
They are not profile fields and are not inherited by newly duplicated hardware. Axis overrides
replace the complete zero/inversion pair and retain a source identity for stale-data detection.
Color observations retain their full immutable optical/native identity and exact complete recipe.
Changing profiles retains stale observations as inactive; do not relabel them as measurements of
the replacement lamp. Live integration and physical acceptance remain separate from metadata validation.
