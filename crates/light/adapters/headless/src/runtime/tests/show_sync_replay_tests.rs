//! Exactly-once sync transactions: retries, new sessions, a restarted desk, and a busy store.
use super::show_sync_support::*;
use super::*;

fn create_note(request_id: &str, association: Uuid, show_id: &str) -> serde_json::Value {
    sync_body(
        request_id,
        association,
        show_id,
        serde_json::json!([{"type": "create_object", "kind": "fixture_note", "id": "n1",
            "body": {"text": "hang from the left"}},
            {"type": "update_object", "kind": "cad_venue_groups", "id": "groups",
            "fields": [{"path": "/count", "base": null, "value": 1}]}]),
    )
}

fn append(
    request_id: &str,
    association: Uuid,
    show_id: &str,
    base: Option<u64>,
) -> serde_json::Value {
    let value = base.map_or(1, |base| base + 1);
    sync_body(
        request_id,
        association,
        show_id,
        serde_json::json!([{"type": "update_object", "kind": "fixture_note", "id": "n1",
            "fields": [{"path": "/count", "base": base, "value": value}]}]),
    )
}

#[tokio::test]
async fn a_lost_reply_retried_from_a_new_session_applies_once() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Architect").await;
    let show_id = open_sync_show(&app, &token, "Sync retry").await;
    let association = Uuid::new_v4();
    let first =
        json(post_sync(&app, &token, &create_note("seed", association, &show_id)).await).await;
    assert_eq!(
        first["status"], "conflicted",
        "an edit to an absent object is a conflict"
    );
    assert_eq!(first["conflicts"][0]["reason"], "object_deleted");
    let request = append("count-1", association, &show_id, None);
    let applied = json(post_sync(&app, &token, &request).await).await;
    assert_eq!(applied["status"], "accepted");
    let revision = active_revision(&state);
    // The reply was lost; the client retries from a fresh session.
    let (retry_token, _) = login(&app, "Architect again").await;
    let replayed = json(post_sync(&app, &retry_token, &request).await).await;
    assert_eq!(replayed["replayed"], true);
    assert_eq!(replayed["show_revision"], applied["show_revision"]);
    assert_eq!(replayed["applied"], applied["applied"]);
    assert_eq!(active_revision(&state), revision, "the retry wrote nothing");
    let stored = read_object(&app, &token, &show_id, "fixture_note", "n1").await;
    assert_eq!(stored["object"]["body"]["count"], 1);
    // A reused identity with different content is refused rather than replayed.
    let reused = post_sync(
        &app,
        &token,
        &append("count-1", association, &show_id, Some(1)),
    )
    .await;
    assert_eq!(reused.status(), StatusCode::CONFLICT);
    assert_eq!(json(reused).await["kind"], "request_reused");
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn a_retry_after_the_desk_restarts_applies_once() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Architect").await;
    let show_id = open_sync_show(&app, &token, "Sync restart").await;
    let association = Uuid::new_v4();
    let seed = sync_body(
        "seed",
        association,
        &show_id,
        serde_json::json!([{"type": "create_object", "kind": "fixture_note", "id": "n1",
            "body": {"count": 0}}]),
    );
    assert_eq!(
        post_sync(&app, &token, &seed).await.status(),
        StatusCode::OK
    );
    let request = append("count-1", association, &show_id, Some(0));
    let applied = json(post_sync(&app, &token, &request).await).await;
    assert_eq!(applied["status"], "accepted");
    let entry = state.active_show.current().unwrap();
    let revision = active_revision(&state);
    drop(app);
    drop(state);

    // A new process: nothing in memory survives, only the show file.
    let (restarted, restarted_dir) = test_state();
    restarted.active_show.replace_current(Some(entry));
    restarted.active_show.clear_document_cache();
    let app = router(restarted.clone());
    let (token, _) = login(&app, "Architect").await;
    let replayed = json(post_sync(&app, &token, &request).await).await;
    assert_eq!(replayed["replayed"], true);
    assert_eq!(replayed["show_revision"], applied["show_revision"]);
    assert_eq!(active_revision(&restarted), revision);
    let stored = read_object(&app, &token, &show_id, "fixture_note", "n1").await;
    assert_eq!(stored["object"]["body"]["count"], 1);
    let _ = std::fs::remove_dir_all(data_dir);
    let _ = std::fs::remove_dir_all(restarted_dir);
}

