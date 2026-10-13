use super::*;
use crate::runtime::cue_media_previews_http::tests::{OUTPUT_A, media_server};

#[test]
fn tl738_native_endpoint_uses_legacy_default_or_explicit_validated_port() {
    let mut fixture = media_server("2 layers", [127, 0, 0, 1], OUTPUT_A);
    assert_eq!(
        native_media_fixture_endpoint(&fixture).unwrap().port(),
        8080
    );
    fixture.direct_control.as_mut().unwrap().http_port = Some(18080);
    assert_eq!(
        native_media_fixture_endpoint(&fixture).unwrap().to_string(),
        "127.0.0.1:18080"
    );
    fixture.direct_control.as_mut().unwrap().http_port = Some(0);
    assert!(native_media_fixture_endpoint(&fixture).is_err());
    assert!(
        light_fixture::validate_patch(&[fixture])
            .unwrap_err()
            .to_string()
            .contains("http_port")
    );
}

#[tokio::test]
async fn tl738_two_native_http_instances_are_isolated_and_a_refusal_never_falls_back() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let count_a = Arc::new(AtomicUsize::new(0));
    let count_b = Arc::new(AtomicUsize::new(0));
    let mut servers = Vec::new();
    let mut ports = Vec::new();
    for (instance, count) in [("A", count_a.clone()), ("B", count_b.clone())] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        ports.push(listener.local_addr().unwrap().port());
        let app = Router::new().route(
            "/api/v2/health",
            get(move || {
                let count = count.clone();
                async move {
                    count.fetch_add(1, Ordering::Relaxed);
                    Json(serde_json::json!({"status":"ok", "instance":instance,
                    "outputs":1, "catalogRevision":1, "catalogItems":1}))
                }
            }),
        );
        servers.push(tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap()
        }));
    }
    let mut fixture = media_server("2 layers", [127, 0, 0, 1], OUTPUT_A);
    let client = native_media_client().unwrap();
    for (port, expected) in ports.iter().zip(["A", "B"]) {
        fixture.direct_control.as_mut().unwrap().http_port = Some(*port);
        let endpoint = native_media_fixture_endpoint(&fixture).unwrap();
        assert_eq!(
            endpoint.port(),
            *port,
            "guard before issuing a scoped request"
        );
        let response: NativeMediaHealth =
            native_media_get(&client, &format!("http://{endpoint}/api/v2/health"))
                .await
                .unwrap();
        assert_eq!(response.instance, expected);
    }
    servers.remove(0).abort();
    fixture.direct_control.as_mut().unwrap().http_port = Some(ports[0]);
    let endpoint = native_media_fixture_endpoint(&fixture).unwrap();
    assert!(
        native_media_get::<NativeMediaHealth>(&client, &format!("http://{endpoint}/api/v2/health"))
            .await
            .is_err()
    );
    assert_eq!(count_a.load(Ordering::Relaxed), 1);
    assert_eq!(
        count_b.load(Ordering::Relaxed),
        1,
        "offline A must not query another output/server"
    );
    servers.remove(0).abort();
}
