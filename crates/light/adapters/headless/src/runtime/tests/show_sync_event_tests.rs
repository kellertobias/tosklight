//! The Control → Architect sync feed: every commit path, bounded bodies, gaps, no live frames.
use super::show_patch_route_tests::install_patch_route_profile;
use super::show_sync_support::*;
use super::*;
use light_application::{
    ShowEvent,
    show_sync::{ShowSyncCommitChange, ShowSyncGapReason},
};

fn committed(events: &[ShowEvent]) -> Vec<&ShowSyncCommitChange> {
    events
        .iter()
        .filter_map(|event| match event {
            ShowEvent::SyncCommitted(change) => Some(change.as_ref()),
            _ => None,
        })
        .collect()
}

fn gaps(events: &[ShowEvent]) -> Vec<ShowSyncGapReason> {
    events
        .iter()
        .filter_map(|event| match event {
            ShowEvent::SyncGap(gap) => Some(gap.reason),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn a_desk_ui_object_edit_reaches_the_feed_exactly_once_with_its_body() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let _feed = subscribe_sync_feed(&state);
    let (token, _) = login(&app, "Operator").await;
    open_sync_show(&app, &token, "Feed desk edit").await;
    let cursor = sync_cursor(&state);
    let desk_main_cursor = state.events.latest_sequence();
    let previous = active_revision(&state);
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v2/patch/layers/front/update")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({"request_id": "desk-layer", "action": {"type": "save",
                        "expected_revision": 0, "layer": {"name": "Front", "order": 2}}})
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let events = sync_events(&state, cursor);
    let commits = committed(&events);
    assert_eq!(commits.len(), 1, "{events:?}");
    let commit = commits[0];
    assert_eq!(commit.previous_show_revision, previous);
    assert_eq!(commit.show_revision, previous + 1);
    assert_eq!(
        commit.request_id, None,
        "a desk edit is not an echo of any sync request"
    );
    assert_eq!(commit.objects.len(), 1);
    assert_eq!(commit.objects[0].kind, "patch_layer");
    assert_eq!(
        commit.objects[0].body.as_ref().unwrap()["name"],
        serde_json::json!("Front")
    );
    assert!(gaps(&events).is_empty());
    let light_application::EventReplay::Events(default_stream) = state
        .events
        .replay(desk_main_cursor, &light_application::EventFilter::default())
    else {
        panic!("the default stream remains replayable")
    };
    assert_eq!(
        default_stream.len(),
        1,
        "a subscription that did not opt in still sees one event per commit"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn a_sync_commit_names_its_request_so_the_sender_recognises_the_echo() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let _feed = subscribe_sync_feed(&state);
    let (token, _) = login(&app, "Architect").await;
    let show_id = open_sync_show(&app, &token, "Feed echo").await;
    let cursor = sync_cursor(&state);
    let association = Uuid::new_v4();
    let body = sync_body(
        "echo",
        association,
        &show_id,
        serde_json::json!([{"type": "create_object", "kind": "visualizer_input", "id": "dmx",
            "body": {"universe": 1}},
            {"type": "set_metadata", "key": "previs.show_version", "value": "v2"}]),
    );
    assert_eq!(
        post_sync(&app, &token, &body).await.status(),
        StatusCode::OK
    );
    let events = sync_events(&state, cursor);
    let commits = committed(&events);
    assert_eq!(commits.len(), 1);
    assert_eq!(commits[0].request_id.as_deref(), Some("echo"));
    assert_eq!(commits[0].association_id, Some(association));
    assert_eq!(
        commits[0].metadata,
        vec![("previs.show_version".to_owned(), Some("v2".to_owned()))]
    );
    // The retry commits nothing and so announces nothing.
    let cursor = sync_cursor(&state);
    assert_eq!(
        post_sync(&app, &token, &body).await.status(),
        StatusCode::OK
    );
    assert!(sync_events(&state, cursor).is_empty());
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn patch_commits_reach_the_feed_and_live_control_never_does() {
    let (state, data_dir) = test_state();
    let (profile_id, mode_id) = install_patch_route_profile(&state);
    let app = router(state.clone());
    let _feed = subscribe_sync_feed(&state);
    let (token, _) = login(&app, "Operator").await;
    let show_id = open_sync_show(&app, &token, "Feed patch").await;
    let cursor = sync_cursor(&state);
    let fixture_id = Uuid::new_v4();
    let patched = app
        .clone()
        .oneshot(
            Request::post("/api/v2/patch/fixtures")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::IF_MATCH, "\"0\"")
                .header("x-tosk-show", &show_id)
                .body(Body::from(
                    serde_json::json!({"request_id": "desk-patch", "fixtures": [{
                        "fixture_id": fixture_id, "fixture_number": 1,
                        "virtual_fixture_number": null, "name": "Desk spot",
                        "profile_id": profile_id, "profile_revision": 1, "mode_id": mode_id,
                        "split_patches": [{"split": 1, "universe": 1, "address": 1}],
                        "layer_id": "default", "direct_control": null,
                        "location": {"x": 0, "y": 0, "z": 0},
                        "rotation": {"x": 0.0, "y": 0.0, "z": 0.0}, "multipatch": [],
                        "move_in_black_enabled": true, "move_in_black_delay_millis": 0,
                        "highlight_overrides": []}]})
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(patched.status(), StatusCode::OK);
    let events = sync_events(&state, cursor);
    let commits = committed(&events);
    assert_eq!(commits.len(), 1);
    assert!(
        commits[0]
            .objects
            .iter()
            .any(|object| object.kind == "patched_fixture" && object.id == fixture_id.to_string())
    );
    assert_eq!(
        commits[0].profile_revisions.len(),
        1,
        "the profile travels as a reference"
    );

    let cursor = sync_cursor(&state);
    let selection = app
        .clone()
        .oneshot(
            Request::post("/api/v2/programming-selection/actions")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({"request_id": "live-select", "action": "replace",
                        "fixtures": [fixture_id], "expected_revision": 0})
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(selection.status(), StatusCode::OK);
    assert!(
        sync_events(&state, cursor).is_empty(),
        "live control is never part of the sync feed"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn bulk_commits_oversized_bodies_and_out_of_band_writes_become_gaps() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let _feed = subscribe_sync_feed(&state);
    let (token, _) = login(&app, "Architect").await;
    let show_id = open_sync_show(&app, &token, "Feed gaps").await;
    let association = Uuid::new_v4();
    let cursor = sync_cursor(&state);
    let operations = (0..=light_application::show_sync::SHOW_SYNC_EVENT_OBJECTS)
        .map(|index| {
            serde_json::json!({"type": "create_object", "kind": "cad_annotation",
                "id": format!("bulk-{index}"), "body": {"text": index}})
        })
        .collect::<Vec<_>>();
    let bulk = sync_body("bulk", association, &show_id, serde_json::json!(operations));
    assert_eq!(
        post_sync(&app, &token, &bulk).await.status(),
        StatusCode::OK
    );
    assert_eq!(
        gaps(&sync_events(&state, cursor)),
        vec![ShowSyncGapReason::BulkCommit]
    );

    let cursor = sync_cursor(&state);
    let large = "x".repeat(light_application::show_sync::SHOW_SYNC_EVENT_BODY_BYTES + 1);
    let underlay = sync_body(
        "large",
        association,
        &show_id,
        serde_json::json!([{"type": "create_object", "kind": "cad_underlay", "id": "plan",
            "body": {"svg": large}}]),
    );
    assert_eq!(
        post_sync(&app, &token, &underlay).await.status(),
        StatusCode::OK
    );
    let events = sync_events(&state, cursor);
    let commit = committed(&events)[0];
    assert!(commit.objects[0].body_omitted);
    assert!(commit.objects[0].body.is_none());

    // A write that bypasses the unit of work is announced as a gap before the next commit.
    let entry = state.active_show.current().unwrap();
    ShowStore::open(&entry.path)
        .unwrap()
        .set_metadata_values(&[("architect.venue", "Written behind the desk's back")])
        .unwrap();
    state.active_show.clear_document_cache();
    let moved_to = active_revision(&state);
    let cursor = sync_cursor(&state);
    let next = sync_body(
        "after-out-of-band",
        association,
        &show_id,
        serde_json::json!([{"type": "create_object", "kind": "fixture_note", "id": "n",
            "body": {"text": "after"}}]),
    );
    assert_eq!(
        post_sync(&app, &token, &next).await.status(),
        StatusCode::OK
    );
    let events = sync_events(&state, cursor);
    assert_eq!(gaps(&events), vec![ShowSyncGapReason::OutOfBandWrite]);
    assert_eq!(committed(&events)[0].previous_show_revision, moved_to);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn opening_a_show_announces_that_mirrors_must_resynchronize() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let _feed = subscribe_sync_feed(&state);
    let (token, _) = login(&app, "Operator").await;
    let cursor = sync_cursor(&state);
    let show_id = open_sync_show(&app, &token, "Feed open").await;
    let events = sync_events(&state, cursor);
    let gap = events
        .iter()
        .find_map(|event| match event {
            ShowEvent::SyncGap(gap) => Some(*gap),
            _ => None,
        })
        .expect("opening a show is announced on the feed");
    assert_eq!(gap.reason, ShowSyncGapReason::ShowReplaced);
    assert_eq!(gap.show_id.0.to_string(), show_id);
    assert_eq!(gap.show_revision, active_revision(&state));
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn without_an_opted_in_subscription_the_desk_stream_is_unchanged() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    open_sync_show(&app, &token, "Feed not subscribed").await;
    let cursor = state.events.latest_sequence();
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v2/patch/layers/front/update")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({"request_id": "quiet-layer", "action": {"type": "save",
                        "expected_revision": 0, "layer": {"name": "Front", "order": 2}}})
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(state.events.latest_sequence(), cursor + 1);
    assert!(sync_events(&state, cursor).is_empty());
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn the_file_manager_refuses_to_replace_move_rename_or_delete_the_active_show() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let _feed = subscribe_sync_feed(&state);
    let (token, _) = login(&app, "Operator").await;
    open_sync_show(&app, &token, "Feed file manager").await;
    let entry = state.active_show.current().unwrap();
    let path = std::path::PathBuf::from(&entry.path);
    let name = path.file_name().unwrap().to_string_lossy().into_owned();
    let operate = |request_id: &str, body: serde_json::Value| {
        let mut body = body;
        body["request_id"] = serde_json::json!(request_id);
        Request::post("/api/v2/files/shows/operations")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    };
    let staged = path.parent().unwrap().join("replacement");
    std::fs::create_dir_all(&staged).unwrap();
    ShowStore::open(&path)
        .unwrap()
        .backup_to(staged.join(&name))
        .unwrap();
    std::fs::write(path.parent().unwrap().join("notes.txt"), b"notes").unwrap();
    let before = std::fs::read(&path).unwrap();
    let cursor = sync_cursor(&state);
    for (request, body) in [
        (
            "fm-replace",
            serde_json::json!({"operation": "copy", "sources": [format!("replacement/{name}")],
                "destination": null, "conflict": "replace"}),
        ),
        (
            "fm-delete",
            serde_json::json!({"operation": "delete", "sources": [name.clone()]}),
        ),
        (
            "fm-move",
            serde_json::json!({"operation": "move", "sources": [name.clone()],
                "destination": "replacement", "conflict": "replace"}),
        ),
        (
            "fm-rename",
            serde_json::json!({"operation": "rename", "sources": [name.clone()],
                "name": "Elsewhere.show"}),
        ),
        (
            "fm-rename-over",
            serde_json::json!({"operation": "rename", "sources": ["notes.txt"],
                "name": name.clone()}),
        ),
    ] {
        let refused = app.clone().oneshot(operate(request, body)).await.unwrap();
        assert_eq!(refused.status(), StatusCode::CONFLICT, "{request}");
        let body = axum::body::to_bytes(refused.into_body(), usize::MAX)
            .await
            .unwrap();
        assert!(
            String::from_utf8_lossy(&body).contains("Open another show first"),
            "{request} says what to do instead"
        );
    }
    assert_eq!(
        std::fs::read(&path).unwrap(),
        before,
        "the running show's file is untouched"
    );
    assert_eq!(state.active_show.current().unwrap().path, entry.path);
    assert!(
        sync_events(&state, cursor).is_empty(),
        "nothing changed, so nothing is announced"
    );

    // Every other file under the shows root is still the operator's to manage.
    let other = app
        .clone()
        .oneshot(operate(
            "fm-other",
            serde_json::json!({"operation": "delete", "sources": [format!("replacement/{name}")]}),
        ))
        .await
        .unwrap();
    assert_eq!(other.status(), StatusCode::OK);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn readiness_says_whether_an_architect_follows_the_show() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let readiness = || async {
        let response = app
            .clone()
            .oneshot(
                Request::get("/api/v2/readiness")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice::<serde_json::Value>(&body).unwrap()["architect_sync_active"].clone()
    };
    assert_eq!(readiness().await, serde_json::json!(false));
    let feed = subscribe_sync_feed(&state);
    assert_eq!(readiness().await, serde_json::json!(true));
    drop(feed);
    assert_eq!(readiness().await, serde_json::json!(false));
    let _ = std::fs::remove_dir_all(data_dir);
}
