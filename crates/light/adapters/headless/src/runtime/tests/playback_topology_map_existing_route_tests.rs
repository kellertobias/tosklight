use super::{playback_topology_route_support::*, *};

#[tokio::test]
async fn copying_definition_changes_one_assignment_with_replay_no_change_and_stale_guard() {
    let scenario = TopologyScenario::new("Map existing Playback").await;
    let cue_list = cue_list("Mapped source");
    let cue_list_id = cue_list.id;
    scenario.seed(
        "cue_list",
        &cue_list_id.0.to_string(),
        &serde_json::to_value(cue_list).unwrap(),
    );
    let mut source = cue_list_playback_body(12, cue_list_id);
    source["future_playback"] = serde_json::json!({"retained": true});
    scenario.seed(
        "playback_page",
        "1",
        &serde_json::json!({"number":1,"name":"Source","assignments":{"12":source}}),
    );
    scenario.seed(
        "playback_page",
        "legacy-page-two",
        &serde_json::json!({
            "number":2,
            "name":"Wing",
            "assignments":{},
            "future_page":{"columns":8}
        }),
    );
    let revision = scenario.show_revision();
    let cursor = scenario.state.events.latest_sequence();
    let request = copy_definition_request(&scenario, "copy-definition", 1);

    let response = scenario.action(revision, request.clone()).await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_etag(&response, revision + 1);
    let changed = json(response).await;
    assert_changed_map(&changed);
    assert_one_topology_event(&scenario.state, cursor, 1);
    assert_source_unchanged(&scenario);

    assert_replay(&scenario, revision, cursor, request, &changed).await;
    let page_revision = projection_revision(&changed, "playback_page");
    assert_no_change(&scenario, revision + 1, page_revision).await;
    assert_conflict(&scenario, revision + 1, page_revision).await;
    assert_eq!(scenario.state.events.latest_sequence(), cursor + 1);
    scenario.cleanup();
}

fn assert_changed_map(changed: &serde_json::Value) {
    assert_eq!(changed["status"], "changed");
    assert_eq!(
        serde_json::json!({"page":changed["resolution"]["page"],"position":changed["resolution"]["position"]}),
        serde_json::json!({"page":2,"position":4})
    );
    assert_eq!(changed["objects"].as_array().unwrap().len(), 1);
    assert_eq!(changed["objects"][0]["kind"], "playback_page");
    assert_eq!(changed["objects"][0]["object_id"], "legacy-page-two");
    assert_eq!(
        changed["objects"][0]["body"]["assignments"]["4"]["name"],
        "Cuelist 12"
    );
    assert_eq!(changed["objects"][0]["body"]["future_page"]["columns"], 8);
}

fn assert_source_unchanged(scenario: &TopologyScenario) {
    let document = scenario.document();
    let source = document.object("playback_page", "1").unwrap();
    assert_eq!(source.revision(), 1);
    assert_eq!(
        source.body()["assignments"]["12"]["future_playback"]["retained"],
        true
    );
}

async fn assert_replay(
    scenario: &TopologyScenario,
    revision: u64,
    cursor: u64,
    request: serde_json::Value,
    changed: &serde_json::Value,
) {
    let response = scenario.action(revision, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let replay = json(response).await;
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["event_sequence"], changed["event_sequence"]);
    assert_eq!(scenario.state.events.latest_sequence(), cursor + 1);
}

async fn assert_no_change(scenario: &TopologyScenario, revision: u64, page_revision: u64) {
    let mut identical = copy_definition_request(scenario, "copy-no-change", page_revision);
    identical["action"]["type"] = serde_json::json!("update_playback");
    let response = scenario.action(revision, identical).await;
    let status = response.status();
    let outcome = json(response).await;
    assert_eq!(status, StatusCode::OK, "{outcome}");
    assert_eq!(outcome["status"], "no_change");
    assert!(outcome.get("event_sequence").is_none());
    assert_eq!(outcome["objects"].as_array().unwrap().len(), 1);
}

async fn assert_conflict(scenario: &TopologyScenario, revision: u64, page_revision: u64) {
    let response = scenario
        .action(
            revision,
            copy_definition_request(scenario, "copy-conflict", page_revision - 1),
        )
        .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        json(response).await["current_related_revision"],
        page_revision
    );
}

fn copy_definition_request(
    scenario: &TopologyScenario,
    request_id: &str,
    page_revision: u64,
) -> serde_json::Value {
    let definition = scenario
        .document()
        .object("playback_page", "1")
        .unwrap()
        .body()["assignments"]["12"]
        .clone();
    serde_json::json!({"request_id":request_id,"action":{"type":"assign_playback",
        "address":{"page":2,"position":4},"expected_page_revision":page_revision,
        "expected_page_object_id":"legacy-page-two","definition":definition}})
}
