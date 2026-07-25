# Typed Active-show Object Contracts

## Goal

Remove generic `serde_json::Value` from application-level active-show mutation and change events
while preserving forward-compatible unknown stored fields inside the show codec layer.

Estimated effort: 0.5–0.8 Codex day.

## Queue dependency

Pending, blocked until plan 07 establishes the final semantic command/event families and generated
client boundary. This plan replaces the bodies carried by those application events and therefore
must not define a competing intermediate transport.

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
