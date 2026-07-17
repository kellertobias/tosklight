# ToskLight transferable fixture-package contract

Re-check the implementation because schema and verification seams can evolve.

## Package ownership

All shipped and operator-transferred fixtures use `fixture-library/*.toskfixture`. Fixture definitions must not be added to Rust or TypeScript catalogs. A `.toskfixture` is a ZIP containing one root `fixture.json` plus only the assets referenced by that manifest.

The wrapper is:

```json
{
  "$schema": "https://tosklight.app/schemas/fixture-package-v1.json",
  "format": "tosklight.fixture",
  "format_version": 1,
  "profile": {}
}
```

The profile is schema v2 and must have `reserved_source: null` or omit catalog ownership. Optional assets are relative paths under `assets/`: photograph and stage icon in PNG/JPEG/WebP, and a self-contained GLB 2.0 model. Imports preserve the stable profile ID. Changed content for the same manufacturer/name becomes a new local revision; an ID collision with a different family is invalid.

Startup reads the same archives through `FixtureLibrary::load_fixture_package_directory`. Package updates apply only while the last package-installed revision is current. A later operator revision is preserved. Patched shows remain insulated by their embedded profile snapshot.

## Schema relationships

- `FixtureProfile` owns fixture-wide identity, physical facts, assets, modes, and safety policy.
- `FixtureMode` owns independent splits, logical heads, ordered physical channels, color systems, control actions, and geometry.
- `FixtureSplit.number` is an independently patchable address block. Every head belongs to exactly one split.
- `FixtureChannel.head_id` selects its logical head. Row order derives primary slots per split; `secondary_slots` reserves fine and higher bytes.
- U8 has zero secondary slots, U16 one, U24 two, and U32 three.
- A physical channel has one default semantic attribute and non-overlapping channel functions. Function arbitration uses configured priority.
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

- Validate every archive with `cargo run -p light-fixture --bin fixture-package -- validate fixture-library/*.toskfixture`; write/export round trips must retain normalized content and stable IDs.
- Assert exact profile/mode inventory, slot coverage, resolution bytes, logical heads, safe/Highlight values, and GLB/icon presence where required.
- Start `light-server` with `--fixture-package-dir "$PWD/fixture-library"` and verify `/api/v1/fixture-profiles` plus `/api/v1/fixture-library`.
- Start twice against the same temporary data directory to prove idempotence.
- Verify a later operator revision is not overwritten by a changed startup package.

GLB is optional unless exact manufacturer appearance is requested or the broad device-type fallback is inadequate. When supplied, verify useful non-collapsed bounds, intended node bindings, pivots, emitter ownership, and finite non-zero scales for visible parts.
