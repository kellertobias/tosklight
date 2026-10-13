use super::*;
use crate::runtime::cue_media_previews_http::tests::media_server as patched;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct Endpoint {
    port: u16,
    calls: Arc<AtomicUsize>,
    job: tokio::task::JoinHandle<()>,
}
impl Drop for Endpoint {
    fn drop(&mut self) {
        self.job.abort();
    }
}
async fn endpoint(instance: &'static str, output: Uuid) -> Endpoint {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = calls.clone();
    let router = axum::Router::new()
        .route(
            "/api/v2/health",
            get(move || {
                let seen = seen.clone();
                async move {
                    seen.fetch_add(1, Ordering::SeqCst);
                    let mut value = health().0;
                    value["instance"] = serde_json::json!(instance);
                    axum::Json(value)
                }
            }),
        )
        .route(
            "/api/v2/outputs",
            get(move || async move { axum::Json(serde_json::json!([{"id":output}])) }),
        )
        .route(
            "/api/v2/outputs/{id}/configuration",
            get(move || async move {
                let mut value = reference();
                value["id"] = serde_json::json!(output);
                axum::Json(value)
            }),
        );
    let job = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    Endpoint { port, calls, job }
}
fn fixture(output: Uuid, port: Option<u16>, citp: u16) -> light_fixture::PatchedFixture {
    let mut fixture = patched("2 layers", [127, 0, 0, 1], &output.to_string());
    fixture.name = "Renamed native server".into();
    let endpoint = fixture.direct_control.as_mut().unwrap();
    endpoint.port = citp;
    endpoint.http_port = port;
    fixture
}
#[tokio::test]
async fn configured_same_host_servers_keep_distinct_http_authority_and_survive_udp_failure() {
    let a_id = Uuid::new_v4();
    let b_id = Uuid::new_v4();
    let a = endpoint("A", a_id).await;
    let b = endpoint("B", b_id).await;
    let fixtures = vec![
        fixture(a_id, Some(a.port), 14809),
        fixture(b_id, Some(b.port), 14810),
    ];
    let mut peer = citp();
    peer.port = 14809;
    let result = inspect_media_discovery(&fixtures, Ok(vec![peer]))
        .await
        .unwrap();
    assert_eq!(result.servers.len(), 2);
    for (server, expected, output) in [
        (&result.servers[0], "A", a_id),
        (&result.servers[1], "B", b_id),
    ] {
        assert_eq!(server.instance.as_deref(), Some(expected));
        assert_eq!(server.outputs[0].id, output);
        assert_eq!(server.outputs[0].mode.as_deref(), Some("8 layers"));
    }
    assert_eq!(result.discovery_error, None);
    let failed = inspect_media_discovery(&fixtures, Err("UDP interface unavailable".into()))
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(&failed.servers).unwrap(),
        serde_json::to_value(&result.servers).unwrap()
    );
    assert!(
        failed
            .discovery_error
            .unwrap()
            .contains("UDP interface unavailable")
    );
    assert_eq!(a.calls.load(Ordering::SeqCst), 2);
    assert_eq!(b.calls.load(Ordering::SeqCst), 2);
}
#[tokio::test]
async fn conflicting_configured_tuple_refuses_probe_but_identical_authority_deduplicates() {
    let output = Uuid::new_v4();
    let a = endpoint("A", output).await;
    let b = endpoint("B", output).await;
    let first = fixture(output, Some(a.port), 14809);
    let conflict = inspect_media_discovery(
        &[first.clone(), fixture(output, Some(b.port), 14809)],
        Ok(vec![]),
    )
    .await
    .unwrap();
    assert_eq!(conflict.servers.len(), 1);
    assert!(
        conflict.servers[0]
            .error
            .as_deref()
            .unwrap()
            .contains("conflicting")
    );
    assert!(conflict.servers[0].outputs.is_empty());
    assert_eq!(a.calls.load(Ordering::SeqCst), 0);
    assert_eq!(b.calls.load(Ordering::SeqCst), 0);
    let duplicate = inspect_media_discovery(&[first.clone(), first], Ok(vec![]))
        .await
        .unwrap();
    assert_eq!(duplicate.servers.len(), 1);
    assert_eq!(duplicate.servers[0].instance.as_deref(), Some("A"));
    assert_eq!(a.calls.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn generic_citp_is_listed_without_guessing_native_http() {
    let mut peer = citp();
    peer.host = "127.0.0.1".into();
    let result = inspect_media_discovery(&[], Ok(vec![peer])).await.unwrap();
    assert_eq!(result.servers.len(), 1);
    assert_eq!(result.servers[0].status, "Unavailable");
    assert!(
        result.servers[0]
            .error
            .as_deref()
            .unwrap()
            .contains("will not be guessed")
    );
}
#[test]
fn address_uses_legacy_default_only_without_configured_native_host() {
    let host = Ipv4Addr::LOCALHOST;
    let output = Uuid::new_v4();
    assert_eq!(
        discovered_address_endpoint(&[], host, output)
            .unwrap()
            .port(),
        8080
    );
    let default = fixture(output, None, 4809);
    assert_eq!(
        discovered_address_endpoint(&[default.clone()], host, output)
            .unwrap()
            .port(),
        8080
    );
    assert!(discovered_address_endpoint(&[default], host, Uuid::new_v4()).is_err());
    let invalid = fixture(output, Some(0), 4809);
    assert!(discovered_address_endpoint(&[invalid], host, output).is_err());
}

#[test]
fn unconfigured_probe_requires_the_production_native_announcement() {
    let mut peer = citp();
    assert_eq!(unconfigured_native_endpoint(&peer), None);
    peer.name = "ToskLight Pixel — rack-a".into();
    assert_eq!(unconfigured_native_endpoint(&peer).unwrap().port(), 8080);
    peer.name = "ToskLight Pixel — ".into();
    assert_eq!(unconfigured_native_endpoint(&peer), None);
}
