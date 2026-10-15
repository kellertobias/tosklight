use super::*;

fn desk() -> (AppState, Session, PathBuf, light_core::FixtureId) {
    let (state, data_dir) = test_state();
    state.installation.update_configuration(|configuration| {
        configuration.command_line_at_uses_programmer_fade = false;
    });
    let session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        token: "pool-tap".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programming.start(session.id);
    state.sessions.insert_session(session.clone());
    let fixture = light_core::FixtureId::new();
    let mut snapshot = light_engine::EngineSnapshot::default();
    snapshot.fixtures = vec![operational_fixture(fixture)].into();
    snapshot.dynamics = vec![command_test_dynamic(Uuid::new_v4(), 1)].into();
    state.output.replace_snapshot(snapshot).unwrap();
    let show = state
        .installation
        .upsert_show(
            "Pool tap",
            &data_dir.join("shows/pool-tap.show").display().to_string(),
            false,
        )
        .unwrap();
    state.active_show.replace_current(Some(show));
    state.programming.select(session.id, [fixture]);
    (state, session, data_dir, fixture)
}

fn execute(
    state: &AppState,
    session: &Session,
    request: &str,
    value: &str,
    pool: Option<u16>,
) -> bool {
    let mut action = serde_json::json!({"type":"command_line_execute", "request":{"value":value}});
    if let Some(number) = pool {
        action["request"]["pool_tap"] = serde_json::json!({"type":"dynamic", "pool_number":number});
    }
    let action = serde_json::from_value(action).unwrap();
    dispatch_live_action(state, session, live_action_frame(session, request, action)).ok
}

