use super::super::{
    compile_show_candidate, prepare_normalized_show_candidate_incremental, prepare_show_candidate,
};
use super::support::{document_with_objects, snapshot_without_revision};
use light_core::CueListId;
use light_playback::{Cue, CueList, CueListMode, IntensityPriorityMode, RestartMode, WrapMode};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

fn normalized_document() -> (light_show::ShowStore, light_show::PortableShowDocument) {
    let cue = CueList {
        id: CueListId::new(),
        name: "Main".into(),
        priority: 0,
        mode: CueListMode::Sequence,
        looped: false,
        chaser_step_millis: 1_000,
        speed_group: None,
        intensity_priority_mode: IntensityPriorityMode::Htp,
        wrap_mode: Some(WrapMode::Tracking),
        restart_mode: RestartMode::FirstCue,
        force_cue_timing: false,
        disable_cue_timing: false,
        auto_off_at_zero: false,
        auto_off_flash_release: false,
        chaser_xfade_millis: 0,
        chaser_xfade_percent: None,
        speed_multiplier: 1.0,
        cues: vec![Cue::new(
            crate::CueNumber::try_from_legacy_f64(1.0).unwrap(),
        )],
    };
    let (store, document) = document_with_objects(&[
        ("cue_list", "main", serde_json::to_value(cue).unwrap()),
        (
            "group",
            "front",
            json!({"id": "front", "name": "Front", "fixtures": []}),
        ),
        (
            "preset",
            "1.1",
            json!({"number": 1, "decimal": 1, "name": "Dim", "family": "Intensity", "values": {}}),
        ),
    ]);
    let transaction = prepare_show_candidate(&document, document.transaction())
        .unwrap()
        .into_parts()
        .0;
    if !transaction.is_empty() {
        store.apply_portable_transaction(transaction).unwrap();
    }
    let document = store.portable_document().unwrap();
    (store, document)
}

#[test]
fn cue_edit_rebuilds_only_the_playback_subgraph() {
    let (_store, document) = normalized_document();
    let previous = prepare_show_candidate(&document, document.transaction())
        .unwrap()
        .into_parts()
        .1;
    let mut cue = document.object("cue_list", "main").unwrap().body().clone();
    cue["name"] = json!("Updated");
    let mut transaction = document.transaction();
    transaction.put("cue_list", "main", cue);

    let incremental =
        prepare_normalized_show_candidate_incremental(&document, transaction.clone(), &previous)
            .unwrap()
            .into_parts()
            .1;
    let full = compile_show_candidate(document.candidate(&transaction).unwrap()).unwrap();

    assert_eq!(
        snapshot_without_revision(incremental.clone()),
        snapshot_without_revision(full)
    );
    assert!(Arc::ptr_eq(&incremental.fixtures, &previous.fixtures));
    assert!(Arc::ptr_eq(&incremental.groups, &previous.groups));
    assert!(Arc::ptr_eq(&incremental.routes, &previous.routes));
    assert!(Arc::ptr_eq(
        &incremental.control_mappings,
        &previous.control_mappings
    ));
    assert!(!Arc::ptr_eq(&incremental.cue_lists, &previous.cue_lists));
}

#[test]
fn preset_edit_reuses_every_runtime_projection() {
    let (_store, document) = normalized_document();
    let previous = prepare_show_candidate(&document, document.transaction())
        .unwrap()
        .into_parts()
        .1;
    let mut preset = document.object("preset", "1.1").unwrap().body().clone();
    preset["name"] = json!("Updated");
    let mut transaction = document.transaction();
    transaction.put("preset", "1.1", preset);
    let next = prepare_normalized_show_candidate_incremental(&document, transaction, &previous)
        .unwrap()
        .into_parts()
        .1;

    assert!(Arc::ptr_eq(&next.fixtures, &previous.fixtures));
    assert!(Arc::ptr_eq(&next.cue_lists, &previous.cue_lists));
    assert!(Arc::ptr_eq(&next.playbacks, &previous.playbacks));
    assert!(Arc::ptr_eq(&next.playback_pages, &previous.playback_pages));
    assert!(Arc::ptr_eq(&next.routes, &previous.routes));
    assert!(Arc::ptr_eq(
        &next.control_mappings,
        &previous.control_mappings
    ));
    assert!(Arc::ptr_eq(&next.groups, &previous.groups));
    assert_eq!(next.revision, previous.revision + 1);
}

