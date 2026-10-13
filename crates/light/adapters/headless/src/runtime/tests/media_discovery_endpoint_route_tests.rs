use super::*;
use crate::runtime::cue_media_previews_http::tests::media_server;
use std::sync::atomic::{AtomicUsize, Ordering};

const A: &str = "00000000-0000-4000-8000-000000000041";
const B: &str = "00000000-0000-4000-8000-000000000042";
const HOST: &str = "127.0.0.1";

struct MockOutput {
    port: u16,
    calls: Arc<AtomicUsize>,
    job: tokio::task::JoinHandle<()>,
}
impl Drop for MockOutput {
    fn drop(&mut self) {
        self.job.abort();
    }
}
async fn mock_output(output: &'static str, port: u16) -> MockOutput {
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = calls.clone();
    let listener = tokio::net::TcpListener::bind((HOST, port))
        .await
        .expect("only own isolated mock endpoint is contacted");
    let port = listener.local_addr().unwrap().port();
    let app = Router::new().route(
        "/api/v2/outputs/{id}/configuration/update",
        post(
            move |Path(id): Path<Uuid>, Json(body): Json<serde_json::Value>| {
                let seen = seen.clone();
                async move {
                    seen.fetch_add(1, Ordering::SeqCst);
                    assert_eq!(
                        id.to_string(),
                        output,
                        "must reach the explicit bound output"
                    );
                    Json(serde_json::json!({
                        "id": output, "name": output, "personality": "two-layers",
                        "protocol": body.get("protocol").and_then(|v| v.as_str()).unwrap_or("sacn"),
                        "universe": body["universe"], "startAddress": body["startAddress"],
                        "dmxPendingRestart": false, "tempoSource": "internal", "speedGroup": null
                    }))
                }
            },
        ),
    );
    let job = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    MockOutput { port, calls, job }
}
fn fixture(output: &str, http: u16, citp: u16) -> light_fixture::PatchedFixture {
    let mut f = media_server("2 layers", [127, 0, 0, 1], output);
    let e = f.direct_control.as_mut().unwrap();
    e.port = citp;
    e.http_port = Some(http);
    f.universe = Some(if output == A { 3 } else { 4 });
    f.address = Some(1);
    f
}
fn request(token: &str, output: &str) -> Request<Body> {
    Request::post("/api/v2/media-servers/discovered/address")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::json!({
                "requestId": Uuid::new_v4(), "host": HOST, "outputId": output,
                "universe": 12, "startAddress": 21, "protocol": "art-net",
                "httpPort": 8080
            })
            .to_string(),
        ))
        .unwrap()
}

#[tokio::test]
async fn tl743_authenticated_address_uses_committed_custom_http_and_refuses_copied_uuid_before_request()
 {
    // Guard the legacy fallback with our own isolated loopback mock, never an unrelated8080 process.
    let default = mock_output(A, 8080).await;
    let a = mock_output(A, 0).await;
    let b = mock_output(B, 0).await;
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let original = vec![fixture(A, a.port, 14809), fixture(B, b.port, 14810)];
    state
        .output
        .replace_snapshot(EngineSnapshot {
            fixtures: original.clone().into(),
            ..Default::default()
        })
        .unwrap();
    let response = app.clone().oneshot(request(&token, A)).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json(response).await["id"], A);
    assert_eq!(
        a.calls.load(Ordering::SeqCst),
        1,
        "custom endpoint owns address mutation"
    );
    assert_eq!(b.calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        default.calls.load(Ordering::SeqCst),
        0,
        "stored override must not fall back"
    );
    let unchanged = serde_json::to_value(&state.output.snapshot().fixtures).unwrap();
    let response_b = app.clone().oneshot(request(&token, B)).await.unwrap();
    assert_eq!(response_b.status(), StatusCode::OK);
    assert_eq!(json(response_b).await["id"], B);
    assert_eq!(a.calls.load(Ordering::SeqCst), 1);
    assert_eq!(b.calls.load(Ordering::SeqCst), 1);
    assert_eq!(default.calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        serde_json::to_value(&state.output.snapshot().fixtures).unwrap(),
        unchanged
    );
    let mut copied = original.clone();
    copied[1].internal_bindings.output = Some(A.into());
    state
        .output
        .replace_snapshot(EngineSnapshot {
            fixtures: copied.into(),
            ..Default::default()
        })
        .unwrap();
    let refused = app.clone().oneshot(request(&token, A)).await.unwrap();
    assert_eq!(refused.status(), StatusCode::BAD_REQUEST);
    assert!(
        json(refused).await["error"]
            .as_str()
            .unwrap()
            .contains("ambiguous")
    );
    assert_eq!(a.calls.load(Ordering::SeqCst), 1);
    assert_eq!(b.calls.load(Ordering::SeqCst), 1);
    assert_eq!(default.calls.load(Ordering::SeqCst), 0);
    state
        .output
        .replace_snapshot(EngineSnapshot {
            fixtures: original.into(),
            ..Default::default()
        })
        .unwrap();
    let missing = app
        .clone()
        .oneshot(request(&token, "00000000-0000-4000-8000-000000000043"))
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::BAD_REQUEST);
    assert_eq!(default.calls.load(Ordering::SeqCst), 0);
    state
        .output
        .replace_snapshot(EngineSnapshot::default())
        .unwrap();
    let legacy = app.clone().oneshot(request(&token, A)).await.unwrap();
    assert_eq!(legacy.status(), StatusCode::OK);
    assert_eq!(json(legacy).await["id"], A);
    assert_eq!(default.calls.load(Ordering::SeqCst), 1);
    assert_eq!(a.calls.load(Ordering::SeqCst), 1);
    assert_eq!(b.calls.load(Ordering::SeqCst), 1);
    let unauthorized = app.oneshot(request("not-a-token", A)).await.unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
    let _ = std::fs::remove_dir_all(data_dir);
}
