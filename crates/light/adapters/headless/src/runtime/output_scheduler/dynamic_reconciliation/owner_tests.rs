//! TL-585: surviving Playback controllers follow their current owner, priority and
//! page-qualified release assignment without restarting or resetting retained history.
use super::tests::{group, live_group_dynamic, positions};
use super::*;
use light_dynamics::{
    DynamicControlCursor, DynamicControllerSource, DynamicDefinition, DynamicRuntime,
    DynamicRuntimeSnapshot, replay_dynamic_controls,
};
use std::num::NonZeroUsize;
use std::sync::Arc;

fn dynamic_playback(
    number: u16,
    definition: &DynamicDefinition,
    priority: i16,
    xfade_millis: u64,
) -> light_playback::PlaybackDefinition {
    serde_json::from_value(serde_json::json!({
        "number": number, "name": format!("Dynamic {number}"), "xfade_millis": xfade_millis,
        "target": {"type": "dynamic", "assignment": {
            "priority": priority,
            "dynamic": {"dynamic_id": definition.id, "last_known_pool_number": definition.pool_number,
                "embedded_fallback": {"definition": definition}}
        }}
    }))
    .unwrap()
}

/// A same-numbered unrelated assignment. A number-only release lookup would pick its xfade.
fn decoy_playback(number: u16, xfade_millis: u64) -> light_playback::PlaybackDefinition {
    serde_json::from_value(serde_json::json!({
        "number": number, "name": "Decoy", "xfade_millis": xfade_millis,
        "target": {"type": "cue_list", "cue_list_id": Uuid::new_v4()}
    }))
    .unwrap()
}

fn page(
    number: u8,
    playbacks: Vec<light_playback::PlaybackDefinition>,
) -> light_playback::PlaybackPage {
    light_playback::PlaybackPage {
        number,
        name: format!("Page {number}"),
        slots: HashMap::new(),
        virtual_playbacks: playbacks
            .into_iter()
            .map(|playback| (playback.number, playback))
            .collect(),
    }
}

fn row(
    definition: &DynamicDefinition,
    identity: PlaybackIdentity,
) -> light_playback::ActiveDynamicPlayback {
    let mut row: light_playback::ActiveDynamicPlayback =
        serde_json::from_value(serde_json::json!({
            "dynamic_id": definition.id, "playback_number": identity.number(), "enabled": true,
            "paused": false, "activated_at": chrono::DateTime::from_timestamp_millis(10).unwrap()
        }))
        .unwrap();
    row.playback_identity = Some(identity);
    row
}

fn setup() -> (
    DynamicDefinition,
    light_engine::EngineSnapshot,
    DynamicRuntime,
) {
    let definition = live_group_dynamic();
    let fixture = FixtureId::new();
    let snapshot = light_engine::EngineSnapshot {
        dynamics: vec![definition.clone()].into(),
        groups: vec![group(&[fixture])].into(),
        dynamic_stage_positions: Arc::new(positions([(fixture, 0.)])),
        ..Default::default()
    };
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    (definition, snapshot, runtime)
}

fn owner_of(
    runtime: &DynamicRuntime,
    definition: &DynamicDefinition,
) -> (DynamicControllerSource, i16) {
    let (_, controller) = runtime
        .controller(light_playback::dynamic_playback_controller_id(
            definition.id,
        ))
        .unwrap();
    (controller.source, controller.priority)
}

fn without_owner_metadata(mut snapshot: DynamicRuntimeSnapshot) -> DynamicRuntimeSnapshot {
    for controller in snapshot
        .instances
        .iter_mut()
        .flat_map(|instance| &mut instance.controllers)
    {
        controller.source = DynamicControllerSource::physical_playback(0);
        controller.priority = 0;
    }
    snapshot
}

fn release_duration(runtime: &DynamicRuntime) -> u64 {
    let snapshot = runtime.snapshot();
    let transition = &snapshot.instances[0].controller_transitions[0];
    assert!(transition.release_started_at_millis.is_some());
    transition.release_duration_millis
}

fn replay_since(
    live: &DynamicRuntime,
    pending: &mut DynamicRuntime,
    cursor: &mut DynamicControlCursor,
) {
    let batch = live.controls_since(*cursor).unwrap().unwrap();
    let anchor = pending.committed_sample_boundary();
    replay_dynamic_controls(pending, &mut Default::default(), cursor, &batch, anchor).unwrap();
    assert_eq!(pending.snapshot(), live.snapshot());
}

