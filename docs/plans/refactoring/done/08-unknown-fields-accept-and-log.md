# 08 — Tolerant wire typing: build the accept-and-log mechanism for unknown fields

## Context (api-rules §5, verified 2026-07-23)

Rule: unknown/additional body properties are **accepted and logged**, never rejected.
Current state: **152 live `#[serde(deny_unknown_fields)]` attributes across 26 files in
`crates/wire/src/v2/`** (plus one doc-comment mention at `group_management.rs:44`).
Heaviest: `preload_values.rs` (14), `programming_update/preview.rs` (12),
`cue_recording.rs` (9), `speed_group.rs` (9), `patch.rs` (8), `playback_topology.rs` (8),
`group_management.rs` (7). Full list in the audit; re-grep at execution time:
`rg -c 'deny_unknown_fields' crates/wire/src/`.

Separate, out of scope here: `crates/fixture/src/package/manifest.rs:22` is a package
manifest format, not an HTTP/WS wire type — leave it.

The rule says existing violations are brought into compliance **as touched**, so this chunk
does not sweep all 152. It builds the mechanism so later chunks can comply cheaply.

## Work

1. Build one shared mechanism in `crates/wire` (or the server's extraction layer) that
   deserializes a typed body while **collecting** unknown top-level keys instead of
   rejecting them — e.g. a generic `Tolerant<T>` extractor: deserialize to
   `serde_json::Value`, diff keys against the typed struct's round-trip, log the extras
   (target: server log with route + key names, no values), return `T`.
2. Keep strict validation for *known* fields: type mismatches still produce a clear 4xx
   naming the field (api-rules §5 first bullet).
3. Apply it to a first small route as the proving case (pick one touched by chunk 03/04,
   e.g. programmer-values), removing that route's `deny_unknown_fields`.
4. Document the pattern in `docs/engineering/api-rules.md` §5 (one sentence pointing at the
   helper) so every later chunk uses it.

## Definition of done

- Reusable helper exists with unit tests: (a) unknown key accepted + logged, (b) known-field
  type mismatch → 4xx naming the field, (c) clean body → identical behavior.
- At least one production route converted; its `deny_unknown_fields` removed.
- api-rules.md points to the helper.

## Verification

```sh
cargo test -p light-wire -p light-server
npm run test:unit
npm run test:e2e-api
npm run test:e2e   # full suite gate
```

## Decisions

None — the policy is decided; the helper design is implementation detail. If nested-object
unknown keys turn out to need recursive handling, start top-level-only and note it.

## Maintainer ruling (2026-07-23, recorded from session)

Unknown fields are tolerated by definition: "if the field is unknown it isn't used, so
it doesn't matter; if it is known, it is part of the contract and gets checked." Apply
this when implementing: accept-and-log unknown fields everywhere, and drop the
forged-`mode`-field rejection guard
(`programmer_values_wire_rejects_transient_or_mode_fields`) together with the
`deny_unknown_fields` attributes it depends on — replace it with coverage that known
contract fields are still validated. (Chunk 03b temporarily restored strictness on the
two programmer-values enums to keep that guard green until this chunk lands.)

## Result

- Added the server-owned generic `TolerantJson<T>` Axum extractor. It preserves Axum's
  field-path validation errors for known fields, accepts unknown fields at any nesting
  depth through `serde_ignored`, and emits one bounded warning with the matched route
  template and field paths only (never values).
- Converted `POST /api/v2/users/{user_id}/programmer-values/actions` as the proving route.
  The complete inbound graph now tolerates future request, action, batch-mutation, and
  timing fields; response and capture-mode DTO strictness was left untouched.
- Replaced the forged-`mode` rejection guard with production-route coverage proving
  unknown top-level/action/timing fields are accepted while a mistyped
  `expected_revision` remains a field-naming 400. Added extractor tests for accepted and
  redacted logging, known-field mismatch, and unchanged clean-body behavior.
- Regenerated the Programmer-values request schema and documented the shared extractor in
  `docs/engineering/api-rules.md` §5.
- Verification:
  - `cargo test -p light-server tolerant_json --lib`: 3 passed.
  - focused production-route test: 1 passed.
  - `cargo test -p light-wire`: 80 unit tests + 1 generated-contract test passed.
  - the combined server/wire run passed 430 server tests before one environment-only CITP
    cache failure (`Operation not permitted`); that exact test passed outside the sandbox,
    and the later complete unit gate passed all 431 server tests with 1 ignored.
  - `npm run test:architecture`: 24 checks passed; source-size hard-limit ratchet stayed at
    zero violations.
  - `npm run test:unit`: 1,991 control-ui tests passed plus the full non-Tauri Rust
    workspace, frontend build, architecture, and generated-contract gates.
  - `npm run test:e2e-api`: 85 passed / 1 skipped.
  - `npm run test:e2e`: 283 passed / 11 skipped / 0 failed.
- No follow-up chunk was needed. The initial sandboxed E2E attempt failed only because
  workers could not bind loopback (`listen EPERM`); both acceptance gates passed outside
  the sandbox.
