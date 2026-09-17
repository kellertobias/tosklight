use super::*;

/// Shared with `crates/media/adapters/http/src/routes/desk_contract_tests.rs`, where the current
/// Media Server is asserted to answer these exact fields.
const MEDIA_REFERENCE: &str = r#"{"id":"00000000-0000-4000-8000-000000000040","name":"Main","personality":"eight-layers","protocol":"sacn","universe":4,"startAddress":1,"dmxPendingRestart":true,"tempoSource":"speed-group","speedGroup":3}"#;

const OUTPUT_ID: &str = "00000000-0000-4000-8000-000000000040";

fn reference() -> serde_json::Value {
    serde_json::from_str(MEDIA_REFERENCE).unwrap()
}

#[test]
fn a_current_media_output_maps_to_its_desk_fixture_mode() {
    let output = classify_media_output(None, reference()).expect("a readable output");
    assert_eq!(output.id.to_string(), OUTPUT_ID);
    assert_eq!(output.mode.as_deref(), Some("8 layers"));
    assert_eq!(output.personality, "eight-layers");
    assert_eq!(output.protocol, "sacn");
    assert_eq!((output.universe, output.start_address), (4, 1));
    assert!(output.dmx_pending_restart);
    assert_eq!(output.tempo_source.as_deref(), Some("speed-group"));
    assert_eq!(output.speed_group, Some(3));
    assert_eq!(output.issue, None);

    let mut two = reference();
    two["personality"] = serde_json::json!("two-layers");
    let two = classify_media_output(None, two).unwrap();
    assert_eq!(two.mode.as_deref(), Some("2 layers"));
}

#[test]
fn every_media_personality_names_a_shipped_profile_mode() {
    let package = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../assets/fixture-library/tosklight--media-server.toskfixture"),
    )
    .unwrap();
    let profile = light_fixture::read_fixture_package(&package).unwrap();
    let modes = profile
        .modes
        .iter()
        .map(|mode| mode.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        modes,
        MEDIA_PERSONALITY_MODES
            .iter()
            .map(|(_, mode)| *mode)
            .collect::<Vec<_>>()
    );
}

#[test]
fn an_unknown_personality_is_listed_but_not_patchable() {
    let mut value = reference();
    value["personality"] = serde_json::json!("sixteen-layers");
    let output = classify_media_output(None, value).unwrap();
    assert_eq!(output.mode, None);
    let issue = output.issue.expect("an operator-facing reason");
    assert!(issue.contains("sixteen-layers"), "{issue}");
    assert!(issue.contains("Settings > Network & DMX"), "{issue}");
}

#[test]
fn a_server_with_retired_channel_layouts_is_outdated() {
    let mut value = reference();
    value["personalityLayout"] = serde_json::json!("legacy");
    let output = classify_media_output(None, value).unwrap();
    assert_eq!(output.mode, None);
    assert_eq!((output.universe, output.start_address), (4, 1));
    assert!(output.issue.unwrap().contains("Update ToskLight Media"));
}

#[test]
fn a_configuration_without_current_fields_is_outdated_under_its_listed_id() {
    let fallback = OUTPUT_ID.parse::<Uuid>().ok();
    let output = classify_media_output(fallback, serde_json::json!({})).unwrap();
    assert_eq!(output.id.to_string(), OUTPUT_ID);
    assert_eq!(output.name, "Output");
    assert_eq!(output.mode, None);
    assert!(output.issue.unwrap().contains("needs an update"));
    assert!(classify_media_output(None, serde_json::json!({})).is_none());
}

#[test]
fn media_refusals_are_worded_as_operator_actions() {
    let refusal = |code: &str, message: &str| {
        native_media_refusal(
            reqwest::StatusCode::BAD_REQUEST,
            serde_json::from_value(serde_json::json!({"code": code, "message": message})).ok(),
        )
    };
    assert!(
        refusal("configuration-not-written", "the change could not be saved")
            .contains("Check free space and write access")
    );
    let invalid = refusal("output-configuration-invalid", "address 400 overlaps Side");
    assert!(invalid.contains("address 400 overlaps Side"), "{invalid}");
    assert!(invalid.contains("Choose another value"), "{invalid}");
    assert!(refusal("unknown-output", "no output").contains("Refresh discovery"));
    assert!(
        native_media_refusal(reqwest::StatusCode::NOT_FOUND, None)
            .contains("Update ToskLight Media")
    );
}

async fn serve(router: axum::Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    format!("http://{address}/api/v2")
}

fn health() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "status": "ready",
        "instance": "rack-a",
        "outputs": 1,
        "catalogRevision": 1,
        "catalogItems": 0
    }))
}

fn citp() -> light_media::DiscoveredCitpServer {
    light_media::DiscoveredCitpServer {
        name: "Pixel Rack".into(),
        host: "127.0.0.1".into(),
        port: 4809,
    }
}

fn media_server(configuration: serde_json::Value) -> axum::Router {
    axum::Router::new()
        .route("/api/v2/health", get(|| async { health() }))
        .route(
            "/api/v2/outputs",
            get(|| async { axum::Json(serde_json::json!([{ "id": OUTPUT_ID }])) }),
        )
        .route(
            "/api/v2/outputs/{output}/configuration",
            get(move || {
                let configuration = configuration.clone();
                async move { axum::Json(configuration) }
            }),
        )
}

#[tokio::test]
async fn discovery_lists_a_configured_media_server_as_patchable() {
    let base = serve(media_server(reference())).await;
    let server =
        inspect_discovered_native_media_server(native_media_client().unwrap(), citp(), base).await;
    assert_eq!(server.name, "ToskLight Pixel Media - rack-a");
    assert_eq!(server.error, None);
    assert_eq!(server.outputs.len(), 1);
    assert_eq!(server.outputs[0].mode.as_deref(), Some("8 layers"));
}

