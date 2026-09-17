//! The output configuration fields Tos Light Control reads when it discovers and patches this
//! server. The desk's discovery tests parse the same reference document, so the two products
//! cannot drift apart unnoticed.

use axum::http::StatusCode;

use crate::routes::bench::{bench, get, post, send};

/// Shared with `crates/light/adapters/headless/src/runtime/media_discovery_tests.rs`. The `id`
/// is replaced with the bench output's id before comparing.
const DESK_REFERENCE: &str = r#"{"id":"00000000-0000-4000-8000-000000000040","name":"Main","personality":"eight-layers","protocol":"sacn","universe":4,"startAddress":1,"dmxPendingRestart":true,"tempoSource":"speed-group","speedGroup":3}"#;

#[tokio::test]
async fn the_output_configuration_carries_every_field_the_desk_reads() {
    let bench = bench();
    let (status, _) = send(
        &bench.router,
        post(
            format!("/api/v2/outputs/{}/configuration/update", bench.output),
            r#"{"requestId":"desk-contract","personality":"eight-layers","protocol":"sacn","universe":4,"startAddress":1,"tempoSource":"speed-group","speedGroup":3}"#,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = send(
        &bench.router,
        get(format!("/api/v2/outputs/{}/configuration", bench.output)),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let mut reference: serde_json::Value = serde_json::from_str(DESK_REFERENCE).unwrap();
    reference["id"] = serde_json::json!(bench.output.to_string());
    for (field, expected) in reference.as_object().unwrap() {
        assert_eq!(&body[field], expected, "desk-read field {field}");
    }
    assert!(
        body.get("personalityLayout").is_none(),
        "the desk treats a reported channel layout as an outdated server"
    );
}

/// Shared with `crates/light/adapters/headless/src/runtime/media_api.rs`: the fields of
/// `GET /api/v2/outputs` the desk's native Media snapshot reads, including the rate the desk
/// converts In and Out points with.
const DESK_OUTPUTS_REFERENCE: &str =
    r#"{"frameRate":30,"layers":[{"effects":[],"visualizerChannels":[]}]}"#;

#[tokio::test]
async fn the_outputs_list_carries_the_point_frame_rate_the_desk_reads() {
    let bench = bench();
    let (status, _) = send(
        &bench.router,
        post(
            "/api/v2/playback/update".into(),
            r#"{"requestId":"desk-frame-rate","frameRate":30}"#,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = send(&bench.router, get("/api/v2/outputs".into())).await;
    assert_eq!(status, StatusCode::OK);
    let reference: serde_json::Value = serde_json::from_str(DESK_OUTPUTS_REFERENCE).unwrap();
    let output = &body[0];
    assert!(output["id"].is_string());
    assert_eq!(output["frameRate"], reference["frameRate"]);
    for field in ["effects", "visualizerChannels"] {
        assert!(
            output["layers"][0][field].is_array(),
            "desk-read layer field {field}"
        );
    }
}

#[tokio::test]
async fn only_the_two_personalities_the_desk_patches_are_accepted() {
    let bench = bench();
    for (personality, accepted) in [
        ("two-layers", true),
        ("eight-layers", true),
        ("legacy", false),
        ("sixteen-layers", false),
    ] {
        let (status, _) = send(
            &bench.router,
            post(
                format!("/api/v2/outputs/{}/configuration/update", bench.output),
                &format!(
                    r#"{{"requestId":"personality-{personality}","personality":"{personality}","startAddress":1}}"#
                ),
            ),
        )
        .await;
        assert_eq!(status == StatusCode::OK, accepted, "{personality}");
    }
}

/// The coordinated edit Tos Light Control sends from Show Patch › Patch address: the address plus
/// the DMX input protocol of the desk route. Art-Net numbers its Port-Address from 0.
#[tokio::test]
async fn a_coordinated_desk_edit_moves_the_input_protocol_and_universe() {
    let bench = bench();
    for (protocol, universe) in [("art-net", 0), ("art-net", 12), ("sacn", 101)] {
        let (status, body) = send(
            &bench.router,
            post(
                format!("/api/v2/outputs/{}/configuration/update", bench.output),
                &format!(
                    r#"{{"requestId":"desk-coordinated-{protocol}-{universe}","universe":{universe},"startAddress":33,"protocol":"{protocol}"}}"#
                ),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{protocol} {universe}");
        assert_eq!(body["protocol"], protocol);
        assert_eq!(body["universe"], universe);
        assert_eq!(body["startAddress"], 33);
        assert_eq!(
            body["dmxPendingRestart"], false,
            "an address and protocol move applies live"
        );
    }
}
