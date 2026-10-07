//! Actual v2 HTTP/WS and command Aim authoring. Physical output fitting remains TL-548.
use super::*;
use light_core::programming::{PositionIntent, TargetReference};

fn legacy_aim(number: u32) -> light_programmer::Preset {
    light_programmer::Preset {
        family: light_programmer::PresetFamily::Position,
        number: 7,
        aim_at_fixture_number: Some(number),
        ..Default::default()
    }
}
fn requested(point: light_core::FixtureId) -> light_core::AttributeValue {
    light_core::AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Point { point_id: point.0 },
        [3., 4., 4.],
    )))
}
fn recall_request(scenario: &CommandHttpScenario, preset_revision: u64) -> serde_json::Value {
    let show = scenario.state.active_show.current().clone().unwrap();
    let show_revision = ActiveShowRepository::open(&show.path)
        .unwrap()
        .portable_document()
        .unwrap()
        .revision()
        .value();
    let selection = scenario
        .state
        .programming
        .programmers()
        .selection(scenario.session.id)
        .unwrap();
    serde_json::json!({
        "address":{"family":"position","number":7},
        "expected_preset_revision":preset_revision, "expected_show_revision":show_revision,
        "expected_programmer_revision":scenario.state.programming.normal_values_revision(),
        "expected_preload_values_revision":scenario.state.programming.preload_values_revision(),
        "expected_capture_mode_revision":scenario.state.programming.capture_mode_revision(),
        "expected_selection_revision":selection.revision,
    })
}

#[tokio::test]
async fn semantic_aim_v2_recall_and_ws_preserve_target_in_normal_blind_and_preload() {
    for mode in ["normal", "blind", "preload"] {
        let scenario = CommandHttpScenario::new().await;
        let show_id = scenario.create_and_open_show("Semantic Aim recall").await;
        let (point, mover, _) =
            super::super::aim_point_tests::install_semantic_aim_rig(&scenario.state);
        let show = scenario.state.active_show.current().clone().unwrap();
        let repository = ActiveShowRepository::open(&show.path).unwrap();
        let mut raw = serde_json::to_value(legacy_aim(5)).unwrap();
        raw["future_extension"] = serde_json::json!({"retained":true});
        repository.put_object("preset", "3.7", &raw, 0).unwrap();
        scenario
            .state
            .programming
            .select(scenario.session.id, [mover]);
        scenario.state.programming.set_modes(
            scenario.session.id,
            Some(mode == "blind"),
            None,
            None,
            None,
        );
        if mode != "normal" {
            scenario
                .state
                .programming
                .arm_preload(scenario.session.id, mode == "preload");
        }
        for (key, amount) in [("pan", 0.2), ("tilt", 0.6)] {
            scenario.state.programming.set(
                scenario.session.id,
                mover,
                light_core::AttributeKey(key.into()),
                light_core::AttributeValue::Normalized(amount),
            );
        }
        let body = recall_request(&scenario, 1);
        let response = scenario
            .preset_recall_action(&show_id, Some(&scenario.token), body)
            .await;
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "{mode}: {:?}",
            json(response).await
        );
        let programmer = scenario.state.programming.get(scenario.session.id).unwrap();
        let values = if mode == "preload" {
            &programmer.preload_pending
        } else {
            &programmer.values
        };
        assert_eq!(values.len(), 1, "one atomic Position owner");
        assert_eq!(values[0].fixture_id, mover);
        assert_eq!(
            values[0].attribute,
            light_core::AttributeKey("position".into())
        );
        assert_eq!(values[0].value, requested(point));
        if mode == "preload" {
            assert!(programmer.values.is_empty());
        }
        assert_eq!(
            repository
                .objects("preset")
                .unwrap()
                .into_iter()
                .find(|object| object.id == "3.7")
                .unwrap()
                .body,
            raw,
            "recall never rewrites the stored legacy operand"
        );
        let body = recall_request(&scenario, 1);
        let action = light_wire::v2::live_action::LiveAction::PresetRecall(
            light_wire::v2::live_action::PresetRecallLiveActionRequest {
                request_id: "semantic-aim-ws".into(),
                show_id: Uuid::parse_str(&show_id).unwrap(),
                request: serde_json::from_value(body).unwrap(),
            },
        );
        let response = dispatch_live_action(
            &scenario.state,
            &scenario.session,
            live_action_frame(&scenario.session, "semantic-aim-ws", action),
        );
        assert!(response.ok, "{:?}", response.error);
        assert_eq!(
            scenario
                .state
                .programming
                .get(scenario.session.id)
                .unwrap()
                .blind,
            mode != "normal"
        );
        let _ = std::fs::remove_dir_all(scenario.data_dir);
    }
}