fn labels(source: &DynamicControllerSource) -> (String, String) {
    (
        crate::runtime::dynamics_http::dynamic_source_label(source),
        crate::runtime::osc_feedback_programmer::osc_dynamic_source(source),
    )
}

#[test]
fn moved_physical_playback_refreshes_owner_priority_labels_and_release_in_retained_history() {
    let (definition, mut snapshot, mut live) = setup();
    snapshot.playbacks = vec![dynamic_playback(1, &definition, 2, 100)].into();
    let mut rows = vec![row(&definition, PlaybackIdentity::physical(1).unwrap())];
    let mut cursor = live.begin_control_recording(NonZeroUsize::new(64).unwrap());
    let mut pending = live.fork_for_pending_preview();
    reconcile_dynamic_playbacks(&mut live, 100, &snapshot, &rows);
    rows[0].paused = true;
    reconcile_dynamic_playbacks(&mut live, 150, &snapshot, &rows);
    replay_since(&live, &mut pending, &mut cursor);
    let before = live.snapshot();
    assert_eq!(
        owner_of(&live, &definition),
        (DynamicControllerSource::physical_playback(1), 2)
    );
    assert_eq!(before.instances[0].paused_at_millis, Some(150));

    // The assignment moves from Playback 1 to Playback 5 and gains priority 7; an unrelated
    // assignment now occupies Playback 1 with a different xfade.
    snapshot.playbacks = vec![
        decoy_playback(1, 40),
        dynamic_playback(5, &definition, 7, 900),
    ]
    .into();
    rows[0] = row(&definition, PlaybackIdentity::physical(5).unwrap());
    rows[0].paused = true;
    reconcile_dynamic_playbacks(&mut live, 200, &snapshot, &rows);
    let moved = live.snapshot();
    assert_eq!(
        owner_of(&live, &definition),
        (DynamicControllerSource::physical_playback(5), 7)
    );
    assert_eq!(moved.instances[0].id, before.instances[0].id);
    assert_eq!(moved.instances[0].started_at_millis, 10);
    assert_eq!(moved.instances[0].paused_at_millis, Some(150));
    assert_eq!(
        without_owner_metadata(moved),
        without_owner_metadata(before)
    );
    let owner = owner_of(&live, &definition).0;
    assert_eq!(
        labels(&owner),
        ("Playback 5".to_owned(), "playback:5".to_owned())
    );
    replay_since(&live, &mut pending, &mut cursor);
    let unchanged = live.control_cursor();
    reconcile_dynamic_playbacks(&mut live, 210, &snapshot, &rows);
    assert_eq!(live.control_cursor(), unchanged, "refresh is recorded once");

    rows.clear();
    reconcile_dynamic_playbacks(&mut live, 300, &snapshot, &rows);
    assert_eq!(release_duration(&live), 900, "current assignment xfade");
    replay_since(&live, &mut pending, &mut cursor);
    assert_eq!(release_duration(&pending), 900);
}

#[test]
fn virtual_owner_release_is_page_qualified_when_numbers_collide() {
    let (definition, mut snapshot, mut live) = setup();
    // Unvalidated/legacy rows share number 1301 across physical, page 1 and page 2.
    snapshot.playbacks = vec![decoy_playback(1301, 40)].into();
    snapshot.playback_pages = vec![
        page(1, vec![decoy_playback(1301, 60)]),
        page(2, vec![dynamic_playback(1301, &definition, 3, 750)]),
    ]
    .into();
    let mut rows = vec![row(
        &definition,
        PlaybackIdentity::virtual_playback(2, 1301).unwrap(),
    )];
    reconcile_dynamic_playbacks(&mut live, 100, &snapshot, &rows);
    let owner = owner_of(&live, &definition);
    assert_eq!(
        owner,
        (DynamicControllerSource::virtual_playback(2, 1301), 3)
    );
    assert_eq!(
        labels(&owner.0),
        (
            "Virtual Playback 1301 (page 2)".to_owned(),
            "virtual-playback:2:1301".to_owned()
        )
    );
    let mut released = live.fork_for_pending_preview();
    reconcile_dynamic_playbacks(&mut released, 200, &snapshot, &[]);
    assert_eq!(release_duration(&released), 750);

    // Moving to page 3 refreshes the page-qualified owner; release follows the new page.
    let before = live.snapshot();
    snapshot.playback_pages = vec![
        page(2, vec![decoy_playback(1301, 60)]),
        page(3, vec![dynamic_playback(1601, &definition, -4, 320)]),
    ]
    .into();
    rows[0] = row(
        &definition,
        PlaybackIdentity::virtual_playback(3, 1601).unwrap(),
    );
    reconcile_dynamic_playbacks(&mut live, 300, &snapshot, &rows);
    assert_eq!(
        owner_of(&live, &definition),
        (DynamicControllerSource::virtual_playback(3, 1601), -4)
    );
    assert_eq!(
        without_owner_metadata(live.snapshot()),
        without_owner_metadata(before)
    );
    reconcile_dynamic_playbacks(&mut live, 400, &snapshot, &[]);
    assert_eq!(release_duration(&live), 320);
}

