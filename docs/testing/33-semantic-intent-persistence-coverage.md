# Semantic Intent Persistence Coverage

## Purpose and status

This is the reader/writer matrix of record for requested semantic intent (Color, UV, Position,
Focus, Zoom, Media color) across every persistence path: record, recall, Update, undo,
save/reload, import, fixture replacement, live Group, unpatched fixtures and Preload GO.
[TL-567](https://plans.tokenet.de/TL/567) built the storage and selective-import matrix.
[TL-560](https://plans.tokenet.de/TL/560) closes the semantic matrix, adds the contract-1
real-startup harness, and adds the programming-contract marker with the legacy validator. The
cutover prerequisites are in
[Semantic programming cutover](../engineering/semantic-programming-cutover.md). The contract
being checked is part of
[Intention programming frame contract](32-intention-programming-frame-contract.md)
(INTENT-FRAME-003) and [Fixture-independent programming](../plans/fixture-independent-programming.md).

These are Rust tests. Payload equality proves retained authoring only. Output claims are named
per test. Production still runs programming contract 0. Contract-1 evidence comes from
application tests (whose domain default is contract 1), from the `#[cfg(test)]` startup harness,
and from the E2E semantic server. Agent tests and Review/test status remain distinct from human
acceptance.

## Deterministic cases

`crates/light/src/programming/semantic_intent_cases.rs` (test-only) provides the shared cases:

| Case | Requested intent |
| --- | --- |
| `magenta` | Exact virtual recipe, White Blend 0.25, 5600 K / Duv −0.004, relativeOutput 0.8, UV 0 |
| `magenta_with_uv` | As magenta with independent UV 0.45 |
| `warm_white_3200` | Amber recipe, White Blend 0.85, 3200 K / Duv 0.0035, relativeOutput 0.6, PreferWhite |
| `warm_white_zero_output` | As warm white with relativeOutput 0 (the serde default is 1) |
| `uv_only_black` | Black recipe and XYZ, relativeOutput 0, UV 0.9 |
| `angles` / `point_target` | Tagged Angles −35.5°/72.25° versus Point target with offset (0.5, −1.25, 2.0) m |
| `focus` / `zoom` | Independent Focus 0.37 and Beam Zoom 23.5° owners |

Live-Group assignments use `GroupFamilyAssignment` with a template and a member exception.
`assert_semantic_value` rejects native recipes, portable estimates or wheel constraints inside a
semantic record. `assert_persisted_eq` compares persisted bodies at the `f32` precision the typed
contract owns. The separate exact stored-number and no-change recording regressions cover the
historical D2 parser defect, now fixed by TL-571.

## Readers and writers

| Boundary | Writer | Reader |
| --- | --- | --- |
| Portable store | `PortableShowTransaction` → `ShowStore::apply_portable_transaction`, `put_object`, `backup_to`, `prepare_object_undo` | `ShowStore::open` → `portable_document` |
| Preset recording | `ActiveShowService::commit_programming_preset` (`prepare_recording`, lossless merge) | `ActiveShowObjectBody::decode(Preset)` after reopen |
| Cue recording | `ProgrammingService::handle_cue_recording` → `ActiveShowService::commit_programming_cue` | `prepare_show_candidate` → `EngineSnapshot::cue_lists` after reopen |
| Update | `preview_update` / `handle_update` → `commit_programming_update` | Reopened Preset and Cue bodies |
| Typed object write | `ActiveShowService::mutate_objects` (Cue list, test seeding) | `prepare_show_candidate` after reopen |
| Selective import | `SelectiveShowImportService::preview` / `apply` / `undo` | Target document decode and the installed candidate snapshot |
| Headless commit | `ServerActiveShowUnitOfWork::commit`, which stamps the programming-contract marker at contract ≥ 1 | — |
| Real startup | — | `StartupState::load` → read-only contract gate → `prepare_show_load` → `Engine::prepare_snapshot`; Programmer restore with its JSON migrations and contract gate |
| Show activation | — | `prepare_show_for_runtime` / `prepare_show_activation_for_runtime` (open, open clean default, rollback, revision restore) behind the same gate |

## Programming-contract marker and legacy validator (TL-560)

`light-show` owns the marker and the validator (`crates/shared/show/src/programming_contract.rs`):

- **Marker.** The metadata key `light.programming_contract` holds the writer's contract. A
  headless commit stamps it in the same SQLite transaction when the runtime supports contract 1
  or later and the transaction writes or deletes a programming object (`preset`, `cue_list`,
  `group`, `dynamic`, `playback`, `playback_page`). Contract-0 writers never stamp. It is a
  metadata key, not a schema bump. Here is why:
  - `SHOW_SCHEMA_VERSION` describes the SQLite layout.
  - `ShowStore::open` raises it in place on every older file, so it cannot record which contract
    wrote the content.
  - Unknown metadata keys are retained by every reader and writer.
- **Validator.** It is dormant at contract 0 and never opens the file there. At contract 1 or
  later it inspects the file through a read-only connection before anything opens it for
  writing. It rejects:
  - legacy normalized Position (`pan`, `tilt`, and their `.continuous` aliases);
  - legacy Color component programming (`color.red/green/blue/white/amber/uv/…`), in Preset
    value maps, Cue rows, Group programming, legacy scalar Dynamic lanes and the legacy `cues`
    table;
  - a marker newer than the runtime, or an unreadable marker.

  Wheel slots (`color.wheel*`), Media color (`color.tint`), Focus, Zoom and every non-family
  attribute are untouched.
- **Recovery.**
  - The message names the records and attributes, and states that the file was not changed.
  - It offers to open or create a different show.
  - The original bytes are preserved, unrelated desk settings are intact, and a separate valid
    show opens through the real show-library routes.
  - Legacy Programmer JSON is preserved byte for byte in `backups/runtime-recovery-*.json`.
  - A file the inspection cannot read falls through to the existing "corrupted or incompatible"
    recovery.
- **Activation gate.** After `build_app_state`, the activation gate uses the contract the real
  startup engaged. Synthetic `test_state()` desks (engine contract 1, no startup) keep it
  dormant until TL-552 migrates their legacy fixtures.

## Family × path matrix

Abbreviations:
- **AP** = `crates/light/src/programming`
- **HL** = `crates/light/adapters/headless/src/runtime`
- **PP** = `HL/output_scheduler/dynamic_projection/programming_projection/tests`
- **TLC** = `HL/output_scheduler/dynamic_projection/physical_adapter/color/tests_lifecycle`
- **SIS** = `crates/light/src/selective_import/tests/semantic_intent_store.rs`
- **SI** = `…/selective_import/tests/semantic_intent.rs`
- **SSI** = `…/semantic_static_import.rs`
- **STORE** = `crates/shared/show/src/tests/semantic_intent_store.rs`
- **CL** = `TLC.rs`
- **606** = `AP/cue_semantic_storage_tests.rs`
- **627** = `AP/update/semantic_storage_tests.rs`
- **START** = `HL/tests/semantic_contract_startup_tests.rs`

Status markers:
- *(stored)* means payload equality only, with no output.
- *(in progress)* means a TL-560 test written by the parallel TL-560 test work in `TLC/tl560/`; its result is reported in that handoff.
- **Gap** means no evidence yet.

| Path | Position Angles | Position Target (Point UUID) | Color semantic | Color Direct | UV | Focus | Zoom | Media color |
|---|---|---|---|---|---|---|---|---|
| Preset record | SIS `recorded_semantic_presets_reopen_with_identical_bodies_and_typed_intent` | same; `command_http_semantic_aim_tests::semantic_aim_record_stores_target_and_group_recall_keeps_live_owner` | SIS (same) | `direct_intent::recorded_direct_preset_reopens_identically_and_rerecord_is_a_no_change` | SIS (`uv_only_black`, `magenta_with_uv`) | SIS (Beam preset) | SIS | **Gap** (planned TL-560 `media_color`) |
| Preset recall | `family_ownership::complete_families_replace_independent_values_in_normal_preload_and_preset_recall_with_one_undo`; `preset_recall::color_position_and_mixed_presets_share_empty_selection_target_behavior` | `command_http_semantic_aim_tests::semantic_aim_v2_recall_and_ws_preserve_target_in_normal_blind_and_preload` | `preset_recall` (same) | `update/tests/direct_cases::preset_recall_materializes_tagged_direct_without_baking_destination_values` | Direct only: `preset_recall_plan::native_universal_spread_uses_original_source_and_predicts_each_exact_u32_and_uv`. Semantic UV: **Gap** | **Gap** | `optics_physical::a_recalled_zoom_preset_reaches_its_opening_on_differing_optics_and_leaves_focus` | **Gap** |
| Cue record (TL-606) | 606 `recorded_semantic_cue_reopens_with_exact_fixture_group_and_direct_payloads`, `merge_replaces_only_captured_rows_and_keeps_every_other_intent_exact`, `overwrite_replaces_the_cue_with_exactly_the_new_requested_intent`, `identical_overwrite_and_merge_after_reopen_are_verified_no_changes` | 606 | 606; CL `recorded_semantic_cues_survive_save_reload_fixture_replacement_and_new_live_group_members` | 606; CL `recorded_direct_cues_keep_tagged_identity_through_reload_replacement_group_and_rerecord` | 606 | 606; `command_http_optics_tests::recorded_cues_store_focus_and_zoom_as_independent_typed_changes_in_the_show` | 606; same | CL `media_layer_cues_carry_semantic_intent_through_record_rerecord_and_reload` |
| Update (TL-627) | 627 `preset_update_commits_exact_semantic_and_direct_payloads_after_reopen`, `preset_update_reopens_with_the_named_semantic_and_direct_facts`, `cue_update_modes_write_exact_intent_to_their_actual_sources_only` | 627 | 627 | 627 | 627 | 627 | 627 | **Gap** |
| Undo (Programmer and Record) | `position_intent_authoring_tests::canonical_pan_edit_adopts_actual_calibrated_fitted_pose_and_undo_restores_target`; STORE `committed_semantic_intent_reopens_backs_up_and_undoes_to_identical_typed_values` | `command_http_semantic_aim_tests::semantic_aim_commands_replace_whole_position_family_and_undo_once`; STORE | `values_actions::semantic_color_edits_adopt_each_fixture_once_and_undo_the_whole_gesture`; STORE | Import undo only (`direct_intent::import_undo_restores_the_replaced_direct_preset_exactly`). Programmer and Record undo: **Gap** (planned TL-560 `direct_undo`) | STORE | `optics_values_tests::focus_and_zoom_edit_spread_release_and_undo_as_independent_owners` | same; `optics_physical::releasing_the_zoom_effect_keeps_focus_and_undo_restores_the_same_request` | **Gap** |
| Save/reload (store) | STORE `reopened_semantic_body_is_bitwise_identical_to_the_committed_body`; SIS `semantic_cue_list_reopens_and_recompiles_through_the_show_open_reader`; 606/627 reopen | same | CL; 606/627 | CL; `direct_intent`; 606/627 | CL `uv_only_black_and_wheel_constraints_persist_and_degrade_passively_on_replacement` | `optics_replacement::group_focus_zoom_cues_survive_reopen_replacement_group_growth_and_convention_mismatch` (TL-629) | same; `programming_contract_recovery_tests::typed_focus_and_zoom_programmer_round_trips_and_production_preserves_it` | CL (media) |
| Save/reload via **real headless startup at contract 1** (TL-560) | START `contract_one_startup_loads_position_angles` | START `contract_one_startup_loads_position_point_target` | START `contract_one_startup_loads_semantic_color` | START `contract_one_startup_loads_direct_color` | START `contract_one_startup_loads_uv` | START `contract_one_startup_loads_focus_and_contract_zero_still_does` | START `contract_one_startup_loads_zoom` | START `contract_one_startup_loads_media_color` |
| Selective import | SI `semantic_presets_cues_and_dynamic_sources_survive_import_with_remapped_identities`; SSI `static_dynamic_cue_intent_follows_duplicated_identities_after_collisions`; `programming::typed_dynamic_import_remaps_point_preset_fallback_and_nested_group_member_keys` | SI `unresolved_point_target_blocks_the_whole_import`; SSI | SI; `semantic_import_undo::*` | `direct_intent::import_keeps_direct_portable_appearance_relative_output_uv_and_pinned_identity`; `programming_native::*`; TL-607 `group_direct_color::*`; TL-608 `installed_color_calibration::*` | SI; SSI | SI; SSI | SI; SSI | **Gap** |
| MVR import (patch and geometry only, by decision) | **Gap**: `mvr_import/tests` covers patch only; no semantic Cue survives an MVR profile change | **Gap** | **Gap** | **Gap** | **Gap** | **Gap** | **Gap** | **Gap** |
| Fixture replacement (output) | *(in progress)* TLC `tl560/position_replacement.rs::recorded_angles_and_point_target_survive_reopen_shipped_replacement_inversion_and_point_edits`; `position_compatibility::*` (history rejection only) | *(in progress)* same; `static_programs::actual_live_static_target_with_no_dynamics_tracks_an_independent_moving_mount` (runtime only) | CL; `color/tests::rgb_to_rgbw_replacement_with_white_seeded_decides_every_new_control`; `color_physical::fixture_replacement_recompiles_and_decides_every_new_color_control`; TL-628 `dynamic_programming::recorded_semantic_color_dynamic_keeps_intent_across_reopen_and_fixture_replacement` | CL `recorded_direct_cues_…` | CL `uv_only_black_…`; TL-611 `color_uv_widths::*` (widths and output only) | TL-629 `optics_replacement` | TL-629; `optics/tests::fixture_replacement_recompiles_and_keeps_the_stored_request` | **Gap** |
| Live Group | `position_group_authoring_tests::live_group_pan_edit_adopts_each_actual_calibrated_pose_and_retains_one_group_owner`; SI `live_group_stored_programming_remaps_nested_point_and_member_identities` | SI (same); `family_values::different_target_references_adopt_offsets_and_keep_dormant_members` | CL (new member); TL-628 `new_live_group_member_receives_the_current_semantic_dynamic_sample_without_rewrite`; START `contract_one_startup_loads_live_group_color` | TL-607 `group_direct_color::duplicate_remaps_group_stored_direct_template_exceptions_and_dormant_members` | SIS Group value *(stored)* | TL-629 | TL-629 | **Gap** |
| Unpatched fixture | 606/627 dormant row *(stored)*; *(in progress)* TLC `tl560/unpatched.rs::unpatched_members_keep_semantic_programming_and_visibility_and_only_lose_dmx_until_repatched` | 606/627 dormant row *(stored)*; behaviour **Gap** | *(in progress)* `tl560/unpatched.rs` (per fixture and live Group) | TL-607 dormant member *(stored)*; behaviour **Gap** | `engine/tests/physical_forward::unpatched_uv_only_copies_retain_native_activity_without_visible_white` (native only); semantic **Gap** | **Gap** | **Gap** | **Gap** |
| Preload GO (commit) | `preload_commit_order::go_preserves_cross_lane_order_shared_edits_and_complete_intents`; 606 `every_capture_lane_records_the_same_exact_intent_through_the_programming_service` | *(in progress)* TLC `tl560/preload_go.rs::preload_go_commits_target_direct_uv_and_zoom_unchanged_into_live_and_the_recorded_show`; output: `retained_preload_hybrid/tests/position/static_programs::pending_static_target_without_fixat_fits_both_branches_and_preserves_original_evidence` | `preload_commit_order::typed_color_release_keeps_original_component_cutoff_after_go`; `retained_preload_hybrid/tests/physical::preload_color_release_is_source_aware_and_retained_evidence_is_owned` | *(in progress)* `tl560/preload_go.rs`; output: `retained_preload_hybrid/tests/color_direct::retained_preload_branches_fade_into_a_foreign_direct_recipe_independently` | *(in progress)* `tl560/preload_go.rs` (live Group) | Output only: `retained_preload_hybrid/tests/optics::preload_focus_and_zoom_resolve_per_branch_and_release_only_the_released_owner`; GO commit **Gap** | *(in progress)* `tl560/preload_go.rs`, with `tl560/static_zoom.rs` as its static-Zoom regression | **Gap** |

These neighbouring rows are not persistence paths:
- **Cue GO playback** has evidence for Direct and UV (TL-603 `PP/color_direct_cues.rs`:
  `cue_go_fades_semantic_into_a_foreign_direct_recipe_and_back_releases_it`,
  `cue_fade_takes_uv_independently_to_zero` and others) and for Focus/Zoom
  (`optics_physical::cue_faded_focus_and_zoom_keep_their_own_times_through_independent_dynamics`).
- **TL-609** (`HL/fixture_freeze/native_transport_tests.rs`) covers native Freeze only. Its
  header says it is not semantic evidence.
- **TL-554** (`HL/native_color_pages_tests/*`, `live_state_tests/color_adoption_tests.rs`)
  covers Programmer editing of Direct Color. It assigns a single Undo group, but no test runs
  Undo.
- **TL-637** (`crates/shared/fixture/src/tests/physical_adoption.rs`) adds the shipped
  Position graphs and Zoom conventions that the in-progress Position replacement test relies
  on.

## Legacy rejection and recovery evidence (TL-560)

| Concern | Test |
| --- | --- |
| Every legacy shape is found; semantic, wheel, Media-tint, Focus and non-family values are not | `light-show` `tests::programming_contract::every_legacy_programming_shape_is_found_and_semantic_or_non_family_values_are_not` |
| Dormant at contract 0 (never opens the file) | `…::the_validator_is_dormant_at_contract_zero_and_never_opens_the_file`; START `legacy_show_still_loads_at_contract_zero_because_the_validator_is_dormant` |
| Read-only rejection, actionable message, no in-place schema migration | `…::legacy_shows_are_rejected_read_only_with_an_actionable_message_at_contract_one` |
| Newer or unreadable marker | `…::the_marker_rejects_newer_or_unreadable_contracts_and_accepts_semantic_shows`; START `malformed_or_newer_shows_are_rejected_at_contract_one_without_blocking_startup` |
| Malformed files | `…::malformed_shows_fail_inspection_instead_of_passing_silently`; START (same) |
| Writer stamps only at contract 1 and only for programming kinds | `…::writers_stamp_the_marker_only_at_contract_one_and_only_for_programming_changes`; START `the_preset_writer_stamps_the_marker_only_at_contract_one` (real `/api/v2/presets/record`) |
| Real startup rejects a legacy show; bytes and desk settings preserved; a new show opens; reopening the legacy show is refused | START `legacy_show_is_rejected_at_contract_one_keeps_bytes_and_settings_and_a_new_show_opens` |
| Legacy Programmer JSON preserved for recovery | START `legacy_programmer_is_preserved_for_recovery_at_contract_one_and_restored_at_zero` |
| The packaged default show (`assets/demo.show`) is still legacy | START `the_packaged_default_show_is_legacy_until_the_tl552_swap` (TL-552 inverts it) |
| Harness scope | START `the_startup_harness_is_scoped_to_its_call`; `e2e_semantic_contract::tests::the_startup_contract_override_is_scoped_thread_local_and_restored` |

## Storage and import detail (TL-567)

| Concern | Existing before TL-567 | Added by TL-567 |
| --- | --- | --- |
| Store keeps unknown fields, atomic transactions, exact undo/redo, backup | `light-show` `portable::tests::*` (generic JSON) | `tests::semantic_intent_store::committed_semantic_intent_reopens_backs_up_and_undoes_to_identical_typed_values` (Color/UV/zero output, Angles/Point, Focus/Zoom through commit, reopen, backup, undo) |
| Preset writer with semantic intent | `programming::preset_active_show::tests` (legacy ids, unknown fields; Normalized only) | `semantic_intent_store::recorded_semantic_presets_reopen_with_identical_bodies_and_typed_intent` (Color, Position, Beam presets; universal, per-fixture and live Group; no native keys) |
| Cue list save/load | none with typed intent | `semantic_intent_store::semantic_cue_list_reopens_and_recompiles_through_the_show_open_reader` (fixture and Group changes; explicit zero relativeOutput) |
| Import: Position Point, preset fixture/Group keys, Dynamic retained/fallback/last-valid | `selective_import::tests::programming::typed_dynamic_import_remaps_point_preset_fallback_and_nested_group_member_keys` | Not duplicated |
| Import: Color/UV/relativeOutput, Focus/Zoom and Cue Group changes after identity change | UV component source only (`numeric_typed_sources_including_random_import_live_preset_dependencies`) | `semantic_intent::semantic_presets_cues_and_dynamic_sources_survive_import_with_remapped_identities` (typed presets, installed Cue list, whole-Color Dynamic source) |
| Import: deleted live Preset keeps retained generation | `deleted_typed_preset_with_retained_value_imports_without_inventing_a_live_dependency` (Angles) | `semantic_intent::dynamic_without_its_live_preset_keeps_authored_retained_and_fallback_intent` (Color template, bounded fallback, remapped fixture/Group) |
| Import conflict resolution | `apply::apply_rewrites_duplicate_identity_and_all_imported_references` (generic); native Keep blocker | `semantic_intent::conflicting_destination_preset_keeps_or_replaces_its_complete_intent` |
| Import failure atomicity | `apply::unresolved_preview_cannot_partially_write`, `candidate_compile_failure_leaves_every_object_unwritten`, `runtime_preparation_and_commit_failures_are_atomic` (generic) | `semantic_intent::semantic_bundle_commit_and_runtime_failures_leave_the_target_unchanged`, `semantic_intent::unresolved_point_target_blocks_the_whole_import` |
| Direct native records | `programming_native::duplicate_rebinds_fix_at_recipe_wheel_and_group_member_to_complete_native_identity`, `incompatible_keep_destination_and_forged_native_digest_block_preview_and_apply` | Reused. New tests assert that semantic records never gain native recipes, estimates or wheel constraints |
| Group stored programming import (TL-570) | D1 repro only | `semantic_intent::live_group_stored_programming_remaps_nested_point_and_member_identities` now covers legacy/explicit/derived/frozen sources with collisions; `conflicting_live_group_keeps_or_replaces_its_stored_programming` and `unresolved_live_group_point_blocks_without_changing_the_destination` cover conflict/failure policy |
| Reopened numeric fidelity (TL-571) | D2 repros only | `reopened_semantic_body_is_bitwise_identical_to_the_committed_body`, `rerecording_identical_semantic_preset_after_reopen_is_a_verified_no_change`, `small_and_zero_authored_numeric_changes_still_record_after_reopen` are enabled; unchanged values keep object/show revisions, actual small/zero edits record |
| Semantic static Dynamic cue import (TL-572) | Direct masks and Dynamic references | `semantic_static_import::static_dynamic_cue_intent_follows_duplicated_identities_after_collisions`, `static_dynamic_cue_intent_binds_to_kept_destination_identities_on_replace` cover static Color/UV, Position and independent Focus/Zoom |
| Static semantic import failure (TL-572) | Generic import atomicity | `semantic_static_import::static_dynamic_cue_failures_leave_the_destination_unchanged` covers unresolved Points, runtime preparation and commit failures |
| Semantic import undo (TL-572) | Generic Macro undo | `semantic_import_undo::semantic_import_undo_restores_replaced_intent_removes_added_objects_and_keeps_unrelated_edits`, `rejected_semantic_import_undo_is_atomic` cover restoration, removal and five rejection variants |
| Retained templates after Preset edit/delete/reopen | `show_compiler::tests::dynamic_presets::*`, `dynamics::preset_sources::tests::*` | Not duplicated |

When a live Preset is imported with its Dynamic, the candidate retention stage
(`show_compiler::dynamic_presets::stage_retention`) re-derives the retained generation from the
imported Preset. The test therefore requires only remapped semantic fallback values in that case.

## Defects found by TL-567 and repaired by Opus handoffs

| ID | Original defect | Current implementation and enabled regression |
| --- | --- | --- |
| D1 | Group stored programming was omitted from dependency discovery/remapping, leaving source member and Point identities after import | TL-570 adds `/programming` traversal through the shared `ProgrammingReferences`; the enabled Group matrix and conflict/failure tests above cover it |
| D2 | Default JSON floating-point parsing changed some stored `f64` values by one ULP, producing a false changed Preset recording after reopen | TL-571 enables workspace `serde_json/float_roundtrip`; exact stored-body and real no-change/small-change/zero-value recording tests above cover it without epsilon equality |

Run these regressions with `cargo test -p light-application --lib semantic_` and
`cargo test -p light-show --lib semantic_intent`. TL-570/571/572 are in Review / test;
their implementation evidence is distinct from final TL-560 or human acceptance.

## Remaining gaps for TL-552

- **Media color:**
  - Preset record and recall, Update, undo;
  - selective and MVR import, fixture replacement;
  - live Group, unpatched, Preload GO.
  - Only Cue record, save/reload and real startup are covered.
- **Color Direct:** Programmer undo and Record undo.
- **Preset recall:** semantic UV and Focus.
- **MVR re-import of a GDTF profile change under a semantic Cue:** every column.
- **Unpatched behaviour (not just stored rows):** Target, Direct, UV, Focus and Zoom.
- **Preload GO commit:** Focus and Media.
- **Contract coverage beyond the portable show:**
  - Playback and Output runtime payloads are gated by the existing content-derived requirement
    only. They have no legacy-key validator.
  - Undo history (`object_history`) is not inspected.
- **Legacy Zoom:** a percentage `zoom` is not rejected (owner scope). It is listed as a cutover
  decision in the engineering checklist.

Run the regressions with:
- `cargo test -p light-application --lib semantic_`;
- `cargo test -p light-show --lib semantic_intent`;
- `cargo test -p light-show --lib programming_contract`;
- `LIGHT_TMP_DIR=$PWD/.artifacts/tmp cargo test -p light-headless-runtime --lib semantic_contract_startup`.
