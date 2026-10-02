# Semantic programming cutover

This document lists what [TL-552](https://plans.tokenet.de/TL/552) needed before it raised the
production programming contract from 0 to 1, the order in which it did it, and the result.
[TL-560](https://plans.tokenet.de/TL/560) prepared the prerequisites:
- the persistence matrix;
- the contract-1 real-startup harness;
- the programming-contract marker and the legacy validator;
- a regenerated semantic demo show candidate.

The coverage of record is
[Semantic Intent Persistence Coverage](../testing/33-semantic-intent-persistence-coverage.md).

## Current state (after the TL-552 cutover)

### The contract

- `PRODUCTION_PROGRAMMING_CONTRACT = PROGRAMMING_CONTRACT_VERSION` (1) is set in
  `crates/light/adapters/headless/src/runtime/e2e_semantic_contract.rs`.
  - A compile-time assertion there fails every build whose contract or Live family engagement
    drifts from the cutover.
- `startup_state::supported_programming_contract()` reads it.
- The `#[cfg(test)]` startup harness (`startup_contract_override`) still lets unit tests run real
  startup at another contract. Its main use now is the contract-0 half of compatibility tests: what
  an older build does with contract-1 data.
- The `e2e-semantic-contract` cargo feature is a retained no-op. `npm run test:e2e-semantic` still
  builds a separately named server with it, and that server behaves exactly like production.
  `LIGHT_E2E_SEMANTIC_PROGRAMMING_CONTRACT` is no longer read.
- The TL-548 C3 all-family Live path is engaged in production (`live_family_adapters_opted_in()`
  is `true`). `LiveFamilyAdapters::engaged` still requires a contract-1 engine.
- No `== 0` contract gate and no "Contract0" path remains. The legacy scalar Aim preset branch in
  `command_presets.rs` was removed.

### The marker

- The metadata key `light.programming_contract` is written by `ServerActiveShowUnitOfWork::commit`.
- Since TL-552 it is also written by the direct object writers `ActiveShowRepository::put_object`,
  `delete_object` and `mutate_objects_atomically`, through `ShowStore::*_at_contract`. These cover
  the object routes on an inactive show and Preload Store.
- In both cases the marker is written only when the writer supports contract 1 or later and the
  write touches a programming object.
- It is a metadata key, not a `SHOW_SCHEMA_VERSION` bump, for three reasons:
  - `SHOW_SCHEMA_VERSION` describes the SQLite layout;
  - `ShowStore::open` migrates every older file to the current schema in place, so the schema
    cannot carry provenance;
  - unknown metadata keys are retained everywhere.

### The validator

The validator is `light_show::validate_show_programming_contract`. It runs read-only at contract 1,
before any writable open, at these points:
- startup (`load_active_show_runtime_for_startup`);
- activation, open and rollback (`prepare_show_for_runtime` and
  `prepare_show_activation_for_runtime`);
- revision and Architect replacement of the active show;
- Programmer restore, Playback runtime restore and Output runtime restore. These check the stored
  JSON, and runtime recovery preserves it.
- object Undo/Redo. A step that would restore a legacy body is refused.

It rejects three kinds of legacy programming:
- legacy normalized `pan`/`tilt`;
- legacy `color.*` component programming;
- percentage `zoom`, that is a `normalized` or `spread` value at `zoom`. Semantic Zoom in degrees
  shares the address and is accepted.

It also rejects a marker that is newer than the runtime, or one it cannot read.

Synthetic test states (`test_state()`) engage the activation gate at their engine contract, just
as `build_app_state` does for a real startup.

### Recovery

- The message is visible and actionable.
- The original bytes are unchanged and no schema is migrated in place.
- Unrelated desk settings are intact.
- A separate show can be created and opened.
- A refused Undo/Redo changes nothing: no revision, no backup, no event and no runtime change.

### The packaged default show

`assets/demo.show` is the regenerated contract-1 demo:
- marker 1;
- no legacy findings;
- the same object counts as the legacy demo.

It is `include_bytes!` in `default_show.rs`. The test
`the_packaged_default_show_is_semantic_after_the_tl552_swap` pins this. It checks four things:
- production startup loads the default show;
- the default show has marker 1 and no legacy findings;
- it holds semantic Angles and Color presets;
- an older contract-0 runtime refuses it visibly.

## Decisions (TL-552 owner)

- **Percentage Zoom is legacy.** `legacy_programming_family("zoom")` is
  `LegacyProgrammingFamily::Zoom`. It applies only with a `normalized`/`spread` value, or as a
  scalar Dynamic lane. Test: `light-show`
  `percentage_zoom_is_legacy_and_zoom_in_degrees_is_not`.
- **Session and Programmer payloads have no extra marker field.** The content-based gate is
  sufficient for three reasons:
  - the stored Programmer JSON is scanned whole (Normal, Preload, Undo/Redo);
  - typed values carry their own `required_programming_contract`, which an older runtime already
    refuses;
  - a marker field would add a second source of truth that could disagree with the content.
- **Playback and Output runtime payloads are validated at restore.** This uses
  `show_programming_contract::check_runtime_payload` before decode.
  - A legacy payload is preserved in the desk setting and in `backups/runtime-recovery-*.json`,
    and the desk starts in recovery, exactly like the Programmer.
  - Test: `legacy_playback_and_output_runtime_payloads_are_preserved_for_recovery_at_contract_one`.
- **Undo history.** An Undo or Redo that would restore a pre-cutover legacy programming body is
  refused quietly.
  - The refusal is `ActionErrorKind::Invalid` (HTTP 400) with an actionable message.
  - Nothing changes, and nothing crashes.
  - A contract-0 runtime still restores the body.
  - Test: `undo_and_redo_refuse_a_legacy_programming_body_quietly_at_contract_one`.
- **Direct object writes stamp the marker** (see above). Test: `light-show`
  `direct_object_writes_stamp_the_marker_only_for_programming_at_contract_one`.
- **Family adapters.** Production engages the hybrid all-family Live path at contract 1, so the
  semantic producers are live. Known limits carried over from TL-548 C3:
  - `CommittedDynamicOutput.ordinary` is `None` on the hybrid path, so the visualization's
    dynamic-stack detail shows no ordinary values for a hybrid frame;
  - the Pending executor is not woken from the Live tick, so it polls every 40 ms;
  - stale-frame mapping is message-based.
  
  None of these blocks the cutover.
- **Fixture shows and seed.** Three things were audited:
  - `default_show/{definition,seed}.rs` seed only fixtures. Their `pan`/`color.red` names are
    fixture channel attributes, not programming.
  - `tests/fixtures/compact-rig.show` holds groups without programming; `default-stage.show`
    holds no programming. Both have 0 legacy findings and were left unchanged.
  - Profile degree, travel and optics metadata is a separate data gap and was not changed here:
    no shipped profile declares a Zoom `opening_convention` (see the E2E opt-in handoff). A mode
    with one Pan and one Tilt channel but no authored Position physical graph gets a derived,
    estimated one in the transient runtime projection (`light_fixture::apply_derived_position_physical`,
    TL-552 Position fallback; named exclusions in `PositionDerivationExclusion`). An Angle Dynamic
    over an owner with no static Position starts from the declared default pose
    (`Engine::declared_default_position`, a frame-local base like a FixAT base).

## Live write gate (TL-552 follow-up)

At contract 1 no accepted live write can make the show or the stored Programmer fail the
validator on the next open. Every writer applies the validator's own rule
(`light_show::legacy_attribute_value` / `legacy_programming_address`):

- **Programmer values** (`light-application` `values_legacy.rs`, Normal and Preload lanes, HTTP and
  WebSocket): `set_fixture`/`set_group`/`batch`/`set_selection` and the expanded legacy Color range
  at a legacy address are refused as `Invalid` (400) before mutation. An `apply_intent` at a Color
  component whose encoder slot carries the same percentage on the semantic owner (Red, Green, Blue,
  Amber, UV, Saturation, White Blend) is converted to that `component_edits` edit of `color`;
  every other legacy intent (Pan/Tilt percentages, Hue, Temperature, Duv, White, CMY, percentage
  Zoom) is refused.
- **Dynamic programmer values** (`dynamics/legacy_addresses.rs`): a percentage FixAT at a legacy
  address (HTTP FixAT, command-line `ATTRIBUTE pan FixAT 50`) is refused; a Release of native
  Position/Color channels (command line, OSC keypad) is stored as the Release of `position`/`color`.
- **Objects**: the unit-of-work commit (`show_programming_contract::check_transaction`) and the
  direct writers (`ShowStore::*_at_contract`) refuse a programming body with legacy findings before
  anything is written (recordings, Updates, object intents, Dynamics create, test-bench seeding).
- **Validator precision**: a `zoom` row is legacy only as a percentage (normalized/spread value,
  FixAT or static percentage, scalar lane). A Release of the Zoom owner and a running typed Zoom
  Dynamic share the address and are no longer false positives.
- **First semantic Color edit**: a colour-capable head with no colour starts from open white
  (`values_environment::semantic_color_defaults`), so the converted Color edit and the operator's
  first encoder turn or dialog pick no longer fail with "component edit requires a complete current
  or default family".

Colour fallback (closed after the follow-up): a mode without an authored `color_physical` model
gets a derived one in the transient runtime projection (`light_fixture::apply_derived_color_physical`,
called by `apply_runtime_profile_compatibility`) from the colour systems Color Intent reads for the
head. Authored systems keep their calibration (measured stays measured, nominal becomes estimated);
heads with no authored system are resolved from channel names with unknown provenance and report
Uncalibrated. Output, the Stage/preview projection and the accepted-frame report all evaluate it.
Colour coverage (TL-552 follow-up): each head now picks its most capable engine (emitters/CMY,
else a hue/saturation engine as a nominal sRGB HSV grid of whole-path observations, else the wheel
with the most coloured steady slots, else white only). Every other colour control (CTC, tint,
colour point, a wheel or macro in front of emitters, a second wheel) is parked: a filter whose only
known range is its neutral state with a unit transmission, which the forward model and the fitter
treat as no filter. A master-shared head's colour controls over child heads with emitters stay at
their defaults. `FixtureMode::derived_color_outcome` names a reason for everything else (shipped:
only Lustr `HSI Plus 7` / `HSIC Plus 7`, direct emitters layered over the HSI engine); the
accepted-frame report shows those heads as Unsupported with that reason and adds a `note` naming
parked controls. A whole-family FixAT with no static underlay gets its own family as the
frame-local static baseline (`programming_projection/hybrid/fixed_bases.rs`), so it renders like
any other Color/Position/Focus/Zoom value.

## TL-552 checklist

### Matrix and validator

- [ ] Every cell in the matrix of record is green or carries an explicit, accepted exclusion,
      including the real-startup row at contract 1 (already green). The remaining gaps are listed
      in the matrix document: Media color, Direct undo, MVR, unpatched behaviour, and Preload GO
      for Focus and Media.
- [x] Marker writer and reader exist for shows; legacy rejection is tested at contract 1; the
      original file is preserved; a separate empty show is offered; desk preferences are intact.
      This is done for portable shows and Programmer JSON. Two decisions remain:
  - [x] Decide whether session and Programmer payloads also get an explicit marker field.
        Decided: no, the content-based gate is sufficient (see Decisions).
  - [x] Decide whether Playback and Output runtime payloads need the legacy-key validator.
        Done: validated at restore, preserved and recovered visibly.
  - [x] Decide on undo history (`object_history`): an undo could restore a pre-cutover legacy
        body. Done: such an Undo/Redo is refused quietly.
- [x] Decide whether a percentage `zoom` (a `Normalized` value at `zoom`) is legacy and must be
      rejected. Plan §13 says "normalized-position/zoom"; the TL-560 owner scope covered Position
      and Color only. If yes, add `"zoom"` to `legacy_programming_family` together with its test.

### Raising the contract

- [x] Raise `PRODUCTION_PROGRAMMING_CONTRACT` and every gate site in the TL-560 brief §1 in the
      same change. A grep for `== 0`, "Contract0" paths and `supported_programming_contract() == 0`
      must find none left (for example `command_presets.rs:94`). The compile-time assertion in
      `e2e_semantic_contract.rs` must move with it.
- [x] Engage the activation gate for synthetic test states. `test_state()` runs the engine at
      contract 1 without the startup path, so `ActiveShowResource::legacy_programming_gate` stays
      0 there. Before TL-552 engages it there (or derives it from the engine contract), it must
      migrate the headless tests that open legacy content at contract 1. With the gate keyed on
      the engine contract, 23 tests failed. Among them:
  - every `position_intent_authoring` test;
  - `programmer_values_ws_action_tests` and `programming_interaction_ws_action_tests`;
  - `color_model_route_tests::switching_a_programmed_show_is_refused_until_its_lossy_impact_is_acknowledged`;
  - `startup_tests::clean_default_load_creates_a_pristine_copy_without_replacing_manual_changes`;
  - `show_activation_caller_tests` for the clean default.

  Most open the legacy default demo or seed `color.*`/`pan` presets.
- [x] Decide whether direct object writes (`ActiveShowRepository::put_object` /
      `mutate_objects_atomically`, for example legacy object routes for inactive shows) stamp the
      marker. Today only the unit-of-work commit stamps it. The validator does not depend on the
      marker.

### Regenerated owned data

- [x] Regenerate `assets/demo.show`. Swap step:
  1. In the enabling change, build the E2E semantic server (`npm run test:e2e-semantic-build`, or
     the production binary once it reports contract 1).
  2. Run the generator with `LIGHT_DEMO_SEMANTIC=1`. Leave `LIGHT_DEMO_SHOW_OUTPUT` unset to write
     `assets/demo.show`, or set it to a scratch path first. For example:

     `LIGHT_DEMO_SEMANTIC=1 LIGHT_DEMO_SHOW_OUTPUT=<path> npx playwright test --config <config extending playwright.config.ts with semanticProgrammingContract: true> tests/76-demo-show-generation.spec.ts -g DEMO-GENERATOR-001`

     Once the contract is raised, make `LIGHT_DEMO_SEMANTIC` the default (or delete the legacy
     branch) in `tests/support/plannedDemo{Presets,Playbacks,Dynamics}.ts`.
  3. Verify it with `76-packaged-demo-asset`, `00-canonical-virtual-playback-schema`,
     `tests/bench/show/plannedDemo*.test.ts` and `light-show`
     `inspect_show_programming_contract` (no legacy findings, marker 1).
  4. Invert the contract-1 half of `the_packaged_default_show_is_legacy_until_the_tl552_swap`.
  5. `npm run demo-show` and release CI (`.github/workflows/release.yml`) ship the swapped file
     unchanged.

  The TL-560 candidate is `.artifacts/tmp/tl560-core/demo/demo-semantic.show`. See the TL-560
  core handoff for its verification.

  **Result.** The swap was done in two passes:
  1. The TL-560 switch ran on the contract-1 production E2E server into a scratch file, and that
     file was staged as `assets/demo.show`.
  2. The legacy branch was then deleted (the generator now has no `LIGHT_DEMO_SEMANTIC` switch),
     and `npm run test:demo-show` regenerated `assets/demo.show` (sha256 `c0471d98…89fb0b`).

  The result has marker 1 and 0 legacy findings. It is identical to the TL-560 candidate except for
  server-generated UUIDs (logical heads, Dynamics, layout). `npm run demo-show` only stages a copy
  of this file.
- [x] Audit the default seed and the fixture shows:
  - `default_show/{definition,seed,tests}.rs` seed only fixtures;
  - `tests/fixtures/compact-rig.show` and `default-stage.show` hold no legacy programming.

  Decide whether their profiles need degree, travel or optics metadata.
- [x] `76-demo-show-generation`, `76-packaged-demo-asset` and `00-generate-show-files` pass, and
      so do `40-semantic-generate-show-files` and `tests/bench/show/*.test.ts`.
  - `00-generate-show-files` registers no tests under the default configuration.
  - `tests/bench/show/plannedDemoPatch.test.ts` has a pre-existing ACL rotation failure that is
    unrelated to programming.
- [ ] The obsolete-literal grep is clean: `"color.red"`, `"pan"`/`"tilt"` with
      `kind: "normalized"` in `tests/bench/**`, benchmark builders and unit/integration JSON.
      Triage intensity-only literals. **Partly done in TL-552:**
  - Migrated: the planned demo generator and its vitest (`plannedDemo{Presets,Playbacks}.test.ts`,
    `plannedDemoVirtualPlaybackZones.test.ts`).
  - Migrated: the headless tests that stored legacy content at contract 1
    (`color_model_route_tests`).
  - Remaining, triaged:
    - `tests/bench/show/productDemoScenario.ts` (the `@demo` video recording) still reads the
      demo's Color and Position presets as `color.red/green/blue` and normalized `pan`/`tilt`. It
      needs a semantic rework before the next `npm run test:demo`.
    - Benchmark builders still author legacy scalar lanes on `color.*`/`pan`/`tilt`:
      `apps/light-headless/src/bin/light_benchmark/{headless_stress_show,scenario}.rs`,
      `tools/stage-dynamics-scene.mjs` and `tools/run-headless-stress-benchmark.mjs`. They build
      in-process or through the live API and never open a stored show. The runtime still executes
      contract-0 lanes, because contract ≤ support. Migrating them changes the TL-553
      performance baselines.
    - `tests/support/foundational/supplementalProgrammer.ts` (`@supplemental`, registers no tests
      today), `tests/bench/groups-presets/presetRecall.test.ts` (a mocked unit fixture) and the
      UI encoder catalogue / Return Home scenarios use legacy addresses as live Programmer input,
      which the live API still accepts at contract 1.

### Dependencies and verification

- [ ] TL-556, TL-548, TL-594, TL-549/550/551/554 and TL-596 are done, so producers and readers
      are ready together. The TL-630/631 defects stay fixed.
- [ ] Mixed-owner and Focus collision validation across Dynamics definitions (plan §10, item 6)
      is resolved.
- [ ] Startup goes through the real server for both new-file initialization and recovery:
      `npm run build:open`, `curl -fsS http://127.0.0.1:5000/api/v2/readiness` and
      `.artifacts/runtime/light-data/light-headless.log`. Never run it against a committed
      `.show`.
- [x] The plan and the result record the `docs/acceptance-criteria.md` "pre-v1 break" callout.