#[test]
fn missing_or_deleted_assignment_releases_immediately_and_legacy_owner_uses_its_bank() {
    let (definition, mut snapshot, mut live) = setup();
    snapshot.playback_pages =
        vec![page(2, vec![dynamic_playback(1301, &definition, 1, 500)])].into();
    let rows = [row(
        &definition,
        PlaybackIdentity::virtual_playback(2, 1301).unwrap(),
    )];
    reconcile_dynamic_playbacks(&mut live, 100, &snapshot, &rows);
    let controller = light_playback::dynamic_playback_controller_id(definition.id);

    // A checkpoint written before page qualification carries only the Virtual number.
    let mut legacy = live.fork_for_pending_preview();
    legacy
        .update_controller_owner(
            controller,
            DynamicControllerSource::physical_playback(1301),
            1,
        )
        .unwrap();
    reconcile_dynamic_playbacks(&mut legacy, 200, &snapshot, &[]);
    assert_eq!(release_duration(&legacy), 500);

    // The deleted assignment follows the explicit immediate-release fallback.
    snapshot.playback_pages = vec![page(2, Vec::new())].into();
    reconcile_dynamic_playbacks(&mut live, 200, &snapshot, &[]);
    assert_eq!(live.instance_count(), 0);
}

#[test]
fn cold_reconciliation_records_the_same_owner_refresh_as_warm_and_failure_keeps_live() {
    let (definition, mut snapshot, mut live) = setup();
    snapshot.playbacks = vec![dynamic_playback(1, &definition, 2, 100)].into();
    let rows = vec![row(&definition, PlaybackIdentity::physical(1).unwrap())];
    reconcile_dynamic_playbacks(&mut live, 100, &snapshot, &rows);
    let cursor = live.begin_control_recording(NonZeroUsize::new(64).unwrap());
    let live_before = live.snapshot();

    let mut destination = snapshot.clone();
    destination.playbacks = vec![dynamic_playback(5, &definition, 7, 900)].into();
    let moved = vec![row(&definition, PlaybackIdentity::physical(5).unwrap())];
    let inputs = |playbacks| ColdDynamicReconciliationInputs {
        captured_at_millis: 200,
        snapshot: &destination,
        programmer_values: &[],
        extra_programmer_values: &[],
        cue_values: &[],
        playbacks,
        playback_paused: false,
    };

    let mut warm = live.fork_for_cold_install();
    reconcile_dynamic_playbacks(&mut warm, 200, &destination, &moved);
    let mut cold = live.fork_for_cold_install();
    reconcile_cold_dynamic_candidate(&mut cold, inputs(&moved)).unwrap();
    assert_eq!(cold.snapshot(), warm.snapshot());
    assert_eq!(
        owner_of(&cold, &definition),
        (DynamicControllerSource::physical_playback(5), 7)
    );
    for candidate in [&warm, &cold] {
        let mut retained = live.fork_for_pending_preview();
        let mut replayed = cursor;
        replay_since(candidate, &mut retained, &mut replayed);
        assert_eq!(
            owner_of(&retained, &definition),
            (DynamicControllerSource::physical_playback(5), 7)
        );
    }

    // A destination with an unresolvable enabled row rejects the private candidate as a whole.
    let mut failing = moved.clone();
    failing.push(row(&definition, PlaybackIdentity::physical(9).unwrap()));
    failing[1].dynamic_id = Some(Uuid::new_v4());
    let mut rejected = live.fork_for_cold_install();
    let error = reconcile_cold_dynamic_candidate(&mut rejected, inputs(&failing)).unwrap_err();
    assert!(!error.failures.is_empty());
    assert_eq!(live.snapshot(), live_before);
    assert_eq!(
        owner_of(&live, &definition),
        (DynamicControllerSource::physical_playback(1), 2)
    );
    assert!(live.controls_since(cursor).unwrap().unwrap().is_empty());
}
