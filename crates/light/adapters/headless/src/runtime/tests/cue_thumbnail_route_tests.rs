use super::*;
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};

/// Smallest byte sequence the server accepts as a picture: a RIFF/WEBP container header.
fn webp(marker: u8) -> String {
    let mut bytes = b"RIFF\0\0\0\0WEBP".to_vec();
    bytes.push(marker);
    BASE64.encode(bytes)
}

async fn open_show(app: &Router, token: &str, show_id: &str) {
    let response = app
        .clone()
        .oneshot(open_show_request(token, show_id))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

/// Builds the Cuelist body from the domain type so the seeded object is exactly what the show
/// stores, rather than a hand-written shape that drifts from `CueList`.
fn cue_list_body(cue_ids: &[Uuid]) -> serde_json::Value {
    let cues = cue_ids
        .iter()
        .enumerate()
        .map(|(index, id)| light_playback::Cue {
            id: *id,
            ..light_playback::Cue::new((index + 1) as f64)
        })
        .collect();
    serde_json::to_value(light_playback::CueList {
        id: light_core::CueListId(Uuid::new_v4()),
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
        cues,
    })
    .unwrap()
}

async fn seed_cue_list(state: &AppState, token: &str, show_id: &str, cue_ids: &[Uuid]) -> Response {
    seed_show_object(
        state,
        token,
        show_id,
        "cue_list",
        "main",
        0,
        cue_list_body(cue_ids),
    )
    .await
}

fn upload(cue_id: Uuid, hash: &str, marker: u8) -> serde_json::Value {
    serde_json::json!({
        "cue_id": cue_id,
        "state_hash": hash,
        "image_base64": webp(marker),
        "width": 240,
        "height": 135,
    })
}

async fn post_update(
    app: &Router,
    token: &str,
    show_id: &str,
    body: &serde_json::Value,
) -> Response {
    app.clone()
        .oneshot(
            Request::post("/api/v2/cues/thumbnails/update")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("x-tosk-show", show_id)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn get_index(app: &Router, token: &str, show_id: &str) -> serde_json::Value {
    let response = app
        .clone()
        .oneshot(
            Request::get("/api/v2/cues/thumbnails")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("x-tosk-show", show_id)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    json(response).await
}

async fn get_thumbnail(app: &Router, token: &str, show_id: &str, cue_id: Uuid) -> Response {
    app.clone()
        .oneshot(
            Request::get(format!("/api/v2/cues/{cue_id}/thumbnail"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("x-tosk-show", show_id)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn a_stored_preview_is_served_back_as_a_picture_with_its_state_hash_as_the_validator() {
    let (state, _data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Preview").await;
    let show_id = show["id"].as_str().unwrap();
    open_show(&app, &token, show_id).await;
    let cue = Uuid::new_v4();
    assert_eq!(
        seed_cue_list(&state, &token, show_id, &[cue])
            .await
            .status(),
        StatusCode::OK
    );

    let response = post_update(
        &app,
        &token,
        show_id,
        &serde_json::json!({"request_id": "one", "thumbnails": [upload(cue, "hash-1", 1)]}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let outcome = json(response).await;
    assert_eq!(outcome["stored"], 1);
    assert_eq!(outcome["replayed"], false);
    assert_eq!(outcome["skipped_cue_ids"], serde_json::json!([]));

    let picture = get_thumbnail(&app, &token, show_id, cue).await;
    assert_eq!(picture.status(), StatusCode::OK);
    assert_eq!(picture.headers()[header::CONTENT_TYPE], "image/webp");
    assert_eq!(picture.headers()[header::ETAG], "\"hash-1\"");
    assert_eq!(picture.headers()["x-light-image-width"], "240");

    let index = get_index(&app, &token, show_id).await;
    assert_eq!(index["entries"].as_array().unwrap().len(), 1);
    assert_eq!(index["entries"][0]["state_hash"], "hash-1");
    assert!(
        index["entries"][0].get("image_base64").is_none(),
        "the index must not carry pixels"
    );
}

#[tokio::test]
async fn a_cue_with_no_stored_preview_reports_not_found_rather_than_failing_the_cuelist() {
    let (state, _data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Preview").await;
    let show_id = show["id"].as_str().unwrap();
    open_show(&app, &token, show_id).await;

    let response = get_thumbnail(&app, &token, show_id, Uuid::new_v4()).await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert!(
        get_index(&app, &token, show_id).await["entries"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn redrawing_a_cue_replaces_its_picture_and_changes_the_validator() {
    let (state, _data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Preview").await;
    let show_id = show["id"].as_str().unwrap();
    open_show(&app, &token, show_id).await;
    let cue = Uuid::new_v4();
    seed_cue_list(&state, &token, show_id, &[cue]).await;

    post_update(
        &app,
        &token,
        show_id,
        &serde_json::json!({"request_id": "one", "thumbnails": [upload(cue, "hash-1", 1)]}),
    )
    .await;
    post_update(
        &app,
        &token,
        show_id,
        &serde_json::json!({"request_id": "two", "thumbnails": [upload(cue, "hash-2", 2)]}),
    )
    .await;

    let index = get_index(&app, &token, show_id).await;
    assert_eq!(
        index["entries"].as_array().unwrap().len(),
        1,
        "a redraw replaces rather than accumulates"
    );
    assert_eq!(index["entries"][0]["state_hash"], "hash-2");
    let picture = get_thumbnail(&app, &token, show_id, cue).await;
    assert_eq!(picture.headers()[header::ETAG], "\"hash-2\"");
}

#[tokio::test]
async fn an_upload_for_a_cue_the_show_no_longer_holds_is_skipped_rather_than_rejected() {
    let (state, _data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Preview").await;
    let show_id = show["id"].as_str().unwrap();
    open_show(&app, &token, show_id).await;
    let (kept, deleted) = (Uuid::new_v4(), Uuid::new_v4());
    seed_cue_list(&state, &token, show_id, &[kept]).await;

    let response = post_update(
        &app,
        &token,
        show_id,
        &serde_json::json!({
            "request_id": "one",
            "thumbnails": [upload(kept, "hash", 1), upload(deleted, "hash", 2)],
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let outcome = json(response).await;
    assert_eq!(outcome["stored"], 1);
    assert_eq!(outcome["skipped_cue_ids"], serde_json::json!([deleted]));
    assert_eq!(
        get_index(&app, &token, show_id).await["entries"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn a_preview_for_a_deleted_cue_is_pruned_on_the_next_upload() {
    let (state, _data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Preview").await;
    let show_id = show["id"].as_str().unwrap();
    open_show(&app, &token, show_id).await;
    let (first, second) = (Uuid::new_v4(), Uuid::new_v4());
    let seeded = seed_cue_list(&state, &token, show_id, &[first, second]).await;
    let revision = json(seeded).await["revision"].as_u64().unwrap();
    post_update(
        &app,
        &token,
        show_id,
        &serde_json::json!({
            "request_id": "one",
            "thumbnails": [upload(first, "hash", 1), upload(second, "hash", 2)],
        }),
    )
    .await;
    assert_eq!(
        get_index(&app, &token, show_id).await["entries"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    // The operator deletes the second Cue, then the desk redraws the first.
    seed_show_object(
        &state,
        &token,
        show_id,
        "cue_list",
        "main",
        revision,
        cue_list_body(&[first]),
    )
    .await;
    post_update(
        &app,
        &token,
        show_id,
        &serde_json::json!({"request_id": "two", "thumbnails": [upload(first, "hash-b", 3)]}),
    )
    .await;

    let index = get_index(&app, &token, show_id).await;
    assert_eq!(index["entries"].as_array().unwrap().len(), 1);
    assert_eq!(index["entries"][0]["cue_id"], serde_json::json!(first));
    assert_eq!(
        get_thumbnail(&app, &token, show_id, second).await.status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn a_repeated_request_id_replays_the_stored_outcome_instead_of_storing_twice() {
    let (state, _data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Preview").await;
    let show_id = show["id"].as_str().unwrap();
    open_show(&app, &token, show_id).await;
    let cue = Uuid::new_v4();
    seed_cue_list(&state, &token, show_id, &[cue]).await;
    let body = serde_json::json!({"request_id": "one", "thumbnails": [upload(cue, "hash-1", 1)]});

    let first = json(post_update(&app, &token, show_id, &body).await).await;
    let replay = json(post_update(&app, &token, show_id, &body).await).await;

    assert_eq!(first["replayed"], false);
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["correlation_id"], first["correlation_id"]);

    // The same request_id carrying different pictures is a client bug, not a replay.
    let conflicting =
        serde_json::json!({"request_id": "one", "thumbnails": [upload(cue, "hash-2", 9)]});
    assert_eq!(
        post_update(&app, &token, show_id, &conflicting)
            .await
            .status(),
        StatusCode::CONFLICT
    );
}

#[tokio::test]
async fn the_server_refuses_bytes_that_are_not_a_picture() {
    let (state, _data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Preview").await;
    let show_id = show["id"].as_str().unwrap();
    open_show(&app, &token, show_id).await;
    let cue = Uuid::new_v4();
    seed_cue_list(&state, &token, show_id, &[cue]).await;

    for bad in [
        BASE64.encode(b"<html>not a picture</html>"),
        "not base64 at all!!".to_string(),
    ] {
        let body = serde_json::json!({
            "request_id": Uuid::new_v4(),
            "thumbnails": [{
                "cue_id": cue, "state_hash": "hash",
                "image_base64": bad, "width": 240, "height": 135,
            }],
        });
        assert_eq!(
            post_update(&app, &token, show_id, &body).await.status(),
            StatusCode::BAD_REQUEST
        );
    }
    assert!(
        get_index(&app, &token, show_id).await["entries"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn preview_routes_require_authentication_and_an_agreeing_show_header() {
    let (state, _data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Preview").await;
    let show_id = show["id"].as_str().unwrap();
    open_show(&app, &token, show_id).await;

    let unauthenticated = app
        .clone()
        .oneshot(
            Request::get("/api/v2/cues/thumbnails")
                .header("x-tosk-show", show_id)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

    let other_show = Uuid::new_v4().to_string();
    let mismatched = app
        .clone()
        .oneshot(
            Request::get("/api/v2/cues/thumbnails")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("x-tosk-show", &other_show)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(mismatched.status(), StatusCode::CONFLICT);
}
