use super::*;
use crate::runtime::cue_media_previews_http::tests::{OUTPUT_A, media_server};

#[tokio::test]
async fn tl738_media_server_read_tracks_http_and_output_binding_without_changing_citp_or_heads() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let mut fixture = media_server("2 layers", [127, 0, 0, 1], OUTPUT_A);
    let mut original_layers = None;
    for (http_port, binding) in [
        (None, OUTPUT_A),
        (Some(18080), OUTPUT_A),
        (Some(18081), OUTPUT_A),
        (Some(18081), "00000000-0000-4000-8000-00000000000b"),
    ] {
        fixture.direct_control.as_mut().unwrap().http_port = http_port;
        fixture.internal_bindings.output = Some(binding.to_owned());
        state
            .output
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture.clone()].into(),
                ..Default::default()
            })
            .unwrap();
        let response = app
            .clone()
            .oneshot(
                Request::get("/api/v2/media-servers")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = json(response).await;
        let row = &body["fixtures"][0];
        assert_eq!(row["fixture_id"], serde_json::json!(fixture.fixture_id));
        assert_eq!(row["endpoint"]["ip_address"], "127.0.0.1");
        assert_eq!(row["endpoint"]["port"], 4809);
        assert_eq!(row["endpoint"]["http_port"], serde_json::json!(http_port));
        assert_eq!(
            row["output_binding"], binding,
            "persisted output identity must be authoritative"
        );
        if let Some(layers) = &original_layers {
            assert_eq!(
                &row["layers"], layers,
                "port/binding edits do not replace logical heads"
            );
        } else {
            original_layers = Some(row["layers"].clone());
        }
    }
    fixture.internal_bindings.output = None;
    state
        .output
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            ..Default::default()
        })
        .unwrap();
    let response = app
        .oneshot(
            Request::get("/api/v2/media-servers")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        json(response).await["fixtures"][0]["output_binding"],
        serde_json::Value::Null
    );
    std::fs::remove_dir_all(data_dir).unwrap();
}
