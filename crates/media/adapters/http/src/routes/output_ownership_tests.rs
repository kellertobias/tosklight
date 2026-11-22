//! A stale preview must not restore over another browser's newer manual playback owner.

use axum::http::StatusCode;
use serde_json::json;

use super::bench::{bench, get, post, send};

#[tokio::test]
async fn stale_preview_generation_cannot_restore_layers_master_or_release_a_new_manual_owner() {
    let bench = bench();
    let base = format!("/api/v2/outputs/{}", bench.output);
    let (status, first) = send(&bench.router, get(format!("{base}/playback/take-over"))).await;
    assert_eq!(status, StatusCode::OK);
    let old = first["ownershipGeneration"].as_u64().unwrap();
    assert_eq!(
        send(&bench.router, get(format!("{base}/playback/release")))
            .await
            .0,
        StatusCode::OK
    );
    let (_, latest) = send(&bench.router, get(format!("{base}/playback/take-over"))).await;
    let current = latest["ownershipGeneration"].as_u64().unwrap();
    assert!(current > old);
    assert_eq!(
        send(
            &bench.router,
            post(
                format!("{base}/layers/0/update"),
                r#"{"dimmer":0.35,"folder":1,"file":7}"#
            )
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        send(
            &bench.router,
            post(format!("{base}/master/update"), r#"{"dimmer":0.4}"#)
        )
        .await
        .0,
        StatusCode::OK
    );

    for (path, body) in [
        (
            "layers/0/update",
            json!({"dimmer":1,"folder":1,"file":1,"expectedOwnershipGeneration":old}),
        ),
        (
            "master/update",
            json!({"dimmer":1,"expectedOwnershipGeneration":old}),
        ),
    ] {
        let (status, error) = send(
            &bench.router,
            post(format!("{base}/{path}"), &body.to_string()),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(error["code"], "playback-owner-changed");
    }
    let (status, error) = send(
        &bench.router,
        get(format!(
            "{base}/playback/release?expectedOwnershipGeneration={old}"
        )),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(error["code"], "playback-owner-changed");
    let (_, output) = send(&bench.router, get(format!("{base}/state"))).await;
    assert_eq!(output["playbackTakeover"], true);
    assert_eq!(output["ownershipGeneration"], current);
    assert!((output["layers"][0]["dimmer"].as_f64().unwrap() - 0.35).abs() < 1e-6);
    assert_eq!(output["layers"][0]["address"]["file"], 7);
    assert!((output["master"]["dimmer"].as_f64().unwrap() - 0.4).abs() < 1e-6);

    let (status, _) = send(
        &bench.router,
        post(
            format!("{base}/layers/0/update"),
            &json!({"dimmer":0.6,"expectedOwnershipGeneration":current}).to_string(),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the current owner can still control the output"
    );
    assert_eq!(
        send(
            &bench.router,
            get(format!(
                "{base}/playback/release?expectedOwnershipGeneration={current}"
            ))
        )
        .await
        .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn guarded_takeover_refuses_a_stale_output_snapshot() {
    let bench = bench();
    let base = format!("/api/v2/outputs/{}", bench.output);
    let (_, initial) = send(&bench.router, get(format!("{base}/state"))).await;
    let generation = initial["ownershipGeneration"].as_u64().unwrap();
    send(&bench.router, get(format!("{base}/playback/take-over"))).await;
    let (status, _) = send(
        &bench.router,
        get(format!(
            "{base}/playback/take-over?expectedOwnershipGeneration={generation}"
        )),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}