#[tokio::test]
async fn a_busy_store_refuses_retryably_and_the_retry_applies_once() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Architect").await;
    let show_id = open_sync_show(&app, &token, "Sync busy").await;
    let association = Uuid::new_v4();
    let request = sync_body(
        "busy",
        association,
        &show_id,
        serde_json::json!([{"type": "create_object", "kind": "cad_underlay", "id": "u1",
            "body": {"file": "plan.pdf"}}]),
    );
    let entry = state.active_show.current().unwrap();
    let before = active_revision(&state);
    let writer = rusqlite::Connection::open(&entry.path).unwrap();
    writer.execute_batch("BEGIN IMMEDIATE;").unwrap();
    let busy = post_sync(&app, &token, &request).await;
    assert_eq!(busy.status(), StatusCode::SERVICE_UNAVAILABLE);
    let busy = json(busy).await;
    assert_eq!(busy["kind"], "unavailable");
    assert_eq!(busy["retryable"], true);
    writer.execute_batch("ROLLBACK;").unwrap();
    drop(writer);
    assert_eq!(
        active_revision(&state),
        before,
        "the refused request wrote nothing"
    );
    let applied = json(post_sync(&app, &token, &request).await).await;
    assert_eq!(applied["status"], "accepted");
    assert_eq!(applied["replayed"], false);
    assert_eq!(active_revision(&state), before + 1);
    let replayed = json(post_sync(&app, &token, &request).await).await;
    assert_eq!(replayed["replayed"], true);
    assert_eq!(active_revision(&state), before + 1);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn a_retry_after_a_manual_whole_file_save_still_applies_once() {
    use base64::Engine;
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Architect").await;
    let show_id = open_sync_show(&app, &token, "Sync manual save").await;
    let association = Uuid::new_v4();
    let seed = sync_body(
        "seed",
        association,
        &show_id,
        serde_json::json!([{"type": "create_object", "kind": "fixture_note", "id": "n1",
            "body": {"count": 0}}]),
    );
    assert_eq!(
        post_sync(&app, &token, &seed).await.status(),
        StatusCode::OK
    );
    let request = append("count-1", association, &show_id, Some(0));
    assert_eq!(
        json(post_sync(&app, &token, &request).await).await["status"],
        "accepted"
    );

    // The Architect's own file holds the same content but none of the desk's identities.
    let entry = state.active_show.current().unwrap();
    let staged = data_dir.join("architect-copy.show");
    ShowStore::open(&entry.path)
        .unwrap()
        .backup_to(&staged)
        .unwrap();
    rusqlite::Connection::open(&staged)
        .unwrap()
        .execute_batch("DELETE FROM sync_applied_requests;")
        .unwrap();
    let expected_revision = active_revision(&state);
    let saved = app
        .clone()
        .oneshot(
            Request::post("/api/v2/shows")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({"request_id": "manual-save", "action": {
                        "type": "update_document", "destination_show_id": show_id,
                        "expected_revision": expected_revision,
                        "data_base64": base64::engine::general_purpose::STANDARD
                            .encode(std::fs::read(&staged).unwrap())}})
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(saved.status(), StatusCode::OK);
    let revision = active_revision(&state);

    let replayed = json(post_sync(&app, &token, &request).await).await;
    assert_eq!(
        replayed["replayed"], true,
        "the desk's identity survived the save"
    );
    assert_eq!(active_revision(&state), revision);
    let stored = read_object(&app, &token, &show_id, "fixture_note", "n1").await;
    assert_eq!(stored["object"]["body"]["count"], 1);
    let _ = std::fs::remove_dir_all(data_dir);
}