#[tokio::test]
async fn semantic_aim_v2_validates_missing_target_before_empty_selection_without_mutation() {
    let scenario = CommandHttpScenario::new().await;
    let show_id = scenario.create_and_open_show("Empty Aim recall").await;
    super::super::aim_point_tests::install_semantic_aim_rig(&scenario.state);
    let show = scenario.state.active_show.current().clone().unwrap();
    let repository = ActiveShowRepository::open(&show.path).unwrap();
    repository
        .put_object(
            "preset",
            "3.7",
            &serde_json::to_value(legacy_aim(998)).unwrap(),
            0,
        )
        .unwrap();
    let before =
        serde_json::to_value(scenario.state.programming.get(scenario.session.id).unwrap()).unwrap();
    let response = scenario
        .preset_recall_action(
            &show_id,
            Some(&scenario.token),
            recall_request(&scenario, 1),
        )
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(
        json(response)
            .await
            .to_string()
            .contains("no fixture numbered 998")
    );
    assert_eq!(
        serde_json::to_value(scenario.state.programming.get(scenario.session.id).unwrap()).unwrap(),
        before
    );
    repository
        .put_object(
            "preset",
            "3.7",
            &serde_json::to_value(legacy_aim(5)).unwrap(),
            1,
        )
        .unwrap();
    let response = scenario
        .preset_recall_action(
            &show_id,
            Some(&scenario.token),
            recall_request(&scenario, 2),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        serde_json::to_value(scenario.state.programming.get(scenario.session.id).unwrap()).unwrap(),
        before
    );
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn semantic_aim_record_stores_target_and_group_recall_keeps_live_owner() {
    let scenario = CommandHttpScenario::new().await;
    scenario.create_and_open_show("Stored semantic Aim").await;
    let (point, mover, _) =
        super::super::aim_point_tests::install_semantic_aim_rig(&scenario.state);
    let show = scenario.state.active_show.current().clone().unwrap();
    let rig = scenario.state.output.snapshot();
    let group = rig.groups[0].clone();
    ActiveShowRepository::open(&show.path)
        .unwrap()
        .put_object("group", "1", &serde_json::to_value(group).unwrap(), 0)
        .unwrap();
    let response = scenario
        .execute("record-semantic-aim", Some("RECORD 3.7 AT FIXTURE 5"))
        .await;
    let status = response.status();
    let outcome = json(response).await;
    assert_eq!(status, StatusCode::OK, "{outcome:?}");
    let show = scenario.state.active_show.current().clone().unwrap();
    let repository = ActiveShowRepository::open(&show.path).unwrap();
    let object = repository
        .objects("preset")
        .unwrap()
        .into_iter()
        .find(|object| object.id == "3.7")
        .unwrap_or_else(|| panic!("Aim record did not create a preset: {outcome:?}"));
    let recorded_body = object.body;
    let preset: light_programmer::Preset = serde_json::from_value(recorded_body.clone()).unwrap();
    assert_eq!(preset.aim_at_fixture_number, None);
    assert!(preset.values.is_empty() && preset.group_values.is_empty());
    assert_eq!(
        preset.universal_values[&light_core::AttributeKey("position".into())],
        requested(point)
    );
    // Record reconciles from the portable show; this authoring fixture rig is intentionally
    // test-local, so restore its captured fixture models before the second target observation.
    scenario
        .state
        .output
        .replace_snapshot(light_engine::EngineSnapshot {
            revision: scenario.state.output.snapshot().revision + 1,
            ..rig.as_ref().clone()
        })
        .unwrap();
    let response = scenario
        .execute("overwrite-semantic-aim", Some("RECORD 3.7 AT FIXTURE 901"))
        .await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "{:?}",
        json(response).await
    );
    let overwritten = repository
        .objects("preset")
        .unwrap()
        .into_iter()
        .find(|object| object.id == "3.7")
        .unwrap();
    assert_ne!(
        overwritten.body, recorded_body,
        "the existing Target was overwritten"
    );
    let response = scenario
        .press_key(&scenario.token, "UND", "undo-overwritten-aim")
        .await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "{:?}",
        json(response).await
    );
    let restored = repository
        .objects("preset")
        .unwrap()
        .into_iter()
        .find(|object| object.id == "3.7")
        .unwrap();
    assert_eq!(
        restored.body, recorded_body,
        "overwrite Undo restores the prior Target rather than deleting it"
    );
    assert_eq!(
        scenario
            .execute("recall-live-aim", Some("GROUP 1 AT 3.7"))
            .await
            .status(),
        StatusCode::OK
    );
    let programmer = scenario.state.programming.get(scenario.session.id).unwrap();
    assert!(programmer.values.is_empty());
    assert_eq!(
        programmer.group_values["1"][&light_core::AttributeKey("position".into())].value,
        requested(point)
    );
    assert_eq!(
        scenario
            .execute("recall-frozen-aim", Some("DEGRP 1 AT 3.7"))
            .await
            .status(),
        StatusCode::OK
    );
    let programmer = scenario.state.programming.get(scenario.session.id).unwrap();
    assert!(
        programmer
            .values
            .iter()
            .any(|value| value.fixture_id == mover && value.value == requested(point))
    );
    for request_id in ["undo-frozen-aim", "undo-live-aim", "undo-recorded-aim"] {
        let response = scenario.press_key(&scenario.token, "UND", request_id).await;
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "{request_id}: {:?}",
            json(response).await
        );
    }
    assert!(
        repository
            .objects("preset")
            .unwrap()
            .iter()
            .all(|object| object.id != "3.7"),
        "Record undo removes the newly created Target preset"
    );
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn semantic_aim_commands_replace_whole_position_family_and_undo_once() {
    for mode in ["normal", "blind", "preload"] {
        for command in [
            "FIXTURE 1 AT FIXTURE 5",
            "FIXTURE 1 AT 3.7",
            "GROUP 1 AT 3.7",
            "DEGRP 1 AT 3.7",
            "FIXTURE 1 AT 0.7",
            "GROUP 1 AT 0.7",
            "FIXTURE GROUP 1 AT FIXTURE 5",
            "FIXTURE GROUP 1 AT FIXTURE 5 TIME 1 DELAY 0.5",
        ] {
            let scenario = CommandHttpScenario::new().await;
            scenario
                .create_and_open_show("Atomic Position command")
                .await;
            let (point, mover, _) =
                super::super::aim_point_tests::install_semantic_aim_rig(&scenario.state);
            let show = scenario.state.active_show.current().clone().unwrap();
            let repository = ActiveShowRepository::open(&show.path).unwrap();
            repository
                .put_object(
                    "preset",
                    "3.7",
                    &serde_json::to_value(legacy_aim(5)).unwrap(),
                    0,
                )
                .unwrap();
            let mut mixed = legacy_aim(5);
            mixed.family = light_programmer::PresetFamily::Mixed;
            mixed.aim_at_fixture_number = None;
            mixed.universal_values.insert(
                light_core::AttributeKey("position".into()),
                requested(point),
            );
            mixed.universal_values.insert(
                light_core::AttributeKey::intensity(),
                light_core::AttributeValue::Normalized(0.4),
            );
            repository
                .put_object("preset", "7", &serde_json::to_value(mixed).unwrap(), 0)
                .unwrap();
            scenario.state.programming.set_modes(
                scenario.session.id,
                Some(mode == "blind"),
                None,
                None,
                None,
            );
            if mode != "normal" {
                scenario
                    .state
                    .programming
                    .arm_preload(scenario.session.id, mode == "preload");
            }
            let group = command.starts_with("GROUP") || command.starts_with("FIXTURE GROUP");
            for (key, amount) in [("pan", 0.2), ("tilt", 0.6)] {
                let key = light_core::AttributeKey(key.into());
                let value = light_core::AttributeValue::Normalized(amount);
                if group {
                    scenario.state.programming.set_group_immediate_with_delay(
                        scenario.session.id,
                        "1".into(),
                        key,
                        value,
                        None,
                    );
                } else {
                    scenario
                        .state
                        .programming
                        .set(scenario.session.id, mover, key, value);
                }
            }
            let before = scenario.state.programming.get(scenario.session.id).unwrap();
            let undo_depth = scenario
                .state
                .programming
                .undo_depth(scenario.session.id)
                .unwrap();
            let response = scenario.execute("atomic-position", Some(command)).await;
            assert_eq!(
                response.status(),
                StatusCode::OK,
                "{mode}: {command}: {:?}",
                json(response).await
            );
            let after = scenario.state.programming.get(scenario.session.id).unwrap();
            let count = if command.ends_with("0.7") { 2 } else { 1 };
            if group {
                let values = if mode == "preload" {
                    &after.preload_group_pending
                } else {
                    &after.group_values
                };
                assert_eq!(values["1"].len(), count, "{mode}: {command}");
                assert_eq!(
                    values["1"][&light_core::AttributeKey("position".into())].value,
                    requested(point)
                );
                if command.contains("TIME 1") {
                    let position = &values["1"][&light_core::AttributeKey("position".into())];
                    assert!(position.fade);
                    assert_eq!(position.fade_millis, Some(1000));
                    assert_eq!(position.delay_millis, Some(500));
                }
                assert!(
                    after.values.is_empty() && after.preload_pending.is_empty(),
                    "live Group owner retained"
                );
            } else {
                let values = if mode == "preload" {
                    &after.preload_pending
                } else {
                    &after.values
                };
                assert_eq!(values.len(), count, "{mode}: {command}");
                assert_eq!(
                    values
                        .iter()
                        .find(|value| value.attribute.0.as_ref() == "position")
                        .unwrap()
                        .value,
                    requested(point)
                );
            }
            if mode == "preload" {
                assert_eq!(after.values, before.values);
                assert_eq!(after.group_values, before.group_values);
                assert_eq!(
                    after.active_context, before.active_context,
                    "Pending recall leaves Normal context"
                );
            }
            assert_eq!(
                scenario.state.programming.undo_depth(scenario.session.id),
                Some(undo_depth + 1),
                "one entered command checkpoint"
            );
            assert!(scenario.state.programming.undo(scenario.session.id));
            let undone = scenario.state.programming.get(scenario.session.id).unwrap();
            assert_eq!(undone.values, before.values);
            assert_eq!(undone.group_values, before.group_values);
            assert_eq!(undone.preload_pending, before.preload_pending);
            assert_eq!(undone.preload_group_pending, before.preload_group_pending);
            assert_eq!(undone.active_context, before.active_context);
        }
    }
}

