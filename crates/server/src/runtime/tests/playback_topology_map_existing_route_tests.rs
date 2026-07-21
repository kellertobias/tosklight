use super::{playback_topology_route_support::*, *};

#[tokio::test]
async fn map_existing_is_a_page_only_action_with_replay_no_change_and_conflict() {
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
    scenario.seed("playback", "legacy-twelve", &source);
    scenario.seed(
        "playback_page",
        "legacy-page-two",
        &serde_json::json!({
            "number":2,"name":"Wing","slots":{},"future_page":{"columns":8}
        }),
    );
    let revision = scenario.show_revision();
    let cursor = scenario.state.application_events.latest_sequence();
    let mut compatibility = scenario.state.events.subscribe();
    let request = map_existing_request(
        "map-existing",
        1,
        Some("legacy-page-two"),
        1,
        Some("legacy-twelve"),
    );

    let response = scenario.action(revision, request.clone()).await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_etag(&response, revision + 1);
    let changed = json(response).await;
    assert_changed_map(&changed);
    assert_one_topology_event(&scenario.state, cursor, 1);
    let compatibility_events =
        std::iter::from_fn(|| compatibility.try_recv().ok()).collect::<Vec<_>>();
    assert_eq!(compatibility_events.len(), 1);
    assert_source_unchanged(&scenario);

    assert_replay(&scenario, revision, cursor, request, &changed).await;
    let page_revision = projection_revision(&changed, "playback_page");
    assert_no_change(&scenario, revision + 1, page_revision).await;
    assert_conflict(&scenario, revision + 1, page_revision).await;
    assert_eq!(
        scenario.state.application_events.latest_sequence(),
        cursor + 1
    );
    scenario.cleanup();
}

fn assert_changed_map(changed: &serde_json::Value) {
    assert_eq!(changed["status"], "changed");
    assert_eq!(changed["resolution"]["playback_number"], 12);
    assert_eq!(changed["objects"].as_array().unwrap().len(), 1);
    assert_eq!(changed["objects"][0]["kind"], "playback_page");
    assert_eq!(changed["objects"][0]["object_id"], "legacy-page-two");
    assert_eq!(changed["objects"][0]["body"]["slots"]["4"], 12);
    assert_eq!(changed["objects"][0]["body"]["future_page"]["columns"], 8);
}

fn assert_source_unchanged(scenario: &TopologyScenario) {
    let document = scenario.document();
    let source = document.object("playback", "legacy-twelve").unwrap();
    assert_eq!(source.revision(), 1);
    assert_eq!(source.body()["future_playback"]["retained"], true);
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
    assert_eq!(
        scenario.state.application_events.latest_sequence(),
        cursor + 1
    );
}

async fn assert_no_change(scenario: &TopologyScenario, revision: u64, page_revision: u64) {
    let response = scenario
        .action(
            revision,
            map_existing_request(
                "map-existing-no-change",
                page_revision,
                Some("legacy-page-two"),
                1,
                Some("legacy-twelve"),
            ),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let outcome = json(response).await;
    assert_eq!(outcome["status"], "no_change");
    assert!(outcome.get("event_sequence").is_none());
    assert_eq!(outcome["objects"].as_array().unwrap().len(), 1);
}

async fn assert_conflict(scenario: &TopologyScenario, revision: u64, page_revision: u64) {
    let response = scenario
        .action(
            revision,
            map_existing_request(
                "map-existing-conflict",
                page_revision,
                Some("legacy-page-two"),
                0,
                Some("legacy-twelve"),
            ),
        )
        .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(json(response).await["current_related_revision"], 1);
}
