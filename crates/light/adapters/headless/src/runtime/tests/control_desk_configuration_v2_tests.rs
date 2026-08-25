use super::playback_topology_page_route_tests::{desk_page, scenario_desk_id, scenario_show_id};
use super::playback_topology_route_support::{TopologyScenario, configure_request};
use super::*;

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
    let changed = post_desk_action(&scenario, desk_id, update.clone()).await;
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

    let replay = post_desk_action(&scenario, desk_id, update).await;
    assert_eq!(replay.status(), StatusCode::OK);
    assert_eq!(json(replay).await["replayed"], true);
    assert_eq!(scenario.state.events.audit_revision(), event_after);
    let collision = post_desk_action(
        &scenario,
        desk_id,
        serde_json::json!({
            "request_id":"desk-update",
            "action":{"type":"set_page","page":1,"existing_only":true}
        }),
    )
    .await;
    assert_eq!(collision.status(), StatusCode::CONFLICT);

    let foreign = post_desk_action(
        &scenario,
        Uuid::new_v4(),
        serde_json::json!({
            "request_id":"foreign",
            "action":{"type":"update","patch":{"name":"No"}}
        }),
    )
    .await;
    assert_eq!(foreign.status(), StatusCode::FORBIDDEN);

    let configured = scenario
        .action(
            scenario.show_revision(),
            configure_request("seed-page", 0, 0),
        )
        .await;
    assert_eq!(configured.status(), StatusCode::OK);
    let missing = post_desk_action(&scenario, desk_id, page_action("missing-page", 2, true)).await;
    assert_eq!(missing.status(), StatusCode::BAD_REQUEST);
    let created = post_desk_action(&scenario, desk_id, page_action("create-page", 2, false)).await;
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
    let replayed_page =
        post_desk_action(&scenario, desk_id, page_action("create-page", 2, false)).await;
    assert_eq!(json(replayed_page).await["replayed"], true);
    assert_eq!(scenario.state.events.audit_revision(), page_event);

    scenario.cleanup();
}

async fn post_desk_action(
    scenario: &TopologyScenario,
    desk_id: Uuid,
    body: serde_json::Value,
) -> Response {
    scenario
        .app
        .clone()
        .oneshot(
            Request::post(format!("/api/v2/control-desks/{desk_id}/actions"))
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