#[tokio::test]
async fn semantic_aim_empty_live_group_is_recordable_but_degroup_is_quiet() {
    let scenario = CommandHttpScenario::new().await;
    scenario.create_and_open_show("Empty live Group Aim").await;
    let (point, _, _) = super::super::aim_point_tests::install_semantic_aim_rig(&scenario.state);
    let mut snapshot = scenario.state.output.snapshot().as_ref().clone();
    let mut groups = snapshot.groups.as_ref().clone();
    groups[0].fixtures.clear();
    snapshot.groups = Arc::new(groups);
    scenario.state.output.replace_snapshot(snapshot).unwrap();
    let show = scenario.state.active_show.current().clone().unwrap();
    ActiveShowRepository::open(&show.path)
        .unwrap()
        .put_object(
            "preset",
            "3.7",
            &serde_json::to_value(legacy_aim(5)).unwrap(),
            0,
        )
        .unwrap();
    assert_eq!(
        scenario
            .execute("empty-live-aim", Some("GROUP 1 AT 3.7"))
            .await
            .status(),
        StatusCode::OK
    );
    let before = scenario.state.programming.get(scenario.session.id).unwrap();
    assert_eq!(
        before.group_values["1"][&light_core::AttributeKey("position".into())].value,
        requested(point)
    );
    scenario
        .state
        .programming
        .programmers()
        .clear_normal_values(scenario.session.id);
    assert_eq!(
        scenario
            .execute("empty-direct-aim", Some("FIXTURE GROUP 1 AT FIXTURE 5"))
            .await
            .status(),
        StatusCode::OK
    );
    let before = scenario.state.programming.get(scenario.session.id).unwrap();
    assert_eq!(
        before.group_values["1"][&light_core::AttributeKey("position".into())].value,
        requested(point)
    );
    let revision = scenario.state.programming.normal_values_revision();
    assert_eq!(
        scenario
            .execute("empty-frozen-aim", Some("DEGRP 1 AT 3.7"))
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        scenario.state.programming.normal_values_revision(),
        revision
    );
    assert_eq!(
        scenario
            .state
            .programming
            .get(scenario.session.id)
            .unwrap()
            .group_values,
        before.group_values
    );
}

