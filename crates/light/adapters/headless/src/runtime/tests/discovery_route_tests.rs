use super::*;

/// A desk that is not looking says so, rather than presenting "found nothing" as if it had.
///
/// A test desk never starts a responder — the network is not what is under test — so this is also
/// the answer an installation with `LIGHT_DISCOVERY=off`, or one on a network without mDNS, gives.
#[tokio::test]
async fn a_desk_that_is_not_browsing_says_so_rather_than_answering_an_empty_list() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;

    let response = app
        .clone()
        .oneshot(
            Request::get("/api/v2/discovery/peers")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let snapshot = json(response).await;
    assert_eq!(snapshot["browsing"], false);
    assert_eq!(snapshot["peers"].as_array().unwrap().len(), 0);
    let _ = std::fs::remove_dir_all(data_dir);
}

/// Loading from a visualizer that is not there fails as that, and imports nothing.
///
/// The menu is drawn from a list that can be a few seconds old, so pressing an entry that has
/// since gone is ordinary. What must not happen is a half-import, or a different show.
#[tokio::test]
async fn loading_from_a_visualizer_that_is_gone_is_refused_rather_than_substituted() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;

    let before = json(show_library(&app, &token).await).await;
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v2/shows")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "request_id": "load-from-a-visualizer-that-left",
                        "action": {
                            "type": "import_from_visualizer",
                            "instance": "tosklight-editor-nowhere._tosklight._tcp.local.",
                        },
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let after = json(show_library(&app, &token).await).await;
    assert_eq!(
        after["shows"].as_array().unwrap().len(),
        before["shows"].as_array().unwrap().len(),
        "a refused load leaves the library exactly as it was"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

async fn show_library(app: &Router, token: &str) -> Response {
    app.clone()
        .oneshot(
            Request::get("/api/v2/shows")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}
