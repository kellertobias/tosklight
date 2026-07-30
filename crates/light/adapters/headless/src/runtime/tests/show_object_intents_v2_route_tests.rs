use super::*;

async fn post_show_object_intent(
    app: &Router,
    token: &str,
    show_id: &str,
    path: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(
            Request::post(path)
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("x-tosk-show", show_id)
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = serde_json::from_slice(&bytes).unwrap_or_else(
        |_| serde_json::json!({"raw": String::from_utf8_lossy(&bytes).into_owned()}),
    );
    (status, body)
}

async fn create_seeded_show(
    state: &AppState,
    app: &Router,
    token: &str,
    name: &str,
    seeds: &[(&str, &str, serde_json::Value)],
) -> String {
    let show = create_show(app, token, name).await;
    let show_id = show["id"].as_str().unwrap().to_owned();
    let entry = state
        .installation
        .show(light_core::ShowId(Uuid::parse_str(&show_id).unwrap()))
        .unwrap()
        .unwrap();
    let store = ShowStore::open(&entry.path).unwrap();
    for (kind, id, body) in seeds {
        store.put_object(kind, id, body, 0).unwrap();
    }
    let opened = app
        .clone()
        .oneshot(open_show_request(token, &show_id))
        .await
        .unwrap();
    assert_eq!(opened.status(), StatusCode::OK);
    show_id
}

#[tokio::test]
async fn dynamic_object_intents_are_revisioned_atomic_and_replay_safe() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show_id = create_seeded_show(&state, &app, &token, "Dynamic intents", &[]).await;
    let create = serde_json::json!({
        "request_id": "dynamic-create-1",
        "definition": dynamic_definition_json(17)
    });
    let (status, created) = post_show_object_intent(
        &app,
        &token,
        &show_id,
        "/api/v2/dynamics/create",
        create.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    assert_eq!(created["object"]["kind"], "dynamic");
    assert_eq!(created["object"]["revision"], 1);
    assert_eq!(created["object"]["body"]["pool_number"], 17);
    assert_eq!(
        created["object"]["body"]["future_definition_field"],
        "preserved"
    );
    let dynamic_id = created["object"]["id"].as_str().unwrap();

    let (status, replayed) =
        post_show_object_intent(&app, &token, &show_id, "/api/v2/dynamics/create", create).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(replayed["replayed"], true);
    assert_eq!(replayed["object"]["id"], dynamic_id);

    let (status, updated) = post_show_object_intent(
        &app,
        &token,
        &show_id,
        &format!("/api/v2/dynamics/{dynamic_id}/update"),
        serde_json::json!({
            "request_id": "dynamic-update-1",
            "expected_revision": 1,
            "intent": {"type": "set_name", "name": "Color chase"},
            "future_request_field": true
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["object"]["revision"], 2);
    assert_eq!(updated["object"]["body"]["name"], "Color chase");
    assert_eq!(
        updated["object"]["body"]["future_definition_field"],
        "preserved"
    );

    let (status, moved) = post_show_object_intent(
        &app,
        &token,
        &show_id,
        &format!("/api/v2/dynamics/{dynamic_id}/move"),
        serde_json::json!({
            "request_id": "dynamic-move-1",
            "expected_revision": 2,
            "pool_number": 23
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{moved}");
    assert_eq!(moved["object"]["body"]["pool_number"], 23);

    let (status, copied) = post_show_object_intent(
        &app,
        &token,
        &show_id,
        &format!("/api/v2/dynamics/{dynamic_id}/copy"),
        serde_json::json!({
            "request_id": "dynamic-copy-1",
            "expected_revision": 3,
            "pool_number": 24
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{copied}");
    assert_ne!(copied["object"]["id"], dynamic_id);
    assert_eq!(copied["object"]["body"]["pool_number"], 24);

    let (status, deleted) = post_show_object_intent(
        &app,
        &token,
        &show_id,
        &format!("/api/v2/dynamics/{dynamic_id}/delete"),
        serde_json::json!({
            "request_id": "dynamic-delete-1",
            "expected_revision": 3
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{deleted}");
    assert_eq!(deleted["object"]["id"], dynamic_id);

    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn dynamic_phase_mode_seeds_per_lane_phase_without_discarding_it_in_uniform_mode() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show_id = create_seeded_show(&state, &app, &token, "Dynamic phase modes", &[]).await;
    let definition = dynamic_definition_json(27);
    let original_lane = definition["lanes"][0].clone();
    let (status, created) = post_show_object_intent(
        &app,
        &token,
        &show_id,
        "/api/v2/dynamics/create",
        serde_json::json!({
            "request_id": "phase-mode-create",
            "definition": definition
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    assert_eq!(created["object"]["body"]["phase_mode"], "uniform");
    assert!(created["object"]["body"]["lanes"][0]["phase"].is_null());
    let dynamic_id = created["object"]["id"].as_str().unwrap();

    let (status, per_lane) = post_show_object_intent(
        &app,
        &token,
        &show_id,
        &format!("/api/v2/dynamics/{dynamic_id}/update"),
        serde_json::json!({
            "request_id": "phase-mode-per-lane",
            "expected_revision": 1,
            "intent": {"type": "set_phase_mode", "phase_mode": "per_lane"}
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{per_lane}");
    assert_eq!(per_lane["object"]["body"]["phase_mode"], "per_lane");
    assert_eq!(
        per_lane["object"]["body"]["lanes"][0]["phase"],
        per_lane["object"]["body"]["phase"]
    );

    let (status, uniform) = post_show_object_intent(
        &app,
        &token,
        &show_id,
        &format!("/api/v2/dynamics/{dynamic_id}/update"),
        serde_json::json!({
            "request_id": "phase-mode-uniform",
            "expected_revision": 2,
            "intent": {"type": "set_phase_mode", "phase_mode": "uniform"}
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{uniform}");
    assert_eq!(uniform["object"]["body"]["phase_mode"], "uniform");
    assert_eq!(
        uniform["object"]["body"]["lanes"][0]["phase"], uniform["object"]["body"]["phase"],
        "uniform mode retains per-lane configuration for a later switch back"
    );

    let (status, per_lane_again) = post_show_object_intent(
        &app,
        &token,
        &show_id,
        &format!("/api/v2/dynamics/{dynamic_id}/update"),
        serde_json::json!({
            "request_id": "phase-mode-per-lane-again",
            "expected_revision": 3,
            "intent": {"type": "set_phase_mode", "phase_mode": "per_lane"}
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{per_lane_again}");

    let mut added_lane = original_lane.clone();
    added_lane["id"] = serde_json::json!(Uuid::new_v4());
    let added_lane_id = added_lane["id"].as_str().unwrap().to_owned();
    let (status, added) = post_show_object_intent(
        &app,
        &token,
        &show_id,
        &format!("/api/v2/dynamics/{dynamic_id}/update"),
        serde_json::json!({
            "request_id": "phase-mode-add-lane",
            "expected_revision": 4,
            "intent": {"type": "add_lane", "lane": added_lane, "index": null}
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{added}");
    assert_eq!(
        added["object"]["body"]["lanes"][1]["phase"], added["object"]["body"]["phase"],
        "a lane added in per-lane mode inherits the uniform phase when omitted"
    );

    let mut replacement = original_lane;
    replacement["phase"] = serde_json::Value::Null;
    let (status, replaced) = post_show_object_intent(
        &app,
        &token,
        &show_id,
        &format!("/api/v2/dynamics/{dynamic_id}/update"),
        serde_json::json!({
            "request_id": "phase-mode-replace-lane",
            "expected_revision": 5,
            "intent": {
                "type": "replace_lane",
                "lane_id": added_lane_id,
                "lane": replacement
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{replaced}");
    assert_eq!(
        replaced["object"]["body"]["lanes"][1]["phase"], replaced["object"]["body"]["phase"],
        "a replacement lane in per-lane mode inherits the uniform phase when omitted"
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn dynamic_live_http_is_fire_and_forget_without_request_identity() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Dynamic live replay").await;
    let show_id = show["id"].as_str().unwrap();
    let opened = app
        .clone()
        .oneshot(open_show_request(&token, show_id))
        .await
        .unwrap();
    assert_eq!(opened.status(), StatusCode::OK);
    let fixture = Uuid::new_v4();
    let mut snapshot = light_engine::EngineSnapshot::default();
    snapshot.fixtures = vec![operational_fixture(light_core::FixtureId(fixture))].into();
    state.output.replace_snapshot(snapshot).unwrap();
    let request = serde_json::json!({
        "targets": [fixture],
        "attribute": "intensity",
        "value": 0.35,
        "timing": {}
    });
    let (status, first) = post_show_object_intent(
        &app,
        &token,
        show_id,
        "/api/v2/programmer/values/fix-at",
        request.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert!(first.get("request_id").is_none());
    let (status, second) = post_show_object_intent(
        &app,
        &token,
        show_id,
        "/api/v2/programmer/values/fix-at",
        serde_json::json!({
            "targets": [fixture],
            "attribute": "intensity",
            "value": 0.75,
            "timing": {}
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{second}");
    assert!(second.get("request_id").is_none());

    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn dynamic_http_routes_use_runtime_instance_identity_and_project_authoritative_speed() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let definition = dynamic_definition_json(18);
    let dynamic_id = definition["id"].as_str().unwrap().to_owned();
    let show_id = create_seeded_show(
        &state,
        &app,
        &token,
        "Dynamic HTTP actions",
        &[("dynamic", &dynamic_id, definition)],
    )
    .await;
    let target = Uuid::new_v4();
    let (status, started) = post_show_object_intent(
        &app,
        &token,
        &show_id,
        &format!("/api/v2/dynamics/{dynamic_id}/start"),
        serde_json::json!({
            "targets": [target],
            "timing": {}
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{started}");
    let instance_id = started["runtime_instance_id"].as_str().unwrap();
    for (path, value) in [("size", 0.4), ("speed", 2.0), ("phase", 90.0)] {
        let (status, outcome) = post_show_object_intent(
            &app,
            &token,
            &show_id,
            &format!("/api/v2/dynamic-instances/{instance_id}/{path}"),
            serde_json::json!({
                "value": value
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{outcome}");
        assert_eq!(outcome["changed"], true);
    }

    let response = app
        .clone()
        .oneshot(
            Request::get("/api/v2/dynamics/runtime")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("x-tosk-show", &show_id)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let runtime = json(response).await;
    assert_eq!(runtime["instances"][0]["instance_id"], instance_id);
    assert_eq!(runtime["instances"][0]["speed_source"], "Fixed");
    assert_eq!(runtime["instances"][0]["effective_cycle_millis"], 500);
    assert_eq!(runtime["instances"][0]["controllers"][0]["size"], 0.4);
    assert_eq!(
        runtime["instances"][0]["controllers"][0]["phase_offset_degrees"],
        90.0
    );

    let (status, off) = post_show_object_intent(
        &app,
        &token,
        &show_id,
        &format!("/api/v2/dynamic-instances/{instance_id}/off"),
        serde_json::json!({
            "timing": {}
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{off}");

    for expected_started in [true, false] {
        let (status, toggled) = post_show_object_intent(
            &app,
            &token,
            &show_id,
            &format!("/api/v2/dynamics/{dynamic_id}/toggle"),
            serde_json::json!({
                "targets": [target],
                "timing": {}
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{toggled}");
        assert_eq!(toggled["started"], expected_started);
    }

    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn dynamic_runtime_is_show_scoped_and_restores_exactly_after_show_reload() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let definition = dynamic_definition_json(19);
    let dynamic_id = definition["id"].as_str().unwrap().to_owned();
    let dynamic_show_id = create_seeded_show(
        &state,
        &app,
        &token,
        "Dynamic runtime reload",
        &[("dynamic", &dynamic_id, definition)],
    )
    .await;
    let target = Uuid::new_v4();
    let (status, started) = post_show_object_intent(
        &app,
        &token,
        &dynamic_show_id,
        &format!("/api/v2/dynamics/{dynamic_id}/start"),
        serde_json::json!({
            "targets": [target],
            "timing": {}
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{started}");
    let instance_id = started["runtime_instance_id"].as_str().unwrap();
    for (path, value) in [("size", 0.35), ("speed", 1.5), ("phase", 72.0)] {
        let (status, outcome) = post_show_object_intent(
            &app,
            &token,
            &dynamic_show_id,
            &format!("/api/v2/dynamic-instances/{instance_id}/{path}"),
            serde_json::json!({"value": value}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{outcome}");
    }
    let before_reload = state.output.dynamic_runtime_snapshot();
    assert_eq!(before_reload.instances.len(), 1);

    let empty_show = create_show(&app, &token, "Empty runtime scope").await;
    let empty_show_id = empty_show["id"].as_str().unwrap();
    let opened = app
        .clone()
        .oneshot(open_show_request(&token, empty_show_id))
        .await
        .unwrap();
    assert_eq!(opened.status(), StatusCode::OK);
    assert!(
        state.output.dynamic_runtime_snapshot().instances.is_empty(),
        "a show without persisted Dynamic runtime must not inherit another show's instances"
    );

    let reopened = app
        .clone()
        .oneshot(open_show_request(&token, &dynamic_show_id))
        .await
        .unwrap();
    assert_eq!(reopened.status(), StatusCode::OK);
    assert_eq!(
        state.output.dynamic_runtime_snapshot(),
        before_reload,
        "show reload restores instance identity, controllers, epoch, and local overrides exactly"
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn deleting_a_dynamic_snapshots_cue_and_playback_references_without_old_slot_recapture() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let definition_json = dynamic_definition_json(31);
    let definition: light_dynamics::DynamicDefinition =
        serde_json::from_value(definition_json.clone()).unwrap();
    let dynamic_id = definition.id;
    let fallback = light_dynamics::DynamicDefinitionSnapshot {
        definition: Arc::new(definition.clone()),
    };
    let reference = light_dynamics::DynamicReference {
        dynamic_id: Some(dynamic_id),
        last_known_pool_number: definition.pool_number,
        embedded_fallback: fallback.clone(),
    };
    let fixture = light_core::FixtureId::new();
    let instance_link = Uuid::new_v4();
    let mut cue_list = test_cue_list();
    cue_list.cues[0].dynamic_changes = vec![light_playback::CueDynamicChange {
        fixture_id: fixture,
        attribute: light_core::AttributeKey::intensity(),
        value: light_dynamics::DynamicSemanticValue::DynamicOn {
            instance_link,
            dynamic: reference.clone(),
            lane_id: definition.lanes[0].id,
            overrides: light_dynamics::DynamicInstanceOverrides {
                size: 1.0,
                speed_multiplier: light_dynamics::Rational::ONE,
                phase_offset_degrees: 0.0,
            },
            timing: light_dynamics::DynamicValueTiming::default(),
        },
        automatic_restore: false,
    }];
    let playback_target = light_playback::PlaybackTarget::Dynamic {
        assignment: light_playback::DynamicPlaybackAssignment {
            dynamic: reference,
            revision: 1,
            target_scope: Some(light_playback::DynamicPlaybackTargetScope::FrozenTargets {
                targets: vec![fixture],
            }),
            fader_mode: light_playback::DynamicPlaybackFaderMode::SizeAndMaster,
            priority: 0,
            activation_override: None,
            resume_policy: light_playback::DynamicPlaybackResumePolicy::FollowDynamic,
            local_speed_multiplier: light_dynamics::Rational::ONE,
            learned_duration_millis: None,
            crossfade_non_intensity: false,
            auto_off_at_zero: false,
            auto_off_flash_release: false,
            auto_off_full_control: true,
        },
    };
    let playback = light_playback::PlaybackDefinition {
        number: 1,
        name: "Deleted Dynamic reference".into(),
        buttons: light_playback::PlaybackDefinition::default_buttons(&playback_target),
        button_count: 3,
        fader: light_playback::PlaybackFaderMode::Master,
        has_fader: true,
        go_activates: true,
        auto_off: false,
        xfade_millis: 0,
        color: "#20c997".into(),
        flash_release: light_playback::FlashReleaseMode::ReleaseAll,
        protect_from_swap: false,
        presentation_icon: None,
        presentation_image: None,
        target: playback_target,
    };
    let cue_list_id = cue_list.id.0.to_string();
    let dynamic_id_text = dynamic_id.to_string();
    let show_id = create_seeded_show(
        &state,
        &app,
        &token,
        "Dynamic deletion references",
        &[
            ("dynamic", &dynamic_id_text, definition_json),
            (
                "cue_list",
                &cue_list_id,
                serde_json::to_value(cue_list).unwrap(),
            ),
            ("playback", "1", serde_json::to_value(playback).unwrap()),
        ],
    )
    .await;

    let (status, deleted) = post_show_object_intent(
        &app,
        &token,
        &show_id,
        &format!("/api/v2/dynamics/{dynamic_id}/delete"),
        serde_json::json!({
            "request_id": "delete-dynamic-with-references",
            "expected_revision": 1
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{deleted}");

    let entry = state
        .installation
        .show(light_core::ShowId(Uuid::parse_str(&show_id).unwrap()))
        .unwrap()
        .unwrap();
    let store = ActiveShowRepository::open(&entry.path).unwrap();
    let cue_body = store
        .object_with_portable_revision("cue_list", &cue_list_id)
        .unwrap()
        .1
        .unwrap()
        .body;
    let playback_body = store
        .object_with_portable_revision("playback", "1")
        .unwrap()
        .1
        .unwrap()
        .body;
    for stored_reference in [
        &cue_body["cues"][0]["dynamic_changes"][0]["value"]["dynamic"],
        &playback_body["target"]["assignment"]["dynamic"],
    ] {
        assert!(stored_reference["dynamic_id"].is_null());
        assert_eq!(
            stored_reference["embedded_fallback"]["definition"]["id"],
            dynamic_id_text
        );
        assert_eq!(
            stored_reference["embedded_fallback"]["definition"]["pool_number"],
            31
        );
    }
    assert!(
        store
            .object_with_portable_revision("dynamic", &dynamic_id_text)
            .unwrap()
            .1
            .is_none()
    );
    drop(store);

    let replacement = dynamic_definition_json(31);
    let (status, created) = post_show_object_intent(
        &app,
        &token,
        &show_id,
        "/api/v2/dynamics/create",
        serde_json::json!({
            "request_id": "replace-deleted-dynamic-slot",
            "definition": replacement
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    assert_ne!(created["object"]["id"], dynamic_id_text);
    assert_eq!(created["object"]["body"]["pool_number"], 31);

    let store = ActiveShowRepository::open(&entry.path).unwrap();
    let cue_body = store
        .object_with_portable_revision("cue_list", &cue_list_id)
        .unwrap()
        .1
        .unwrap()
        .body;
    let playback_body = store
        .object_with_portable_revision("playback", "1")
        .unwrap()
        .1
        .unwrap()
        .body;
    assert!(cue_body["cues"][0]["dynamic_changes"][0]["value"]["dynamic"]["dynamic_id"].is_null());
    assert!(playback_body["target"]["assignment"]["dynamic"]["dynamic_id"].is_null());

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn deleting_a_dynamic_snapshots_nested_references_without_touching_other_ids() {
    let deleted = Uuid::new_v4();
    let retained = Uuid::new_v4();
    let fallback: light_dynamics::DynamicDefinitionSnapshot =
        serde_json::from_value(serde_json::json!({
            "definition": dynamic_definition_json(1)
        }))
        .unwrap();
    let mut body = serde_json::json!({
        "cues": [{
            "dynamic_changes": [
                {"value": {"dynamic_id": deleted, "embedded_fallback": fallback}},
                {"value": {"dynamic_id": retained, "embedded_fallback": fallback}}
            ]
        }]
    });
    assert!(
        super::super::show_object_intents_v2::snapshot_deleted_dynamic_references(
            &mut body, deleted, &fallback
        )
    );
    assert!(body["cues"][0]["dynamic_changes"][0]["value"]["dynamic_id"].is_null());
    assert_eq!(
        body["cues"][0]["dynamic_changes"][1]["value"]["dynamic_id"],
        retained.to_string()
    );
}

fn dynamic_definition_json(pool_number: u16) -> serde_json::Value {
    serde_json::json!({
        "id": Uuid::new_v4(),
        "pool_number": pool_number,
        "revision": 1,
        "name": "Intensity wave",
        "color": null,
        "icon": null,
        "target_binding": {"type": "targetless"},
        "lanes": [{
            "id": Uuid::new_v4(),
            "attribute": "intensity",
            "mode": "keyframes",
            "keyframes": {
                "points": [
                    {"position": 0.0, "source": {"type": "value", "value": 0.0}, "interpolation": "linear"},
                    {"position": 0.5, "source": {"type": "value", "value": 1.0}, "interpolation": "linear"}
                ],
                "size": 1.0
            },
            "max_min": {
                "minimum": {"type": "value", "value": 0.0},
                "maximum": {"type": "value", "value": 1.0},
                "function": "sinus",
                "size": 1.0,
                "pwm": {
                    "attack": 0.0, "on": 0.5, "decay": 0.0, "off": 0.5,
                    "attack_interpolation": "linear", "decay_interpolation": "linear"
                }
            },
            "middle_amplitude": {
                "middle": {"type": "current"},
                "amplitude": 0.5,
                "function": "sinus",
                "size": 1.0,
                "pwm": {
                    "attack": 0.0, "on": 0.5, "decay": 0.0, "off": 0.5,
                    "attack_interpolation": "linear", "decay_interpolation": "linear"
                }
            },
            "speed_multiplier": {"numerator": 1, "denominator": 1},
            "width": 1.0,
            "random_group_id": null
        }],
        "random_groups": [],
        "phase": {
            "ordering": {"type": "selection"},
            "offset_degrees": 0.0,
            "span_degrees": 360.0,
            "block_size": 1,
            "repeats": 1,
            "wings": false,
            "anchors_degrees": []
        },
        "speed": {"type": "fixed", "duration_millis": 1000},
        "default_activation": "start_now",
        "future_definition_field": "preserved"
    })
}

#[tokio::test]
async fn layout_and_patch_intents_preserve_unknown_fields_and_replay_once() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let user_id = state
        .sessions
        .sessions()
        .into_iter()
        .next()
        .unwrap()
        .user
        .id
        .0
        .to_string();
    let show_id = create_seeded_show(
        &state,
        &app,
        &token,
        "Typed layout and patch",
        &[
            (
                "user_layout",
                &user_id,
                serde_json::json!({
                    "desks": [],
                    "activeDeskId": "old",
                    "future_layout_field": {"kept": true}
                }),
            ),
            (
                "patch_layer",
                "front",
                serde_json::json!({
                    "id": "front",
                    "name": "Old",
                    "order": 1,
                    "future_patch_field": "kept"
                }),
            ),
        ],
    )
    .await;

    let layout = serde_json::json!({
        "request_id": "save-layout-1",
        "action": {
            "type": "update",
            "expected_revision": 1,
            "patch": {
                "desks": [{"id": "main"}],
                "active_desk_id": "main"
            }
        }
    });
    let before_application_events = state.events.latest_sequence();
    let (status, saved_layout) = post_show_object_intent(
        &app,
        &token,
        &show_id,
        &format!("/api/v2/user-layouts/{user_id}/update"),
        layout.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(saved_layout["object"]["revision"], 2);
    assert!(saved_layout["event_sequence"].as_u64().is_some());
    assert_eq!(
        saved_layout["object"]["body"]["future_layout_field"]["kept"],
        true
    );
    assert_eq!(saved_layout["object"]["body"]["activeDeskId"], "main");
    let show_filter = light_application::EventFilter::default()
        .with_capability(light_application::EventCapability::Show);
    let light_application::EventReplay::Events(layout_events) =
        state.events.replay(before_application_events, &show_filter)
    else {
        panic!("the focused user-layout Show event must remain replayable")
    };
    assert_eq!(layout_events.len(), 1);
    assert_eq!(saved_layout["event_sequence"], layout_events[0].sequence);
    let after_layout_application_event = state.events.latest_sequence();

    let (status, replayed) = post_show_object_intent(
        &app,
        &token,
        &show_id,
        &format!("/api/v2/user-layouts/{user_id}/update"),
        layout,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(replayed["replayed"], true);
    assert_eq!(replayed["object"], saved_layout["object"]);
    let light_application::EventReplay::Events(replayed_layout_events) =
        state.events.replay(before_application_events, &show_filter)
    else {
        panic!("the focused user-layout Show event must remain replayable")
    };
    assert_eq!(replayed_layout_events.len(), 1);

    let (status, saved_layer) = post_show_object_intent(
        &app,
        &token,
        &show_id,
        "/api/v2/patch/layers/front/update",
        serde_json::json!({
            "request_id": "save-layer-1",
            "action": {
                "type": "save",
                "expected_revision": 1,
                "layer": {"name": "Front truss", "order": 2}
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(saved_layer["object"]["revision"], 2);
    assert!(saved_layer["event_sequence"].as_u64().is_some());
    assert_eq!(saved_layer["object"]["body"]["name"], "Front truss");
    assert_eq!(saved_layer["object"]["body"]["future_patch_field"], "kept");
    let light_application::EventReplay::Events(layer_events) = state
        .events
        .replay(after_layout_application_event, &show_filter)
    else {
        panic!("the focused Patch-layer Show event must remain replayable")
    };
    assert_eq!(layer_events.len(), 1);
    assert_eq!(saved_layer["event_sequence"], layer_events[0].sequence);

    let (status, stale) = post_show_object_intent(
        &app,
        &token,
        &show_id,
        "/api/v2/patch/layers/front/update",
        serde_json::json!({
            "request_id": "stale-layer",
            "action": {
                "type": "save",
                "expected_revision": 1,
                "layer": {"name": "Stale", "order": 3}
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(stale["error"].as_str().unwrap().contains("revision"));

    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn preload_record_intent_stores_the_pending_scene_and_replays_once() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, session_id) = login(&app, "Operator").await;
    let show_id = create_seeded_show(&state, &app, &token, "Typed preload", &[]).await;
    let session_id = light_core::SessionId(Uuid::parse_str(&session_id).unwrap());
    assert!(state.programming.set_preload_group(
        session_id,
        "front".into(),
        light_core::AttributeKey::intensity(),
        light_core::AttributeValue::Normalized(0.6),
    ));
    let action = serde_json::json!({
        "request_id": "record-preload-1",
        "action": {
            "type": "preset",
            "target_id": "1.1",
            "expected_revision": 0,
            "name": "Front at sixty",
            "mode": "merge",
            "family": "intensity"
        }
    });

    let (status, stored) = post_show_object_intent(
        &app,
        &token,
        &show_id,
        "/api/v2/preload/record",
        action.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(stored["object"]["kind"], "preset");
    assert_eq!(stored["object"]["id"], "1.1");
    assert_eq!(stored["object"]["revision"], 1);
    assert_eq!(stored["object"]["body"]["name"], "Front at sixty");
    let intensity = &stored["object"]["body"]["group_values"]["front"]["intensity"];
    assert_eq!(intensity["kind"], "normalized");
    assert!(
        (intensity["value"].as_f64().unwrap() - 0.6).abs() < 0.000_001,
        "stored preload intensity should retain the programmer value"
    );

    let (status, replayed) =
        post_show_object_intent(&app, &token, &show_id, "/api/v2/preload/record", action).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(replayed["replayed"], true);
    assert_eq!(replayed["object"], stored["object"]);

    let _ = std::fs::remove_dir_all(data_dir);
}

fn test_cue_list() -> light_playback::CueList {
    light_playback::CueList {
        id: light_core::CueListId::new(),
        name: "Main".into(),
        priority: 0,
        mode: light_playback::CueListMode::Sequence,
        looped: false,
        chaser_step_millis: 1_000,
        speed_group: None,
        intensity_priority_mode: light_playback::IntensityPriorityMode::Htp,
        wrap_mode: Some(light_playback::WrapMode::Off),
        restart_mode: light_playback::RestartMode::FirstCue,
        force_cue_timing: false,
        disable_cue_timing: false,
        chaser_xfade_millis: 0,
        chaser_xfade_percent: Some(0),
        speed_multiplier: 1.0,
        cues: vec![light_playback::Cue::new(1.0)],
    }
}