#[test]
fn semantic_preset_requirement_reaches_engine_activation_and_incremental_deletion() {
    use light_core::{
        AttributeKey, AttributeValue,
        programming::{ColorIntent, ColorProgram},
    };
    let (store, document) = normalized_document();
    let previous = prepare_show_candidate(&document, document.transaction())
        .unwrap()
        .into_parts()
        .1;
    let preset = light_programmer::Preset {
        family: light_programmer::PresetFamily::Color,
        number: 1,
        universal_values: [(
            AttributeKey::color(),
            AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
                intent: ColorIntent::default(),
            })),
        )]
        .into(),
        ..Default::default()
    };
    let mut transaction = document.transaction();
    transaction.put("preset", "2.1", serde_json::to_value(preset).unwrap());
    let next =
        prepare_normalized_show_candidate_incremental(&document, transaction.clone(), &previous)
            .unwrap()
            .into_parts()
            .1;
    assert_eq!(next.required_programming_contract(), 1);
    assert_eq!(
        compile_show_candidate(document.candidate(&transaction).unwrap())
            .unwrap()
            .required_programming_contract(),
        1
    );
    let legacy = light_engine::Engine::with_programming_contract_support(
        light_programmer::ProgrammerRegistry::default(),
        0,
    );
    legacy.replace_snapshot(previous).unwrap();
    assert!(legacy.prepare_snapshot(next.clone()).is_err());
    assert!(
        light_engine::Engine::new(light_programmer::ProgrammerRegistry::default())
            .prepare_snapshot(next.clone())
            .is_ok()
    );
    store.apply_portable_transaction(transaction).unwrap();
    let updated = store.portable_document().unwrap();
    let mut deletion = updated.transaction();
    deletion.delete("preset", "2.1");
    let restored = prepare_normalized_show_candidate_incremental(&updated, deletion, &next)
        .unwrap()
        .into_parts()
        .1;
    assert_eq!(restored.required_programming_contract(), 0);
    assert!(legacy.prepare_snapshot(restored).is_ok());
}

#[test]
fn touched_legacy_object_is_normalized_without_sweeping_unrelated_objects() {
    let (_store, document) = normalized_document();
    let previous = prepare_show_candidate(&document, document.transaction())
        .unwrap()
        .into_parts()
        .1;
    let mut cue = document.object("cue_list", "main").unwrap().body().clone();
    let cue = cue.as_object_mut().unwrap();
    cue.remove("chaser_xfade_percent");
    cue.insert("chaser_xfade_millis".into(), json!(250));
    for field in [
        "intensity_priority_mode",
        "wrap_mode",
        "restart_mode",
        "force_cue_timing",
        "disable_cue_timing",
        "speed_multiplier",
    ] {
        cue.remove(field);
    }
    let mut transaction = document.transaction();
    transaction.put("cue_list", "main", Value::Object(cue.clone()));

    let prepared =
        prepare_normalized_show_candidate_incremental(&document, transaction, &previous).unwrap();
    let (transaction, next) = prepared.into_parts();
    let candidate = document.candidate(&transaction).unwrap();
    let migrated = candidate.object("cue_list", "main").unwrap().body();

    assert_eq!(migrated["chaser_xfade_percent"], 25);
    assert!(migrated.get("chaser_xfade_millis").is_none());
    assert_eq!(migrated["restart_mode"], "first_cue");
    assert_eq!(migrated["intensity_priority_mode"], "htp");
    assert!(Arc::ptr_eq(&next.groups, &previous.groups));
    assert!(Arc::ptr_eq(&next.fixtures, &previous.fixtures));
}