#[test]
fn dynamic_pool_tap_toggles_but_plain_typed_dynamic_remains_idempotent() {
    let (state, session, data_dir, fixture) = desk();
    assert!(execute(&state, &session, "typed-1", "DYNAMIC 1", None));
    let first = state.output.dynamic_runtime_snapshot();
    assert_eq!(first.instances.len(), 1);
    assert_eq!(first.instances[0].targets, vec![fixture]);
    let controller = first.instances[0].controllers[0].id;
    let initial_programmer = state.programming.get(session.id).unwrap();
    assert_eq!(initial_programmer.dynamic_values.len(), 1);
    let light_dynamics::DynamicSemanticValue::DynamicOn {
        instance_link: authored_link,
        ..
    } = initial_programmer.dynamic_values[0].value
    else {
        panic!("typed start must author one DynamicOn");
    };
    assert_eq!(
        controller,
        light_dynamics::programmer_dynamic_controller_id(initial_programmer.id, authored_link)
    );
    assert_ne!(
        controller, authored_link,
        "authored link and runtime controller are separate identities"
    );

    assert!(execute(&state, &session, "typed-2", "DYNAMIC 1", None));
    let repeated = state.output.dynamic_runtime_snapshot();
    assert_eq!(repeated.instances.len(), 1);
    assert_eq!(repeated.instances[0].id, first.instances[0].id);
    assert_eq!(repeated.instances[0].controllers[0].id, controller);
    assert!(execute(&state, &session, "pool-off", "DYNAMIC 1", Some(1)));
    let programmer = state.programming.get(session.id).unwrap();
    assert_eq!(programmer.dynamic_values.len(), 1);
    assert_eq!(programmer.dynamic_values[0].fixture_id, fixture);
    assert!(
        matches!(programmer.dynamic_values[0].value, light_dynamics::DynamicSemanticValue::DynamicOff { instance_link, .. } if instance_link == authored_link)
    );
    assert_eq!(state.output.dynamic_runtime_snapshot().instances.len(), 0);
    assert!(execute(&state, &session, "pool-on", "DYNAMIC 1", Some(1)));
    let programmer = state.programming.get(session.id).unwrap();
    assert_eq!(
        programmer.dynamic_values.len(),
        2,
        "Off lineage remains authored beside a fresh On"
    );
    assert!(
        programmer
            .dynamic_values
            .iter()
            .all(|value| value.fixture_id == fixture)
    );
    let old = programmer.dynamic_values.iter().filter(|value| matches!(value.value, light_dynamics::DynamicSemanticValue::DynamicOff { instance_link, .. } if instance_link == authored_link)).count();
    let fresh = programmer.dynamic_values.iter().filter(|value| matches!(&value.value, light_dynamics::DynamicSemanticValue::DynamicOn { instance_link, dynamic, .. } if *instance_link != authored_link && dynamic.dynamic_id == Some(first.instances[0].definition.id))).count();
    assert_eq!((old, fresh), (1, 1));
    let started = state.output.dynamic_runtime_snapshot();
    assert_eq!(started.instances.len(), 1);
    assert_eq!(started.instances[0].targets, vec![fixture]);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn dynamic_pool_tap_rejects_mismatched_provenance_before_programming() {
    let (state, session, data_dir, _) = desk();
    let before = serde_json::to_value(state.programming.get(session.id).unwrap()).unwrap();
    assert!(!execute(
        &state,
        &session,
        "wrong-pool",
        "DYNAMIC 1",
        Some(2)
    ));
    assert!(
        state
            .programming
            .get(session.id)
            .unwrap()
            .dynamic_values
            .is_empty()
    );
    assert_eq!(state.output.dynamic_runtime_snapshot().instances.len(), 0);
    // Shared command feedback may change, but fixture values and undo must not.
    assert_eq!(
        before["dynamic_values"],
        serde_json::to_value(state.programming.get(session.id).unwrap()).unwrap()["dynamic_values"]
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn dynamic_pool_tap_replay_origin_is_distinct_and_repeated_tap_is_not_resent() {
    let (state, session, data_dir, _) = desk();
    assert!(execute(
        &state,
        &session,
        "same-envelope",
        "DYNAMIC 1",
        None
    ));
    assert!(
        !execute(&state, &session, "same-envelope", "DYNAMIC 1", Some(1)),
        "one request ID cannot change typed intent into pool intent"
    );
    assert_eq!(state.output.dynamic_runtime_snapshot().instances.len(), 1);
    assert!(execute(&state, &session, "real-pool", "DYNAMIC 1", Some(1)));
    assert!(execute(&state, &session, "real-pool", "DYNAMIC 1", Some(1)));
    assert_eq!(
        state.output.dynamic_runtime_snapshot().instances.len(),
        0,
        "replay must not toggle back On"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

fn start_cue_owner(state: &AppState, fixture: light_core::FixtureId) -> Uuid {
    let mut snapshot = state.output.snapshot().as_ref().clone();
    let definition = snapshot.dynamics[0].clone();
    let cue_list_id = light_core::CueListId::new();
    let mut cue = light_playback::Cue::new(1_u16.into());
    cue.fade_millis = 0;
    cue.delay_millis = 0;
    cue.dynamic_changes.push(light_playback::CueDynamicChange {
        fixture_id: fixture,
        attribute: light_core::AttributeKey::intensity(),
        value: light_dynamics::DynamicSemanticValue::DynamicOn {
            instance_link: Uuid::new_v4(),
            dynamic: light_dynamics::DynamicReference {
                dynamic_id: Some(definition.id),
                last_known_pool_number: definition.pool_number,
                embedded_fallback: light_dynamics::DynamicDefinitionSnapshot {
                    definition: Arc::new(definition.clone()),
                },
            },
            lane_id: definition.lanes[0].id,
            overrides: light_dynamics::DynamicInstanceOverrides {
                size: 1.0,
                speed_multiplier: light_dynamics::Rational::ONE,
                phase_offset_degrees: 0.0,
            },
            timing: Default::default(),
        },
        automatic_restore: false,
    });
    let list: light_playback::CueList = serde_json::from_value(serde_json::json!({
        "id":cue_list_id,"name":"Independent running Cue", "priority":1,"mode":"sequence", "looped":false,"cues":[cue]
    })).unwrap();
    snapshot.cue_lists = vec![list].into();
    state.output.replace_snapshot(snapshot).unwrap();
    state
        .output
        .execute_playback(EnginePlaybackCommand::CueList {
            id: cue_list_id,
            action: light_engine::CueListPlaybackAction::Go,
        })
        .unwrap();
    assert_eq!(
        state.output.active_cue_dynamic_values().len(),
        1,
        "registered Cue must author the independent runtime owner"
    );
    state.output.reconcile_dynamic_runtime();
    let runtime = state.output.dynamic_runtime_snapshot();
    assert_eq!(runtime.instances.len(), 1);
    assert_eq!(runtime.instances[0].controllers.len(), 1);
    assert!(
        matches!(runtime.instances[0].controllers[0].source, light_dynamics::DynamicControllerSource::Cue { cue_list_id: owner, .. } if owner == cue_list_id.0)
    );
    runtime.instances[0].controllers[0].id
}

#[test]
fn dynamic_pool_tap_preload_stages_without_touching_live_cue_owner() {
    let (state, session, data_dir, fixture) = desk();
    let cue_controller = start_cue_owner(&state, fixture);
    let live = state.output.dynamic_runtime_snapshot();
    assert_eq!(live.instances.len(), 1);
    assert_eq!(live.instances[0].controllers[0].id, cue_controller);
    assert!(state.programming.arm_preload(session.id, true));
    assert!(execute(
        &state,
        &session,
        "pending-on",
        "DYNAMIC 1",
        Some(1)
    ));
    let pending = state.programming.get(session.id).unwrap();
    assert_eq!(pending.preload_dynamic_pending.len(), 1);
    let light_dynamics::DynamicSemanticValue::DynamicOn {
        instance_link: pending_link,
        ..
    } = pending.preload_dynamic_pending[0].value
    else {
        panic!("pending On expected");
    };
    assert_eq!(pending.preload_dynamic_pending[0].fixture_id, fixture);
    assert!(execute(
        &state,
        &session,
        "pending-off",
        "DYNAMIC 1",
        Some(1)
    ));
    let pending = state.programming.get(session.id).unwrap();
    assert_eq!(pending.preload_dynamic_pending.len(), 1);
    assert!(
        matches!(pending.preload_dynamic_pending[0].value, light_dynamics::DynamicSemanticValue::DynamicOff { instance_link, .. } if instance_link == pending_link)
    );
    assert_eq!(state.output.dynamic_runtime_snapshot(), live);
    assert!(pending.dynamic_values.is_empty());
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn dynamic_pool_tap_empty_selection_selects_capable_before_start_and_explicit_off_stays_off() {
    let (state, session, data_dir, fixture) = desk();
    state.programming.select(session.id, []);
    assert!(execute(
        &state,
        &session,
        "select-capable",
        "DYNAMIC 1",
        Some(1)
    ));
    assert_eq!(
        state.programming.get(session.id).unwrap().selected,
        vec![fixture]
    );
    assert_eq!(state.output.dynamic_runtime_snapshot().instances.len(), 0);
    assert!(execute(
        &state,
        &session,
        "start-capable",
        "DYNAMIC 1",
        Some(1)
    ));
    assert_eq!(state.output.dynamic_runtime_snapshot().instances.len(), 1);
    assert!(execute(
        &state,
        &session,
        "explicit-off",
        "DYNAMIC 1 OFF",
        Some(1)
    ));
    assert_eq!(state.output.dynamic_runtime_snapshot().instances.len(), 0);
    assert!(execute(
        &state,
        &session,
        "explicit-off-again",
        "DYNAMIC 1 OFF",
        Some(1)
    ));
    assert_eq!(state.output.dynamic_runtime_snapshot().instances.len(), 0);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn dynamic_pool_tap_group_prefix_stops_only_its_exact_target_scope() {
    let (state, session, data_dir, fixture_a) = desk();
    let fixture_b = light_core::FixtureId::new();
    let mut snapshot = state.output.snapshot().as_ref().clone();
    let mut second = operational_fixture(fixture_b);
    second.address = Some(2);
    snapshot.fixtures = vec![operational_fixture(fixture_a), second].into();
    snapshot.groups = vec![light_programmer::GroupDefinition {
        id: "18".into(),
        fixtures: vec![fixture_b],
        ..Default::default()
    }]
    .into();
    state.output.replace_snapshot(snapshot).unwrap();
    assert!(execute(&state, &session, "a-on", "DYNAMIC 1", Some(1)));
    assert!(execute(
        &state,
        &session,
        "b-on",
        "GROUP 18 DYNAMIC 1",
        Some(1)
    ));
    let running = state.output.dynamic_runtime_snapshot();
    assert_eq!(running.instances.len(), 2);
    assert!(execute(
        &state,
        &session,
        "b-off",
        "GROUP 18 DYNAMIC 1",
        Some(1)
    ));
    let running = state.output.dynamic_runtime_snapshot();
    assert_eq!(running.instances.len(), 1);
    assert_eq!(running.instances[0].targets, vec![fixture_a]);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn dynamic_pool_tap_does_not_release_an_independent_cue_owner() {
    let (state, session, data_dir, fixture) = desk();
    let cue_controller = start_cue_owner(&state, fixture);
    assert!(execute(
        &state,
        &session,
        "mine-start",
        "DYNAMIC 1",
        Some(1)
    ));
    let both = state.output.dynamic_runtime_snapshot();
    assert_eq!(both.instances.len(), 2);
    assert!(execute(&state, &session, "mine-stop", "DYNAMIC 1", Some(1)));
    let after = state.output.dynamic_runtime_snapshot();
    assert_eq!(after.instances.len(), 1);
    assert_eq!(after.instances[0].controllers.len(), 1);
    assert_eq!(after.instances[0].controllers[0].id, cue_controller);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn dynamic_pool_tap_another_surface_of_same_desk_toggles_shared_programmer() {
    let (state, session, data_dir, _) = desk();
    let other = Session {
        id: SessionId::new(),
        token: "same-desk-surface".into(),
        ..session.clone()
    };
    state.programming.start(other.id);
    state.sessions.insert_session(other.clone());
    assert!(execute(
        &state,
        &session,
        "surface-a-on",
        "DYNAMIC 1",
        Some(1)
    ));
    assert!(execute(
        &state,
        &other,
        "surface-b-off",
        "DYNAMIC 1",
        Some(1)
    ));
    assert_eq!(state.output.dynamic_runtime_snapshot().instances.len(), 0);
    assert_eq!(
        state.programming.get(session.id).unwrap().id,
        state.programming.get(other.id).unwrap().id
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn dynamic_pool_tap_http_execute_matches_ws_toggle_and_accepts_unknown_properties() {
    let (state, session, data_dir, _) = desk();
    let app = router(state.clone());
    for request_id in ["http-on", "http-off"] {
        let mut payload = serde_json::json!({"command":"DYNAMIC 1", "request_id":request_id, "pool_tap":{"type":"dynamic","pool_number":1}, "future_property":true});
        if request_id == "http-off" {
            state
                .programming
                .update_command_line(session.id, |current| {
                    ("DYNAMIC 1".into(), current.target, false)
                })
                .unwrap();
            payload.as_object_mut().unwrap().remove("command");
        }
        let response = app
            .clone()
            .oneshot(
                Request::post("/api/v2/command-line/execute")
                    .header("Authorization", format!("Bearer {}", session.token))
                    .header("Content-Type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(
            status,
            StatusCode::OK,
            "{}",
            String::from_utf8_lossy(&bytes)
        );
    }
    assert_eq!(state.output.dynamic_runtime_snapshot().instances.len(), 0);
    assert_eq!(
        state
            .programming
            .get(session.id)
            .unwrap()
            .dynamic_values
            .len(),
        1
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn dynamic_pool_tap_explicit_size_changes_controller_without_toggling_it() {
    let (state, session, data_dir, _) = desk();
    assert!(execute(&state, &session, "size-on", "DYNAMIC 1", Some(1)));
    let before = state.output.dynamic_runtime_snapshot();
    assert!(execute(
        &state,
        &session,
        "size-half",
        "DYNAMIC 1 SIZE AT 50",
        Some(1)
    ));
    let after = state.output.dynamic_runtime_snapshot();
    assert_eq!(after.instances.len(), 1);
    assert_eq!(after.instances[0].id, before.instances[0].id);
    assert_eq!(
        after.instances[0].controllers[0].id,
        before.instances[0].controllers[0].id
    );
    assert!((after.instances[0].controllers[0].size - 0.5).abs() < f32::EPSILON);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn dynamic_pool_tap_rejects_ambiguous_programmer_addresses_before_group_selection() {
    let (state, session, data_dir, fixture) = desk();
    let group_target = light_core::FixtureId::new();
    let mut snapshot = state.output.snapshot().as_ref().clone();
    snapshot.groups = vec![light_programmer::GroupDefinition {
        id: "18".into(),
        fixtures: vec![group_target],
        ..Default::default()
    }]
    .into();
    state.output.replace_snapshot(snapshot).unwrap();
    let before = state.programming.get(session.id).unwrap();
    assert_eq!(before.selected, vec![fixture]);
    for command in [
        "GROUP 18 DYNAMIC 1 DYNAMIC 2",
        "MOVE DYNAMIC 1 AT DYNAMIC 2",
    ] {
        assert!(!execute(&state, &session, command, command, Some(2)));
        let after = state.programming.get(session.id).unwrap();
        assert_eq!(after.selected, before.selected);
        assert_eq!(
            serde_json::to_value(&after.values).unwrap(),
            serde_json::to_value(&before.values).unwrap()
        );
        assert_eq!(
            serde_json::to_value(&after.dynamic_values).unwrap(),
            serde_json::to_value(&before.dynamic_values).unwrap()
        );
        assert_eq!(after.undo.len(), before.undo.len());
        assert_eq!(state.output.dynamic_runtime_snapshot().instances.len(), 0);
    }
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn dynamic_pool_tap_valid_copy_uses_destination_provenance_without_changing_programmer() {
    let (state, session, data_dir, _) =
        super::dynamic_pool_command_tests::dynamic_pool_command_desk().await;
    let before = state.programming.get(session.id).unwrap();
    assert!(execute(
        &state,
        &session,
        "pool-copy",
        "COPY DYNAMIC 32 AT DYNAMIC 33",
        Some(33)
    ));
    let entry = state.active_show.current().unwrap();
    let objects = ActiveShowRepository::open(&entry.path)
        .unwrap()
        .objects_with_portable_revision("dynamic")
        .unwrap()
        .1;
    assert!(
        objects
            .iter()
            .any(|object| object.body["pool_number"] == 32)
    );
    assert!(
        objects
            .iter()
            .any(|object| object.body["pool_number"] == 33)
    );
    let after = state.programming.get(session.id).unwrap();
    assert_eq!(after.selected, before.selected);
    assert_eq!(
        serde_json::to_value(&after.dynamic_values).unwrap(),
        serde_json::to_value(&before.dynamic_values).unwrap()
    );
    assert_eq!(after.undo.len(), before.undo.len());
    let _ = std::fs::remove_dir_all(data_dir);
}
