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
    for field in ["hardware_led_brightness", "hardware_gooseneck_brightness", "hardware_gooseneck_color"] {
        let invalid = post_desk_action(&scenario, serde_json::json!({"request_id":format!("invalid-{field}"),"action":{"type":"update","patch":{(field):101,"name":"must not change"}}})).await;
        assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    }
    let desk = scenario.state.installation.desk().unwrap();
    assert_eq!(desk.hardware_led_brightness, 80);
    assert_ne!(desk.name, "must not change");
    assert_eq!(crate::runtime::osc_feedback_broadcast::illumination_arguments(&desk), vec![OscArgument::Int(80), OscArgument::Int(45), OscArgument::Int(25)]);
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
            "patch":{"name":"Front desk renamed","future_patch_field":42}
        }
    });
    let event_before = scenario.state.events.audit_revision();
    let changed = post_desk_action(&scenario, update.clone()).await;
    assert_eq!(changed.status(), StatusCode::OK);
    let changed = json(changed).await;
    assert_eq!(changed["replayed"], false);
    assert_eq!(changed["desk"]["name"], "Front desk renamed");
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
            "action":{"type":"set_page","page":1,"existing_only":true}
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
    let missing = post_desk_action(&scenario, page_action("missing-page", 2, true)).await;
    assert_eq!(missing.status(), StatusCode::BAD_REQUEST);
    let created = post_desk_action(&scenario, page_action("create-page", 2, false)).await;
    assert_eq!(created.status(), StatusCode::OK);
    let created = json(created).await;
    assert_eq!(created["page"], 2);
    assert!(created["event_sequence"].is_number());
    assert!(created["page_creation_event_sequence"].is_number());
    assert_eq!(
        desk_page(&scenario, desk_id, scenario_show_id(&scenario)),
        2
    );
    let page_event = scenario.state.events.audit_revision();
    let replayed_page = post_desk_action(&scenario, page_action("create-page", 2, false)).await;
    assert_eq!(json(replayed_page).await["replayed"], true);
    assert_eq!(scenario.state.events.audit_revision(), page_event);

    scenario.cleanup();
}

async fn post_desk_action(scenario: &TopologyScenario, body: serde_json::Value) -> Response {
    scenario
        .app
        .clone()
        .oneshot(
            Request::post("/api/v2/control-desk/actions")
                .header(header::AUTHORIZATION, format!("Bearer {}", scenario.token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

fn page_action(request_id: &str, page: u8, existing_only: bool) -> serde_json::Value {
    serde_json::json!({
        "request_id":request_id,
        "action":{"type":"set_page","page":page,"existing_only":existing_only}
    })
}
