# Typed Active-show Object Contracts

## Goal

Remove generic `serde_json::Value` from application-level active-show mutation and change events
while preserving forward-compatible unknown stored fields inside the show codec layer.

Estimated effort: 0.5–0.8 Codex day.

## Queue dependency

Completed. Plan 07 established the final semantic command/event families and generated client
boundary. This plan replaces the bodies carried by those application events without defining a
competing intermediate transport.

## Required work

1. Replace generic Put/change bodies with a discriminated enum covering Cue Lists, Groups,
   Playbacks, Pages, Presets, layouts, routes, Patch layers, and other supported families.
2. Keep lossless tolerant decoding and unknown-field round trips in `light-show`; do not push raw
   JSON back into application services.
3. Make validation and compile preparation exhaustive by object family.
4. Generate matching wire/client types and runtime decoders.
5. Remove casts and generic body handling after all callers migrate.

## Acceptance and verification

- Unsupported kinds fail explicitly; known kinds cannot carry another family's body.
- Unknown fields survive read/mutate/write and legacy show migration.
- Generated contracts, codec round trips, mutation/event tests, and full persistence startup
  checks pass.

## Result

### Changes

- Added an eight-family `ActiveShowObjectBody` application contract for Cuelists, Groups, Patch
  layers, Playbacks, Playback Pages, Presets, Stage layouts, and User layouts. Put mutations,
  changes, undo, programming, topology, route, HTTP, and event adapters now carry the matching
  typed family instead of a generic JSON body.
- Moved the lossless typed/raw body and merge implementation into `light-show`, keeping unknown
  root and nested fields at the portable-show codec boundary. Legacy empty Presets, group bodies
  without an embedded ID, and early opaque User-layout payloads remain readable and lossless.
- Generated a discriminated eight-family wire union and added exhaustive frontend runtime
  decoders and snapshot collections. Typed change events reject unknown kinds, while raw snapshots
  and selective-import notifications retain their deliberately tolerant transport behavior.
- Added exhaustive family, wrong-family, unknown-kind, legacy, lossless round-trip, wire
  discriminant, and frontend decoder coverage.

### Tests

- `cargo fmt --all -- --check` — passed after formatting.
- `cargo check -q -p light-wire -p light-application -p light-headless-runtime` — passed.
- `cargo test -q -p light-show --no-default-features` — 57 passed.
- `cargo test -q -p light-application --no-default-features` — 416 passed.
- `cargo test -q -p light-wire` — passed, including generated-contract freshness.
- `cargo test -q -p light-headless-runtime` — 480 passed and 1 ignored; the sole in-sandbox
  loopback-listener failure passed when rerun with local bind access.
- `npm run typecheck --workspace @tosklight/light-desktop` — passed.
- Focused desktop show-object and event-transport tests — 16 passed.
- `npm run test:e2e-api` — 21 passed with local loopback bind access.

### Limitations

- `npm run test:unit` reaches the architecture gate and stops on four unrelated concurrent CSS
  duplication findings in `hardware-dense.css`, `playback-colors.css`, and `window-kit.css`. Those
  files were not changed or staged by this plan.
- Existing `ts-rs` warnings about unsupported Serde attributes remain non-fatal during wire
  generation and Rust test builds.

### Commit

- `feat(show): type active show object contracts`
