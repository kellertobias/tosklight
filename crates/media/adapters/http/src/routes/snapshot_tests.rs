use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use axum::http::{StatusCode, header};
use http_body_util::BodyExt as _;
use tower::ServiceExt as _;

use super::{SnapshotFailure, SnapshotImage, SnapshotRequest};
use crate::routes::bench::{bench, post, send};
use crate::routes::router;

/// Two layers of 59 slots and the 40-slot master.
const TWO_LAYER_SLOTS: usize = 2 * 59 + 40;

fn body(slots: usize, layer: Option<usize>) -> String {
    serde_json::json!({
        "slots": vec![7; slots],
        "layer": layer,
        "width": 64,
        "height": 36,
    })
    .to_string()
}

struct Recording {
    router: axum::Router,
    requests: Arc<Mutex<Vec<SnapshotRequest>>>,
    renders: Arc<AtomicUsize>,
    output: media_domain::OutputId,
}

fn recording(result: Result<SnapshotImage, SnapshotFailure>) -> Recording {
    let bench = bench();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let renders = Arc::new(AtomicUsize::new(0));
    let mut api = bench.api.clone();
    let seen = Arc::clone(&requests);
    let counted = Arc::clone(&renders);
    api.snapshot = Arc::new(move |request| {
        seen.lock().unwrap().push(request);
        counted.fetch_add(1, Ordering::SeqCst);
        Box::pin(std::future::ready(result.clone()))
    });
    Recording {
        router: router(api),
        requests,
        renders,
        output: bench.output,
    }
}

fn picture(empty: bool) -> SnapshotImage {
    SnapshotImage {
        png: Arc::new(b"\x89PNG-fake".to_vec()),
        width: 64,
        height: 36,
        empty,
    }
}

#[tokio::test]
async fn a_program_snapshot_renders_the_supplied_slots_and_is_remembered_by_content() {
    let recording = recording(Ok(picture(false)));
    let uri = format!("/api/v2/outputs/{}/snapshot", recording.output);

    let response = recording
        .router
        .clone()
        .oneshot(post(uri.clone(), &body(TWO_LAYER_SLOTS, None)))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "image/png");
    assert_eq!(
        response.headers()["x-tosklight-snapshot-content"],
        "content"
    );
    assert_eq!(response.headers()["x-tosklight-snapshot-cache"], "miss");
    let etag = response.headers()[header::ETAG].clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(bytes.as_ref(), b"\x89PNG-fake");
    {
        let requests = recording.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].output, recording.output);
        assert_eq!(requests[0].layer, None, "no layer means the whole Program");
        assert_eq!(requests[0].slots, vec![7; TWO_LAYER_SLOTS]);
        assert_eq!((requests[0].width, requests[0].height), (64, 36));
    }

    let again = recording
        .router
        .clone()
        .oneshot(post(uri, &body(TWO_LAYER_SLOTS, None)))
        .await
        .unwrap();
    assert_eq!(again.headers()["x-tosklight-snapshot-cache"], "hit");
    assert_eq!(again.headers()[header::ETAG], etag);
    assert_eq!(recording.renders.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_layer_snapshot_is_its_own_picture_and_reports_an_empty_layer() {
    let recording = recording(Ok(picture(true)));
    let uri = format!("/api/v2/outputs/{}/snapshot", recording.output);
    let program = recording
        .router
        .clone()
        .oneshot(post(uri.clone(), &body(TWO_LAYER_SLOTS, None)))
        .await
        .unwrap();
    let layer = recording
        .router
        .clone()
        .oneshot(post(uri, &body(TWO_LAYER_SLOTS, Some(1))))
        .await
        .unwrap();
    assert_eq!(layer.status(), StatusCode::OK);
    assert_eq!(layer.headers()["x-tosklight-snapshot-content"], "empty");
    assert_ne!(
        program.headers()[header::ETAG],
        layer.headers()[header::ETAG],
        "a layer and the Program never share a remembered picture"
    );
    assert_eq!(recording.renders.load(Ordering::SeqCst), 2);
    assert_eq!(recording.requests.lock().unwrap()[1].layer, Some(1));
}

#[tokio::test]
async fn a_snapshot_rejects_slots_that_do_not_match_the_personality_or_a_missing_layer() {
    let recording = recording(Ok(picture(false)));
    let uri = format!("/api/v2/outputs/{}/snapshot", recording.output);
    let (status, error) = send(
        &recording.router,
        post(uri.clone(), &body(8 * 59 + 40, None)),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(error["code"], "snapshot-slots-mismatch");

    let (status, error) = send(
        &recording.router,
        post(uri.clone(), &body(TWO_LAYER_SLOTS, Some(2))),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(error["code"], "layer-not-found");

    let oversized =
        serde_json::json!({ "slots": vec![0; TWO_LAYER_SLOTS], "width": 4096, "height": 36 });
    let (status, error) = send(&recording.router, post(uri.clone(), &oversized.to_string())).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(error["code"], "snapshot-size");

    let out_of_range = serde_json::json!({ "slots": vec![256; TWO_LAYER_SLOTS] });
    let (status, error) = send(&recording.router, post(uri, &out_of_range.to_string())).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(error["code"], "snapshot-slot-range");

    let (status, error) = send(
        &recording.router,
        post(
            format!("/api/v2/outputs/{}/snapshot", media_domain::OutputId::new()),
            &body(TWO_LAYER_SLOTS, None),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(error["code"], "unknown-output");
    assert_eq!(recording.renders.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn a_renderer_that_cannot_draw_answers_unavailable_and_nothing_is_remembered() {
    let recording = recording(Err(SnapshotFailure::NotReady("still loading".into())));
    let uri = format!("/api/v2/outputs/{}/snapshot", recording.output);
    let (status, error) = send(
        &recording.router,
        post(uri.clone(), &body(TWO_LAYER_SLOTS, None)),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(error["code"], "snapshot-not-ready");
    let (status, _) = send(&recording.router, post(uri, &body(TWO_LAYER_SLOTS, None))).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        recording.renders.load(Ordering::SeqCst),
        2,
        "a failure is not cached"
    );

    let bench = bench();
    let (status, error) = send(
        &bench.router,
        post(
            format!("/api/v2/outputs/{}/snapshot", bench.output),
            &body(TWO_LAYER_SLOTS, None),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(error["code"], "snapshot-unavailable");
    assert_eq!(bench.api.snapshots.len(), 0);
}