#[tokio::test]
async fn semantic_aim_record_publishes_one_alignment_reset_in_the_entered_interaction() {
    let scenario = CommandHttpScenario::new().await;
    scenario
        .create_and_open_show("Aim Record Align completion")
        .await;
    let (_, mover, _) = super::super::aim_point_tests::install_semantic_aim_rig(&scenario.state);
    let registry = scenario.state.programming.programmers();
    registry.select(scenario.session.id, [mover]);
    registry
        .activate_alignment(
            scenario.session.id,
            light_programmer::ProgrammerAlignmentMode::Left,
        )
        .unwrap();
    let cursor = scenario.state.events.latest_sequence();
    let response = scenario
        .execute("record-aligned-aim", Some("RECORD 3.7 AT FIXTURE 5"))
        .await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "{:?}",
        json(response).await
    );
    assert!(registry.alignment(scenario.session.id).is_none());
    let light_application::EventReplay::Events(events) = scenario.state.events.replay(
        cursor,
        &light_application::EventFilter::default()
            .with_capability(light_application::EventCapability::Desk),
    ) else {
        panic!("Record interaction cursor remains replayable");
    };
    let align_resets = events.iter().filter(|event| {
        matches!(&event.payload, light_application::ApplicationEvent::Programming(
            light_application::ProgrammingEvent::InteractionChanged(change)) if change.alignment().is_some())
    }).count();
    assert_eq!(
        align_resets, 1,
        "the entered command owns one Align reset publication"
    );
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn semantic_aim_record_with_unresolved_mount_is_a_quiet_zero_applied_no_op() {
    let scenario = CommandHttpScenario::new().await;
    scenario.create_and_open_show("Passive Aim Record").await;
    let (_, _, target) = super::super::aim_point_tests::install_semantic_aim_rig(&scenario.state);
    let snapshot = scenario.state.output.snapshot();
    let mut fixtures = snapshot.fixtures.to_vec();
    fixtures
        .iter_mut()
        .find(|fixture| fixture.fixture_id == target)
        .unwrap()
        .position_master = Some(Uuid::new_v4());
    scenario
        .state
        .output
        .replace_snapshot(light_engine::EngineSnapshot {
            fixtures: fixtures.into(),
            revision: snapshot.revision + 1,
            ..snapshot.as_ref().clone()
        })
        .unwrap();
    let show = scenario.state.active_show.current().clone().unwrap();
    let repository = ActiveShowRepository::open(&show.path).unwrap();
    let show_revision = repository.portable_document().unwrap().revision();
    let undo_depth = scenario.state.programming.undo_depth(scenario.session.id);
    let response = scenario
        .execute("passive-aim-record", Some("RECORD 3.7 AT FIXTURE 5"))
        .await;
    let status = response.status();
    let body = json(response).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["outcome"], "accepted");
    assert_eq!(body["applied"], 0, "no preset was stored");
    assert_eq!(
        repository.portable_document().unwrap().revision(),
        show_revision
    );
    assert!(
        repository
            .objects("preset")
            .unwrap()
            .iter()
            .all(|object| object.id != "3.7")
    );
    assert_eq!(
        scenario.state.programming.undo_depth(scenario.session.id),
        undo_depth
    );
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}
