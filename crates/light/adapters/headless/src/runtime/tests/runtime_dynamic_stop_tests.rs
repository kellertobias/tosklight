use super::*;

#[tokio::test]
async fn guarded_running_dynamic_stop_is_live_while_preload_is_armed() {
    assert_guarded_stop(false).await;
}

#[tokio::test]
async fn guarded_virtual_dynamic_stop_rejects_stale_targets_and_keeps_live_programmer() {
    assert_guarded_stop(true).await;
}

async fn assert_guarded_stop(virtual_owner: bool) {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = session_for_token(&state, &token);
    open_playback_test_show(&app, &token).await;
    install_pool_cuelist_test_state(&state);
    let fixture = light_core::FixtureId::new();
    let mut dynamic = command_test_dynamic(Uuid::new_v4(), 1);
    dynamic.target_binding = light_dynamics::DynamicTargetBinding::FrozenTargets {
        targets: vec![fixture],
    };
    let dynamic_id = dynamic.id;
    let mut snapshot = (*state.output.snapshot()).clone();
    snapshot.fixtures = vec![operational_fixture(fixture)].into();
    snapshot.dynamics = vec![dynamic.clone()].into();
    Arc::make_mut(&mut snapshot.playback_pages)[0]
        .assignments
        .get_mut(&1)
        .unwrap()
        .target = light_playback::PlaybackTarget::Dynamic {
        assignment: light_playback::DynamicPlaybackAssignment {
            dynamic: light_dynamics::DynamicReference {
                dynamic_id: Some(dynamic_id),
                last_known_pool_number: 1,
                embedded_fallback: light_dynamics::DynamicDefinitionSnapshot {
                    definition: Arc::new(dynamic),
                },
            },
            revision: 1,
            target_scope: None,
            fader_mode: light_playback::DynamicPlaybackFaderMode::SizeAndMaster,
            priority: 0,
            activation_override: None,
            resume_policy: light_playback::DynamicPlaybackResumePolicy::FollowDynamic,
            local_speed_multiplier: light_dynamics::Rational::ONE,
            learned_duration_millis: None,
            crossfade_non_intensity: false,
            auto_off_at_zero: false,
            auto_off_flash_release: false,
            auto_off_full_control: false,
        },
    };
    let definition = &mut Arc::make_mut(&mut snapshot.playback_pages)[0]
        .assignments
        .get_mut(&1)
        .unwrap();
    definition.buttons = light_playback::PlaybackDefinition::default_buttons(&definition.target);
    let owner_number = if virtual_owner { 303 } else { 1 };
    if virtual_owner {
        let definition = Arc::make_mut(&mut snapshot.playback_pages)[0]
            .assignments
            .remove(&1)
            .unwrap();
        Arc::make_mut(&mut snapshot.playback_pages).push(light_playback::PlaybackPage {
            number: 2,
            name: "Second page".into(),
            assignments: HashMap::from([(owner_number, definition)]),
        });
    }
    state.output.replace_snapshot(snapshot).unwrap();
    let owner =
        light_playback::PlaybackAddress::new(if virtual_owner { 2 } else { 1 }, owner_number)
            .unwrap();
    state.output.execute_playback(start_command(owner)).unwrap();
    state
        .output
        .execute_playback(start_command(
            light_playback::PlaybackAddress::new(1, 2).unwrap(),
        ))
        .unwrap();
    state.output.reconcile_dynamic_runtime();
    let runtime = state.output.dynamic_runtime_snapshot();
    let instance = runtime.instances.first().expect("live Dynamic instance");
    let controller = instance.controllers.first().unwrap();
    assert_eq!(
        controller.source,
        light_dynamics::DynamicControllerSource::Playback {
            page: owner.page(),
            position: owner.position()
        }
    );
    state.programming.set(
        session.id,
        fixture,
        light_core::AttributeKey::intensity(),
        light_core::AttributeValue::Normalized(0.2),
    );
    assert_preload_key(
        &app,
        &token,
        session.desk.id,
        "arm-dynamic-stop",
        "preload_entered",
    )
    .await;
    let programmer_before = serde_json::to_value(state.programming.get(session.id)).unwrap();
    let address =
        serde_json::json!({"kind":"playback","page":owner.page(),"position":owner.position()});
    let action = serde_json::json!({"type":"runtime_stop_dynamic", "dynamic_id":dynamic_id,"instance_id":instance.id,"controller_id":controller.id});
    for field in ["dynamic_id", "instance_id", "controller_id"] {
        let mut forged = action.clone();
        forged[field] = Uuid::new_v4().to_string().into();
        let response = post_action(&app, Some(&token), session.desk.id, serde_json::json!({"address":address,"control_source":{"surface_id":"main","control_id":1},"action":forged})).await;
        let status = response.status();
        let body = json(response).await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert!(state.output.active_dynamic_playback(owner).unwrap().enabled);
        assert_eq!(
            serde_json::to_value(state.programming.get(session.id)).unwrap(),
            programmer_before
        );
    }
    let action = assert_assignment_changed(
        &state,
        &app,
        &token,
        &session,
        owner,
        &address,
        &action,
        &programmer_before,
    )
    .await;
    let foreign_controller = Uuid::new_v4();
    let foreign_instance = state
        .output
        .start_dynamic(light_dynamics::DynamicStartRequest {
            definition_id: dynamic_id,
            controller: light_dynamics::DynamicController {
                id: foreign_controller,
                source: light_dynamics::DynamicControllerSource::Programmer {
                    programmer_id: Uuid::new_v4(),
                    instance_link: None,
                },
                ..controller.clone()
            },
            target_scope: light_dynamics::DynamicTargetScope {
                ordered_targets: vec![fixture],
            },
            stage_positions: HashMap::new(),
            inherited_spatial_mapping: None,
            now_millis: controller.activated_at_millis,
            activation_delay_millis: 0,
            activation_duration_millis: 0,
            activation_policy_override: None,
            reuse_matching_targetless: false,
        })
        .unwrap();
    let mut foreign = action.clone();
    foreign["controller_id"] = foreign_controller.to_string().into();
    foreign["instance_id"] = foreign_instance.to_string().into();
    let rejected = post_action(&app, Some(&token), session.desk.id, serde_json::json!({"address":address,"control_source":{"surface_id":"main","control_id":1},"action":foreign})).await;
    assert_eq!(rejected.status(), StatusCode::CONFLICT);
    assert_eq!(
        serde_json::to_value(state.programming.get(session.id)).unwrap(),
        programmer_before
    );
    let runtime_response = app
        .clone()
        .oneshot(
            Request::get("/api/v2/dynamics/runtime")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(runtime_response.status(), StatusCode::OK);
    let projected = json(runtime_response).await;
    let rows = projected["instances"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|instance| instance["controllers"].as_array().unwrap())
        .collect::<Vec<_>>();
    let owner_row = rows
        .iter()
        .find(|row| row["controller_id"] == action["controller_id"])
        .unwrap();
    let expected_owner =
        serde_json::json!({"kind":"playback","page":owner.page(),"position":owner.position()});
    assert_eq!(owner_row["stop_owner"], expected_owner);
    let foreign_row = rows
        .iter()
        .find(|row| row["controller_id"] == foreign_controller.to_string())
        .unwrap();
    assert!(foreign_row.get("stop_owner").is_none());
    let wrong_owner = post_action(&app, Some(&token), session.desk.id, serde_json::json!({"address":{"kind":"playback","page":1,"position":2},"control_source":{"surface_id":"main","control_id":1},"action":action})).await;
    assert_eq!(wrong_owner.status(), StatusCode::CONFLICT);
    assert!(state.output.active_dynamic_playback(owner).unwrap().enabled);
    let request = serde_json::json!({"address":address,"control_source":{"surface_id":"main","control_id":1},"action":action});
    if virtual_owner {
        let response = dispatch_live_action(
            &state,
            &session,
            live_action_frame(
                &session,
                "stop-exact-dynamic",
                serde_json::from_value(serde_json::json!({"type":"playback","request":request}))
                    .unwrap(),
            ),
        );
        assert!(response.ok, "{:?}", response.error);
    } else {
        let response = post_action(&app, Some(&token), session.desk.id, request).await;
        let status = response.status();
        let body = json(response).await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
    assert!(
        !state
            .output
            .active_dynamic_playbacks_for_persistence()
            .iter()
            .any(|playback| playback.enabled)
    );
    assert_eq!(
        serde_json::to_value(state.programming.get(session.id)).unwrap(),
        programmer_before
    );
    assert!(
        state
            .output
            .playback_runtime_status_at(light_playback::PlaybackAddress::new(1, 2).unwrap())
            .unwrap()
            .playback
            .enabled
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

async fn assert_assignment_changed(
    state: &AppState,
    app: &Router,
    token: &str,
    session: &Session,
    owner: light_playback::PlaybackAddress,
    address: &serde_json::Value,
    action: &serde_json::Value,
    programmer_before: &serde_json::Value,
) -> serde_json::Value {
    let original = (*state.output.snapshot()).clone();
    let mut reassigned = original.clone();
    let cue_target = reassigned
        .playback_pages
        .iter()
        .find(|page| page.number == 1)
        .unwrap()
        .assignments[&2]
        .target
        .clone();
    Arc::make_mut(&mut reassigned.playback_pages)
        .iter_mut()
        .find(|page| page.number == owner.page())
        .unwrap()
        .assignments
        .get_mut(&owner.position())
        .unwrap()
        .target = cue_target;
    state.output.replace_snapshot(reassigned).unwrap();
    let state_before = state.output.dynamic_runtime_snapshot();
    let stale = post_action(app, Some(token), session.desk.id, serde_json::json!({"address":address,"control_source":{"surface_id":"main","control_id":1},"action":action})).await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    assert_eq!(state.output.dynamic_runtime_snapshot(), state_before);
    assert_eq!(
        serde_json::to_value(state.programming.get(session.id)).unwrap(),
        *programmer_before
    );

    state.output.replace_snapshot(original).unwrap();
    state.output.execute_playback(start_command(owner)).unwrap();
    state.output.reconcile_dynamic_runtime();
    let runtime = state.output.dynamic_runtime_snapshot();
    let instance = runtime
        .instances
        .iter()
        .find(|row| row.definition.id.to_string() == action["dynamic_id"].as_str().unwrap())
        .unwrap();
    let controller = instance.controllers.iter().find(|row| matches!(row.source, light_dynamics::DynamicControllerSource::Playback { page, position } if page == owner.page() && position == owner.position())).unwrap();
    serde_json::json!({"type":"runtime_stop_dynamic","dynamic_id":instance.definition.id,"instance_id":instance.id,"controller_id":controller.id})
}

fn start_command(address: light_playback::PlaybackAddress) -> light_engine::EnginePlaybackCommand {
    light_engine::EnginePlaybackCommand::Playback {
        address,
        action: light_engine::PlaybackAction::On,
        activation_origin: None,
        control_source: light_playback::PlaybackControlSource {
            surface_id: "main".into(),
            control_id: 1,
        },
    }
}
#[test]
fn runtime_owner_projection_keeps_exact_addresses_and_omits_unsafe_sources() {
    use crate::runtime::dynamics_http::runtime_stop_owner;
    use light_dynamics::DynamicControllerSource as Source;
    use light_wire::v2::dynamics::DynamicRuntimeStopOwner as Owner;
    for (page, position) in [(1, 11), (2, 303), (99, 1000)] {
        assert_eq!(
            runtime_stop_owner(&Source::Playback { page, position }),
            Some(Owner::Playback { page, position })
        );
    }
    for (page, position) in [(1, 1001), (0, 303), (100, 1), (1, 0)] {
        assert_eq!(
            runtime_stop_owner(&Source::Playback { page, position }),
            None
        );
    }
    assert_eq!(
        runtime_stop_owner(&Source::Programmer {
            programmer_id: Uuid::new_v4(),
            instance_link: None
        }),
        None
    );
    assert_eq!(
        runtime_stop_owner(&Source::Cue {
            cue_list_id: Uuid::new_v4(),
            instance_link: Uuid::new_v4()
        }),
        None
    );
}
