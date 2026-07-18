---
name: build-light-fixtures
description: Build, revise, research, package, and verify transferable fixture profiles in this ToskLight repository, including `.toskfixture` archives, manufacturer, Generic, and visual-only Venue fixtures, DMX modes and channels, fine bytes, logical heads, splits, color systems, safe defaults and Highlight values, physical metadata, photographs, icons, GLB models, and Stage geometry. Use when adding a lamp, scenic element, or fixture family, translating a manufacturer manual or DMX chart into the shipped fixture library, correcting an existing fixture package, checking that every documented mode is represented, transferring fixtures desk to desk, or validating fixture-library output without clicking through the ToskLight UI.
---

# Build ToskLight Fixtures

Build desk-transferable fixture packages from authoritative documentation and prove them through the repository package codec, tests, serialized profiles, and local API. Do not add fixture definitions to Rust or TypeScript product code. Do not use UI clicking as the primary authoring or verification path.

## Workflow

1. Read `AGENTS.md`, `docs/help/20-Show-Setup/03-fixture-library.md`, `docs/acceptance-criteria.md`, [references/tosklight-fixture-contract.md](references/tosklight-fixture-contract.md), and the current package tests. Preserve unrelated worktree changes.
2. Identify the exact manufacturer, model, hardware revision, firmware/manual revision, and requested scope. Treat similarly named products as different fixtures unless the manual explicitly defines them as modes of one product.
3. Find the official product page, user manual, and DMX chart. Prefer manufacturer-hosted sources. Record exact mode names, footprints, channel order, coarse/fine relationships, functions, defaults, safe shutter/open values, pan/tilt ranges, dimensions, weight, power, emitters, zones, and pixels. Do not invent undocumented data.
4. Write a compact inventory before editing: stable profile identity, every requested mode and footprint, multi-head layout, physical facts, assets, and source URLs. Reconcile “all modes” against the complete manual list.
5. Export or unpack a nearby `.toskfixture` as the schema template. Author `fixture.json` and optional `assets/photograph.*`, `assets/icon.*`, and `assets/model.glb` under `assets/fixture-library/`. The archive itself, not a code generator retained in the product, is the deliverable.
6. Generate UUIDs once for the profile, modes, heads, channels, functions, and geometry parts, then retain them. UUID v4 is acceptable. Splits use stable positive numbers, not UUIDs. Preserve manual mode order and create new identities only for genuinely different semantic objects.
7. Validate every new or changed archive with the repository package validator. Add focused tests for exact mode names and footprints, slot ownership, fine-byte allocation, logical heads, safe Highlight values, asset round trips, startup idempotency, and custom Stage geometry.
8. Run the non-interactive verification ladder below. Inspect the loaded API profile and package inventory; do not claim success from JSON or source inspection alone.
9. Update fixture-library help when operator-visible shipped content or the package contract changes. Report manual ambiguity or deliberately unsupported behavior.

## Authoring contracts

- ToskLight has no built-in fixture definitions. Shipped fixtures are normal `.toskfixture` archives loaded by the same reader as desk-to-desk imports.
- Keep one profile per physical fixture family with ordered modes inside it. Use separate profiles for physically different lanterns even when their DMX personality is identical.
- For a scenic object that must never be DMX patched, set `patch_policy` to `visual_only`, give each mode a stable split with footprint `0`, and keep channels, color systems, and control actions empty. Keep a head for geometry ownership. Verify that parent and multi-patch instances remain addressless.
- Preserve all requested personalities. Never silently collapse modes because channel counts match.
- Assign every published DMX slot exactly once. Represent fine and higher bytes as secondary slots on the owning channel in documented byte order.
- Model independently programmable cells or zones as logical heads. Assign each physical channel to its independently patchable split; one head may own channels in several splits. Keep fixture-wide controls on the master/shared head.
- Use exact raw defaults and Highlight values. A useful Highlight look normally needs full intensity, physical white, and documented shutter-open. Leave movement, reset, lamp control, macros, and hazardous functions safe.
- Preserve physical units and manufacturer ranges. Mark unknown data as unknown rather than estimating it.
- Package raster photographs/icons as PNG, JPEG, or WebP. Package recognizable or manufacturer-specific geometry as a self-contained GLB 2.0. Asset fields must use relative `assets/` paths.
- Set `model_units` to `metres` when GLB coordinates are authored as real-world metres; use the default `auto` only for conventional lamp models that should be normalized to profile dimensions.
- Keep geometry functional: emitter placement, moving pivots, and multicell layout must agree with head ownership. Verify GLB bounds and semantic shape without relying only on visual judgment.
- Keep patched shows compatible. Package updates create local immutable revisions and must preserve later operator-created revisions.

## Non-interactive verification

Start narrow, then widen according to risk:

```sh
cargo fmt --all -- --check
cargo run -p light-fixture --bin fixture-package -- validate assets/fixture-library/*.toskfixture
cargo test -p light-fixture
cargo check -p light-server
(cd apps/control-ui && npm run typecheck)
(cd apps/control-ui && npm test -- --run src/windows/stage3dScene.test.ts)
./test unit
```

Build and start an isolated server with both a temporary desk directory and the repository package directory:

```sh
cargo build -p light-server
fixture_verify_dir=$(mktemp -d)
target/debug/light-server \
  --data-dir "$fixture_verify_dir" \
  --fixture-package-dir "$PWD/assets/fixture-library" \
  --bind 127.0.0.1:5011 \
  --osc-bind 127.0.0.1:0 \
  >"$fixture_verify_dir/server.log" 2>&1 &
fixture_server_pid=$!
```

Poll `/api/v1/readiness`, then fetch `/api/v1/fixture-profiles`. Assert the exact manufacturer/name, mode names and footprints, stable IDs, `reserved_source: null`, and required asset data URLs. Exercise package export and re-import through the authenticated REST endpoints or Rust codec, comparing normalized profiles and stable IDs.

Stop and restart the isolated server against the same temporary data directory and confirm IDs and revisions are unchanged. Terminate only the recorded PID and remove only the recorded temporary directory. Do not run `./build open` unless desktop packaging itself is in scope. When shipped-library help changes, also run `./build manual`.

## Completion criteria

- Authoritative manual title, revision, firmware applicability, and source URLs are recorded in `profile.notes` until the schema gains structured provenance.
- Requested mode inventory matches the package exactly.
- The `.toskfixture` contains `fixture.json` and every referenced asset, passes package validation, and round-trips without identity loss.
- Profile validation, slot coverage, safe values, and focused tests pass.
- Startup loads the package through `--fixture-package-dir`, is idempotent, and preserves operator revisions.
- The live API profile matches the package inventory and required identities.
- No fixture definition or fixture-specific model is compiled into product code.
- No UI clicking was required to establish correctness.