#[test]
fn touched_legacy_dynamic_writes_through_its_inferred_spatial_mapping() {
    let dynamic = light_dynamics::DynamicDefinition {
        id: Uuid::new_v4(),
        pool_number: 7,
        revision: 1,
        name: "Touched legacy Dynamic".into(),
        color: None,
        icon: None,
        target_binding: light_dynamics::DynamicTargetBinding::Targetless,
        lanes: Vec::new(),
        random_groups: Vec::new(),
        phase_spread_mode: light_dynamics::DynamicPhaseSpreadMode::Uniform,
        spatial_mapping: light_dynamics::DynamicSpatialMappingOverride::default(),
        phase: light_dynamics::PhaseDistribution {
            ordering: light_dynamics::PhaseOrdering::Selection,
            offset_degrees: 0.0,
            span_degrees: 360.0,
            block_size: 1,
            repeats: 1,
            wings: false,
            anchors_degrees: Vec::new(),
        },
        speed: light_dynamics::DynamicSpeed::Fixed {
            duration_millis: 1_000,
        },
        overall_speed_multiplier: light_dynamics::Rational::ONE,
        run_mode: light_dynamics::DynamicRunMode::Loop,
        default_activation: light_dynamics::ActivationPolicy::StartNow,
        activation_boundary: light_dynamics::ActivationBoundary::Beat,
    };
    let (_, document) =
        document_with_objects(&[("dynamic", "7", serde_json::to_value(&dynamic).unwrap())]);
    let previous = prepare_show_candidate(&document, document.transaction())
        .unwrap()
        .into_parts()
        .1;
    let mut legacy = serde_json::to_value(dynamic).unwrap();
    legacy["phase"]["ordering"] = json!({"type": "grid_linear", "angle_degrees": 30.0});
    legacy.as_object_mut().unwrap().remove("spatial_mapping");
    legacy["future_dynamic"] = json!({"preserved": true});
    let mut transaction = document.transaction();
    transaction.put("dynamic", "7", legacy);

    let prepared =
        prepare_normalized_show_candidate_incremental(&document, transaction, &previous).unwrap();
    let (transaction, _) = prepared.into_parts();
    let candidate = document.candidate(&transaction).unwrap();
    let migrated = candidate.object("dynamic", "7").unwrap().body();

    assert_eq!(
        migrated["spatial_mapping"]["shape"]["value"]["type"],
        "grid"
    );
    assert_eq!(migrated["phase"]["ordering"]["type"], "grid_linear");
    assert_eq!(migrated["future_dynamic"], json!({"preserved": true}));
}

/// Characterizes the kind matrix in `docs/engineering/show-sync.md`: every object kind an
/// Architect synchronizes is invisible to the compiled runtime, so a sync commit of those kinds
/// installs the live runtime unchanged instead of recompiling the show.
#[test]
fn synchronized_architect_kinds_share_every_compiled_projection() {
    let (_store, document) = normalized_document();
    let previous = prepare_show_candidate(&document, document.transaction())
        .unwrap()
        .into_parts()
        .1;
    let mut transaction = document.transaction();
    for kind in crate::show_sync::SHOW_SYNC_OBJECT_KINDS {
        let body = if *kind == "patch_layer" {
            json!({"id": "sync", "name": "Synced", "order": 1})
        } else {
            json!({"name": format!("synced {kind}")})
        };
        transaction.put(*kind, "sync", body);
    }
    transaction.set_metadata("architect.venue", "Hall A");
    let next = prepare_normalized_show_candidate_incremental(&document, transaction, &previous)
        .unwrap()
        .into_parts()
        .1;
    assert!(Arc::ptr_eq(&next.fixtures, &previous.fixtures));
    assert!(Arc::ptr_eq(&next.cue_lists, &previous.cue_lists));
    assert!(Arc::ptr_eq(&next.dynamics, &previous.dynamics));
    assert!(Arc::ptr_eq(&next.playbacks, &previous.playbacks));
    assert!(Arc::ptr_eq(&next.playback_pages, &previous.playback_pages));
    assert!(Arc::ptr_eq(&next.routes, &previous.routes));
    assert!(Arc::ptr_eq(
        &next.control_mappings,
        &previous.control_mappings
    ));
    assert!(Arc::ptr_eq(&next.groups, &previous.groups));
    assert_eq!(next.revision, previous.revision + 1);
}

/// The counterpart: desk kinds Control compiles are rebuilt by the same incremental path, so a
/// sync of them would recompile — which is why they are not synchronized object kinds.
#[test]
fn compiled_desk_kinds_rebuild_their_projection() {
    let (_store, document) = normalized_document();
    let previous = prepare_show_candidate(&document, document.transaction())
        .unwrap()
        .into_parts()
        .1;
    let mut group = document.object("group", "front").unwrap().body().clone();
    group["name"] = json!("Front wash");
    let mut transaction = document.transaction();
    transaction.put("group", "front", group);
    let next = prepare_normalized_show_candidate_incremental(&document, transaction, &previous)
        .unwrap()
        .into_parts()
        .1;
    assert!(!Arc::ptr_eq(&next.groups, &previous.groups));
    assert!(Arc::ptr_eq(&next.cue_lists, &previous.cue_lists));
}