#[tokio::test]
async fn discovery_marks_an_outdated_media_server() {
    let mut outdated = reference();
    outdated["personalityLayout"] = serde_json::json!("mask-positioning");
    let base = serve(media_server(outdated)).await;
    let server =
        inspect_discovered_native_media_server(native_media_client().unwrap(), citp(), base).await;
    assert_eq!(server.outputs.len(), 1);
    assert!(server.outputs[0].issue.is_some());
    assert!(server.error.unwrap().contains("needs an update"));
}

#[tokio::test]
async fn discovery_reports_an_unreachable_media_server() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/api/v2", listener.local_addr().unwrap());
    drop(listener);
    let server =
        inspect_discovered_native_media_server(native_media_client().unwrap(), citp(), base).await;
    assert_eq!(server.status, "Unavailable");
    assert!(server.outputs.is_empty());
    assert!(server.error.unwrap().contains("refresh discovery"));
}

#[tokio::test]
async fn an_address_update_refusal_names_the_rejected_value() {
    let router = axum::Router::new().route(
        "/api/v2/outputs/{output}/configuration/update",
        axum::routing::post(|| async {
            (
                StatusCode::BAD_REQUEST,
                axum::Json(serde_json::json!({
                    "code": "output-configuration-invalid",
                    "message": "the 512-slot block starting at 2 does not fit the universe"
                })),
            )
        }),
    );
    let base = serve(router).await;
    let request = DiscoveredMediaAddressUpdateRequest {
        request_id: "address-1".into(),
        host: "127.0.0.1".into(),
        output_id: OUTPUT_ID.parse().unwrap(),
        universe: 4,
        start_address: 2,
        protocol: None,
    };
    let error = update_media_output_address(&native_media_client().unwrap(), &base, &request)
        .await
        .expect_err("the Media Server refused");
    assert_eq!(error.status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(
        error.message.contains("does not fit the universe"),
        "{}",
        error.message
    );
    assert!(
        error.message.contains("Choose another value"),
        "{}",
        error.message
    );
}

#[tokio::test]
async fn an_accepted_address_update_answers_the_authoritative_output() {
    let router = axum::Router::new().route(
        "/api/v2/outputs/{output}/configuration/update",
        axum::routing::post(|| async { axum::Json(reference()) }),
    );
    let base = serve(router).await;
    let request = DiscoveredMediaAddressUpdateRequest {
        request_id: "address-2".into(),
        host: "127.0.0.1".into(),
        output_id: OUTPUT_ID.parse().unwrap(),
        universe: 4,
        start_address: 1,
        protocol: None,
    };
    let output = update_media_output_address(&native_media_client().unwrap(), &base, &request)
        .await
        .unwrap();
    assert!(output.dmx_pending_restart);
    assert_eq!(output.mode.as_deref(), Some("8 layers"));
}

#[tokio::test]
async fn a_coordinated_update_sends_the_desk_route_protocol_and_universe() {
    let received = std::sync::Arc::new(std::sync::Mutex::new(None::<serde_json::Value>));
    let seen = received.clone();
    let router = axum::Router::new().route(
        "/api/v2/outputs/{output}/configuration/update",
        axum::routing::post(move |axum::Json(body): axum::Json<serde_json::Value>| {
            let seen = seen.clone();
            async move {
                let mut answer = reference();
                answer["protocol"] = body["protocol"].clone();
                answer["universe"] = body["universe"].clone();
                answer["startAddress"] = body["startAddress"].clone();
                *seen.lock().unwrap() = Some(body);
                axum::Json(answer)
            }
        }),
    );
    let base = serve(router).await;
    let request = DiscoveredMediaAddressUpdateRequest {
        request_id: "address-3".into(),
        host: "127.0.0.1".into(),
        output_id: OUTPUT_ID.parse().unwrap(),
        universe: 0,
        start_address: 17,
        protocol: Some("art-net".into()),
    };
    let output = update_media_output_address(&native_media_client().unwrap(), &base, &request)
        .await
        .unwrap();
    let body = received
        .lock()
        .unwrap()
        .clone()
        .expect("the server was asked");
    assert_eq!(body["protocol"], "art-net");
    assert_eq!(body["universe"], 0);
    assert_eq!(body["startAddress"], 17);
    assert_eq!(output.protocol, "art-net");
    assert_eq!((output.universe, output.start_address), (0, 17));
}

#[test]
fn an_address_update_without_a_protocol_keeps_the_server_protocol() {
    let request = DiscoveredMediaAddressUpdateRequest {
        request_id: "address-4".into(),
        host: "127.0.0.1".into(),
        output_id: OUTPUT_ID.parse().unwrap(),
        universe: 4,
        start_address: 1,
        protocol: None,
    };
    assert!(
        media_address_update_body(&request)
            .get("protocol")
            .is_none()
    );
}

#[test]
fn media_input_universes_are_checked_per_protocol() {
    assert!(validate_media_input_universe(None, 0).is_ok());
    assert!(validate_media_input_universe(Some("art-net"), 0).is_ok());
    assert!(validate_media_input_universe(Some("art-net"), 32_767).is_ok());
    assert!(validate_media_input_universe(Some("art-net"), 32_768).is_err());
    assert!(validate_media_input_universe(Some("sacn"), 0).is_err());
    assert!(validate_media_input_universe(Some("sacn"), 63_999).is_ok());
    assert!(validate_media_input_universe(Some("sacn"), 64_000).is_err());
    let unsupported = validate_media_input_universe(Some("kinet"), 1).unwrap_err();
    assert!(
        unsupported.message.contains("art-net or sacn"),
        "{}",
        unsupported.message
    );
}
