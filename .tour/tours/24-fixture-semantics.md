---
slug: fixture-semantics
title: "Fixture Semantics: Attributes, Modes, and DMX Channels"
components: [backend, engine, control-ui]
order: 34
---

# Fixture Semantics: Attributes, Modes, and DMX Channels

Operator contract: `docs/help/20-Show-Setup/01-fixtures-and-patch.md`,
`docs/help/20-Show-Setup/03-fixture-library.md`, and
`docs/help/20-Show-Setup/06-attribute-reference-and-activation.md`. Fixture, Patch, Highlight, and
DMX acceptance is exercised in `tests/11-update-highlight-fixture-profiles-and-matter.spec.ts`,
`tests/35-fixture-address-screen.spec.ts`, and `tests/03-network-output-protocols.spec.ts`.

An attribute is operator meaning. A DMX channel is one encoded transport slot. Keeping them
separate is what lets one Programmer action work across different fixture modes.

## Package and immutable revision

`crates/shared/fixture/src/package/` validates `.toskfixture` archives, profiles, icons, photographs, and
GLB models. `crates/shared/fixture/src/profile/` owns modes, logical heads, semantic attributes, attribute
groups, activation groups, channels, splits/functions, defaults, Highlight values, resolution,
color calibration, and geometry.

The desk Fixture Library assigns an immutable profile revision. Patching copies that revision into
the portable show; a later library edit never silently changes an existing show.

## Patch record

`crates/light/src/show_patch/` validates fixture numbers, stable fixture/head identities,
selected mode, universe/address, split assignments, multipatch, stage transform, Highlight
overrides, and profile references as one batch.

An unpatched fixture has no physical output binding, but keeps every semantic identity. It remains
selectable, programmable, groupable, recordable, and visible.

## Compilation

`crates/shared/fixture/src/portable_patch/compiler.rs` resolves the selected mode and logical-head
topology. `crates/light/src/show_compiler/patch.rs` places the compiled fixtures in the
immutable `EngineSnapshot`.

Semantic attributes are addressed by fixture or logical head plus attribute. Attribute groups
organize related controls; activation groups express mutually exclusive activation behavior.
Neither is a DMX slot number.

## Projection to DMX

`crates/light/domain/engine/src/profile_projection.rs` and `profile_projection_plan.rs` map resolved semantic
values through the selected mode. `crates/shared/fixture/src/profile/encoding_plan.rs` handles channel
functions, inversion, transfer curves, virtual intensity, exact raw values, and MSB-first fine
bytes. Splits and multipatch affect binding without changing the semantic Programmer value.

The output layer receives complete frames. It does not query the fixture library or reinterpret
attributes.

## Failure path

Profile digest/revision mismatch, missing selected mode, overlapping patch, invalid split
assignment, or inconsistent logical-head topology rejects the whole candidate. Legacy inline
profiles migrate to deduplicated snapshots without losing selected modes or unknown fields.

## Exercise

Open one profile test in `crates/shared/fixture/src/profile/tests/resolution.rs`. Starting from its
semantic normalized value, calculate the expected coarse/fine bytes, then verify the encoding
assertion.