#[test]
fn live_preset_edit_recompiles_active_cue_without_restarting_or_changing_literals() {
    use light_core::{AttributeKey, AttributeValue, PresetValueOwner, PresetValueReference};
    use light_engine::{CueListPlaybackAction, Engine, EnginePlaybackCommand};
    use light_playback::CueChange;
    let (store, mut document) = normalized_document();
    let (_, mut fixture, _) = super::support::portable_fixture();
    for (logical, head) in fixture
        .logical_heads
        .iter_mut()
        .zip(fixture.definition.heads.iter().filter(|head| !head.shared))
    {
        logical.head_index = head.index;
    }
    fixture.logical_heads.truncate(
        fixture
            .definition
            .heads
            .iter()
            .filter(|head| !head.shared)
            .count(),
    );
    let target = fixture.logical_heads[0].fixture_id;
    let attribute = AttributeKey::intensity();
    let mut preset: light_programmer::Preset =
        serde_json::from_value(document.object("preset", "1.1").unwrap().body().clone()).unwrap();
    let identity = preset.instance_id.unwrap();
    preset.values.insert(
        target,
        std::collections::HashMap::from([(attribute.clone(), AttributeValue::Normalized(0.75))]),
    );
    let mut list: CueList =
        serde_json::from_value(document.object("cue_list", "main").unwrap().body().clone())
            .unwrap();
    let list_id = list.id;
    let mut linked = CueChange::set(target, attribute.clone(), AttributeValue::Normalized(0.75));
    linked.preset_reference = Some(PresetValueReference {
        preset_instance_id: identity,
        source_owner: PresetValueOwner::Fixture { fixture_id: target },
        source_attribute: attribute.clone(),
        sample_rank: None,
        member_fixture: Some(target),
    });
    list.cues[0].changes.push(linked);
    list.cues[0].changes.push(CueChange::set(
        target,
        AttributeKey("beam.focus".into()),
        AttributeValue::Normalized(0.4),
    ));
    let mut seed = document.transaction();
    seed.put("preset", "1.1", serde_json::to_value(&preset).unwrap());
    seed.put("cue_list", "main", serde_json::to_value(list).unwrap());
    let (seed, mut previous) = prepare_show_candidate(&document, seed)
        .unwrap()
        .into_parts();
    let commit = store.apply_portable_transaction(seed).unwrap();
    document.apply_commit(&commit);
    previous.fixtures = vec![fixture].into();
    let engine = Engine::new(light_programmer::ProgrammerRegistry::default());
    engine.replace_snapshot(previous.clone()).unwrap();
    engine
        .execute_playback(EnginePlaybackCommand::CueList {
            id: list_id,
            action: CueListPlaybackAction::GoAt(chrono::Utc::now() - chrono::Duration::seconds(1)),
        })
        .unwrap();
    let active = engine.active_playbacks();
    assert_eq!(active.len(), 1);
    preset
        .values
        .get_mut(&target)
        .unwrap()
        .insert(attribute.clone(), AttributeValue::Normalized(0.25));
    let mut edit = document.transaction();
    edit.put("preset", "1.1", serde_json::to_value(preset).unwrap());
    let (_, next) = prepare_normalized_show_candidate_incremental(&document, edit, &previous)
        .unwrap()
        .into_parts();
    assert!(!Arc::ptr_eq(&previous.cue_lists, &next.cue_lists));
    engine.replace_snapshot(next).unwrap();
    assert_eq!(engine.active_playbacks().len(), 1);
    let contributions = engine.playback_contributions_at(chrono::Utc::now());
    assert!(
        contributions
            .iter()
            .any(|row| row.value.fixture_id == target
                && row.value.attribute == attribute
                && row.value.value == AttributeValue::Normalized(0.25))
    );
    assert!(
        contributions
            .iter()
            .any(|row| row.value.fixture_id == target
                && row.value.attribute.0.as_ref() == "beam.focus"
                && row.value.value == AttributeValue::Normalized(0.4))
    );
}
