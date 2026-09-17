use super::*;
use crate::runtime::cue_media_previews_http::tests::{
    OUTPUT_A, cue, cue_list, layer, media_server, set, wash,
};
use light_wire::v2::cue_media_previews as wire;

async fn get(app: &Router, token: &str, show_id: &str, uri: String) -> Response {
    app.clone()
        .oneshot(
            Request::get(uri)
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("x-tosk-show", show_id)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn the_index_lists_only_media_cues_and_a_lighting_cue_has_no_media_picture() {
    let (state, _data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Media previews").await;
    let show_id = show["id"].as_str().unwrap().to_owned();
    let opened = app
        .clone()
        .oneshot(open_show_request(&token, &show_id))
        .await
        .unwrap();
    assert_eq!(opened.status(), StatusCode::OK);

    // Loopback on a port nothing answers keeps the "offline" path fast and local.
    let server = media_server("2 layers", [127, 0, 0, 1], OUTPUT_A);
    let lighting = wash();
    let media_cue = cue(1, vec![set(layer(&server, 0), "media.folder", 1.0 / 255.0)]);
    let lighting_cue = cue(2, vec![set(lighting.fixture_id, "intensity", 1.0)]);
    let (media_cue_id, lighting_cue_id) = (media_cue.id, lighting_cue.id);
    let current = state.output.snapshot();
    state
        .output
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![server.clone(), lighting].into(),
            cue_lists: vec![cue_list(vec![media_cue, lighting_cue])].into(),
            revision: current.revision + 1,
            ..(*current).clone()
        })
        .unwrap();

    let index = get(&app, &token, &show_id, "/api/v2/cues/media-previews".into()).await;
    assert_eq!(index.status(), StatusCode::OK);
    let index: wire::CueMediaPreviewIndex = serde_json::from_value(json(index).await).unwrap();
    assert_eq!(index.show_id.to_string(), show_id);
    assert_eq!(index.entries.len(), 1);
    assert_eq!(index.entries[0].cue_id, media_cue_id);
    assert_eq!(index.entries[0].server_fixture_id, server.fixture_id.0);
    assert_eq!(index.entries[0].scope, wire::CueMediaPreviewScope::Layer);
    assert_eq!(index.entries[0].layer, Some(0));

    let missing = get(
        &app,
        &token,
        &show_id,
        format!("/api/v2/cues/{lighting_cue_id}/media-preview"),
    )
    .await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    assert_eq!(missing.headers()[header::CACHE_CONTROL], "no-store");
    let failure: wire::CueMediaPreviewFailure =
        serde_json::from_value(json(missing).await).unwrap();
    assert_eq!(failure.state, wire::CueMediaPreviewFailureState::Missing);

    let unauthenticated = app
        .clone()
        .oneshot(
            Request::get("/api/v2/cues/media-previews")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

    let stale_show = get(
        &app,
        &token,
        &Uuid::new_v4().to_string(),
        "/api/v2/cues/media-previews".into(),
    )
    .await;
    assert_eq!(stale_show.status(), StatusCode::CONFLICT);
}
