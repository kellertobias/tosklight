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

The profile is schema v3 and must have `reserved_source: null` or omit catalog ownership. Schema-v2 profiles migrate to explicit identity mappings when read. Optional assets are relative paths under `assets/`: photograph and stage icon in PNG/JPEG/WebP, a self-contained GLB 2.0 model, and — for a laser only — a UTF-8 JavaScript scan engine at `assets/scan.js`, referenced from `profile.laser.scan_script_asset` and capped at 256 KiB. A fixture with a gobo wheel declares it as `profile.gobos`, one entry per slot with the open slot counted as zero: `{ "slot": 3, "name": "Breakup", "artwork_asset": "assets/gobo-3.png" }`. The artwork is a PNG/JPEG/WebP mask where light passes through white, at most 2048 pixels square and 1 MiB, resampled once on the way to a renderer; colour in it is ignored, because a gobo takes the colour of whatever the fixture puts through it. A declared wheel also decides how many slots the gobo channel is divided into, so a fixture with five gobos and an open slot no longer inherits a guess. A slot may name itself and carry no artwork, and a profile that declares no wheel keeps the visualizer's own drawn patterns. Imports preserve the stable profile ID. Changed content for the same manufacturer/name becomes a new local revision; an ID collision with a different family is invalid.

Startup reads the same archives through `FixtureLibrary::load_fixture_package_directory`. Package updates apply only while the last package-installed revision is current. A later operator revision is preserved. Patched shows remain insulated by their embedded profile snapshot.

## Schema relationships

- `FixtureProfile` owns fixture-wide identity, physical facts, optics, assets, modes, and safety policy.
- `FixtureProfile.optics` describes what the fixture's light looks like: relative `output`, `sharpness` and `uniformity` as `0..=1` fractions, and a `light_source` with a `round`/`oval`/`rectangular` form and width and height in millimetres. Every field is optional, and an omitted field is derived from the declared `fixture_type` rather than guessed per fixture. Declare a figure only when it is measured or documented; leaving the block empty is the correct answer for a fixture whose manual says nothing about its optics.
- `FixtureProfile.laser` is present only on a laser and describes what its channels cannot: the scan engine as source text plus the scanner it drives — `scan_angle_degrees` (and an optional `scan_angle_y_degrees`), `points_per_second`, `divergence_milliradians`, `aperture_millimetres`, and `optical_power_milliwatts`. Every field is optional and an omitted one is derived from the declared `fixture_type`, exactly as `optics` works. The script is the exception in kind rather than in optionality: a laser without one is rendered dark and diagnosed, because a laser's output is decided by a pattern bank inside the fixture and inventing one would misrepresent the product. Do not add a `laser` block to a fixture that is not a laser.
- `FixtureProfile.gobos` is the fixture's gobo wheel, and belongs on any fixture that has one. A gobo channel says which slot is in the beam; nothing in the channel model can say what is etched on the glass, so the wheel is declared here and the artwork travels with the package. Slots need not be contiguous, the open slot need not be declared, and the wheel is as long as its highest slot. Declaring a wheel with no artwork is still worth doing: it divides the channel into the slots the fixture actually has.
- `FixtureMode` owns independent splits, logical heads, ordered physical channels, color systems, control actions, and geometry.
- `FixtureSplit.number` is an independently patchable address block.
- `FixtureChannel.head_id` selects its logical head and `FixtureChannel.split` selects its patch block. One head may own channels in several splits. Row order derives primary slots per split; `secondary_slots` reserves fine and higher bytes.
- U8 has zero secondary slots, U16 one, U24 two, and U32 three.
- A physical channel retains a fixture-facing attribute, maps it to one canonical semantic attribute with an explicit normalized transform, and has non-overlapping channel functions. Function arbitration uses configured priority.
- Multi-cell emitters need separate logical heads when independently programmable. Fixture-wide dimmer, shutter, macro, and movement controls stay on the master/shared head.
- A mode geometry graph may be empty when the packaged GLB or a broad device-type fallback supplies the Stage representation.

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
