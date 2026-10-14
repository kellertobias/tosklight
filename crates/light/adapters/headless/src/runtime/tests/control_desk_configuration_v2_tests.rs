use super::playback_topology_page_route_tests::{desk_page, scenario_desk_id, scenario_show_id};
use super::playback_topology_route_support::{TopologyScenario, configure_request};
use super::*;

#[tokio::test]
async fn hardware_illumination_patch_is_sparse_replayed_and_validated() {
    let scenario = TopologyScenario::new("Hardware illumination").await;
    let update = serde_json::json!({"request_id":"illumination","action":{"type":"update","patch":{"hardware_led_brightness":0,"hardware_gooseneck_brightness":45,"hardware_gooseneck_color":25}}});
    let changed = post_desk_action(&scenario, update.clone()).await;
    assert_eq!(changed.status(), StatusCode::OK);
    let changed = json(changed).await;
    assert_eq!(changed["desk"]["hardware_led_brightness"], 0);
    assert_eq!(changed["desk"]["hardware_gooseneck_brightness"], 45);
    assert_eq!(changed["desk"]["hardware_gooseneck_color"], 25);
    let replay = json(post_desk_action(&scenario, update).await).await;
    assert_eq!(replay["replayed"], true);
    let sparse = json(post_desk_action(&scenario, serde_json::json!({"request_id":"illumination-sparse","action":{"type":"update","patch":{"hardware_led_brightness":80}}})).await).await;
    assert_eq!(sparse["desk"]["hardware_gooseneck_brightness"], 45);
    assert_eq!(sparse["desk"]["hardware_gooseneck_color"], 25);
    for field in [
        "hardware_led_brightness",
        "hardware_gooseneck_brightness",
        "hardware_gooseneck_color",
    ] {
        let invalid = post_desk_action(&scenario, serde_json::json!({"request_id":format!("invalid-{field}"),"action":{"type":"update","patch":{(field):101,"name":"must not change"}}})).await;
        assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    }
    let desk = scenario.state.installation.desk().unwrap();
    assert_eq!(desk.hardware_led_brightness, 80);
    assert_ne!(desk.name, "must not change");
    assert_eq!(
        crate::runtime::osc_feedback_broadcast::illumination_arguments(&desk),
        vec![
            OscArgument::Int(80),
            OscArgument::Int(45),
            OscArgument::Int(25)
        ]
    );
    scenario.cleanup();
}

#[tokio::test]
async fn control_desk_v2_is_sparse_replay_safe_authorized_and_retires_v1() {
    let scenario = TopologyScenario::new("Control desk configuration v2").await;
    let desk_id = scenario_desk_id(&scenario);
    let original = scenario.state.installation.desk().unwrap();
    let update = serde_json::json!({
        "request_id":"desk-update",
        "future_request_field":true,
        "action":{
            "type":"update",
            "future_action_field":"accepted",
            "patch":{"name":"Front desk renamed","show_compact_encoders":true,"future_patch_field":42}
        }
    });
    let event_before = scenario.state.events.audit_revision();
    let changed = post_desk_action(&scenario, update.clone()).await;
    assert_eq!(changed.status(), StatusCode::OK);
    let changed = json(changed).await;
    assert_eq!(changed["replayed"], false);
    assert_eq!(changed["desk"]["name"], "Front desk renamed");
    assert_eq!(changed["desk"]["show_compact_encoders"], true);
    assert_eq!(changed["desk"]["columns"], original.columns);
    assert_eq!(
        scenario
            .state
            .sessions
            .sessions()
            .into_iter()
            .next()
            .unwrap()
            .desk
            .name,
        "Front desk renamed"
    );
    let event_after = scenario.state.events.audit_revision();
    assert_eq!(event_after, event_before + 1);

    let replay = post_desk_action(&scenario, update).await;
    assert_eq!(replay.status(), StatusCode::OK);
    assert_eq!(json(replay).await["replayed"], true);
    assert_eq!(scenario.state.events.audit_revision(), event_after);
    let collision = post_desk_action(
        &scenario,
        serde_json::json!({
            "request_id":"desk-update",
            "action":{"type":"update","patch":{"name":"collision"}}
        }),
    )
    .await;
    assert_eq!(collision.status(), StatusCode::CONFLICT);

    // The route named a desk while there could be several, and refused a caller whose session
    // stood at a different one. A caller still sending an id addresses nothing.
    let retired = scenario
        .app
        .clone()
        .oneshot(
            Request::post(format!("/api/v2/control-desks/{}/actions", Uuid::new_v4()))
                .header(header::AUTHORIZATION, format!("Bearer {}", scenario.token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "request_id":"retired",
                        "action":{"type":"update","patch":{"name":"No"}}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(retired.status(), StatusCode::NOT_FOUND);

    let configured = scenario
        .action(
            scenario.show_revision(),
            configure_request("seed-page", 0, 0),
        )
        .await;
    assert_eq!(configured.status(), StatusCode::OK);
    // Page choice is a live surface action: it can select an unassigned sparse page
    // without creating portable show data or entering object-intent replay machinery.
    let show_revision = scenario.show_revision();
    let created = scenario
        .app
        .clone()
        .oneshot(
            Request::post("/api/v2/playback-surfaces/main/page")
                .header(header::AUTHORIZATION, format!("Bearer {}", scenario.token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::json!({"page":2}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::OK);
    let created = json(created).await;
    assert_eq!(created["command_page"], 2);
    assert!(created.get("replayed").is_none());
    assert_eq!(
        desk_page(&scenario, desk_id, scenario_show_id(&scenario)),
        2
    );
    assert_eq!(scenario.show_revision(), show_revision);
    assert!(
        !scenario
            .state
            .output
            .snapshot()
            .playback_pages
            .iter()
            .any(|page| page.number == 2)
    );

    scenario.cleanup();
}

async fn post_desk_action(scenario: &TopologyScenario, body: serde_json::Value) -> Response {
    scenario
        .app
        .clone()
        .oneshot(
            Request::post("/api/v2/control-desk/update")
                .header(header::AUTHORIZATION, format!("Bearer {}", scenario.token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn nullable_configuration_http_clear_survives_persisted_reload() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let send = |body: serde_json::Value| {
        Request::post("/api/v2/configuration/update")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    };
    let original = state.installation.configuration();
    let set = serde_json::json!({"request_id":"nullable-set", "patch":{
        "timecode_frame_rate":{"numerator":25,"denominator":1,"drop_frame":false},
        "timecode_audio_output_device":"Desk USB Out",
        "future_nullable_setting":true
    }});
    assert_eq!(
        app.clone().oneshot(send(set)).await.unwrap().status(),
        StatusCode::OK
    );
    assert!(
        state
            .installation
            .configuration()
            .timecode_frame_rate
            .is_some()
    );
    let clear =
        serde_json::json!({"request_id":"nullable-clear", "patch":{"timecode_frame_rate":null}});
    let result = app.clone().oneshot(send(clear.clone())).await.unwrap();
    assert_eq!(result.status(), StatusCode::OK);
    assert_eq!(json(result).await["replayed"], false);
    let current = state.installation.configuration();
    assert!(current.timecode_frame_rate.is_none());
    assert_eq!(
        current.timecode_audio_output_device.as_deref(),
        Some("Desk USB Out")
    );
    assert_eq!(current.osc_bind, original.osc_bind);
    assert_eq!(current.art_timecode_bind, original.art_timecode_bind);
    assert_eq!(current.frame_rate_hz, original.frame_rate_hz);
    let persisted = state
        .installation
        .setting("server_configuration")
        .unwrap()
        .unwrap();
    let persisted_value: serde_json::Value = serde_json::from_str(&persisted).unwrap();
    assert_eq!(
        persisted_value["timecode_frame_rate"],
        serde_json::Value::Null
    );
    let reloaded = startup_state::load_configuration(
        &DeskStore::open(data_dir.join("desk.sqlite")).unwrap(),
        None,
        None,
    )
    .unwrap();
    assert!(reloaded.timecode_frame_rate.is_none());
    state.installation.replace_configuration(reloaded);
    let read = app
        .clone()
        .oneshot(
            Request::get("/api/v2/configuration")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(read.status(), StatusCode::OK);
    assert_eq!(
        json(read).await["configuration"]["timecode_frame_rate"],
        serde_json::Value::Null
    );
    assert_eq!(
        json(app.clone().oneshot(send(clear)).await.unwrap()).await["replayed"],
        true
    );
    let before_invalid = serde_json::to_value(state.installation.configuration()).unwrap();
    for (index, invalid) in [
        serde_json::json!("25"),
        serde_json::json!({"numerator":25,"denominator":0,"drop_frame":false}),
    ]
    .into_iter()
    .enumerate()
    {
        let response = app.clone().oneshot(send(serde_json::json!({"request_id":format!("nullable-invalid-{index}"),"patch":{"timecode_frame_rate":invalid,"backup_retention":99}}))).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            serde_json::to_value(state.installation.configuration()).unwrap(),
            before_invalid
        );
    }
    let unauthorized = app.oneshot(Request::post("/api/v2/configuration/update")
        .header(header::CONTENT_TYPE,"application/json")
        .body(Body::from(serde_json::json!({"request_id":"nullable-unauthorized","patch":{"timecode_audio_output_device":null}}).to_string())).unwrap()).await.unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        serde_json::to_value(state.installation.configuration()).unwrap(),
        before_invalid
    );
    let _ = std::fs::remove_dir_all(data_dir);
}
