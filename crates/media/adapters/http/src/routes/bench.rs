//! One server every route's tests drive.
//!
//! The real router, the real reducer, and the real replay window; only the filesystem and the
//! clock are stand-ins. A test proving that an unstored edit was not applied is therefore proving
//! the code an operator runs.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use arc_swap::ArcSwap;
use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt as _;
use media_application::MediaConfiguration;
use media_application::configuration::OutputConfiguration;
use media_domain::catalog::CatalogSnapshot;
use media_domain::{LayerPersonality, MediaState, OutputId, OutputState, Timestamp};
use std::sync::Arc;
use tower::ServiceExt as _;

use crate::diagnostics::Diagnostics;
use crate::routes::{ApiState, OutputPreviewFrame, router};

pub(crate) struct Bench {
    /// What was written, so a test can prove an edit reached storage without a filesystem.
    pub(crate) stored: Arc<Mutex<Vec<MediaConfiguration>>>,
    /// Set to make the next write fail.
    pub(crate) refuse: Arc<AtomicBool>,
    /// How many times the process was told an accepted edit had been published.
    pub(crate) applied: Arc<AtomicUsize>,
    pub(crate) router: Router,
    pub(crate) output: OutputId,
    pub(crate) state: Arc<ArcSwap<MediaState>>,
    pub(crate) configuration: Arc<ArcSwap<MediaConfiguration>>,
    pub(crate) preview_frame: Arc<Mutex<Option<OutputPreviewFrame>>>,
}

pub(crate) fn bench() -> Bench {
    bench_with(Diagnostics::default())
}

pub(crate) fn bench_with(diagnostics: Diagnostics) -> Bench {
    let mut configured = OutputConfiguration::new("Main");
    configured.personality = LayerPersonality::TwoLayers;
    let output = configured.id;
    let active_configuration = Arc::new(MediaConfiguration {
        outputs: vec![configured],
        ..Default::default()
    });
    let configuration = Arc::new(ArcSwap::from(Arc::clone(&active_configuration)));
    let state = Arc::new(ArcSwap::from_pointee(MediaState::with_outputs(vec![
        OutputState::new(output, LayerPersonality::TwoLayers),
    ])));

    let stored: Arc<Mutex<Vec<MediaConfiguration>>> = Arc::new(Mutex::new(Vec::new()));
    let writes = Arc::clone(&stored);
    let refuse = Arc::new(AtomicBool::new(false));
    let refusing = Arc::clone(&refuse);
    let applied = Arc::new(AtomicUsize::new(0));
    let applying = Arc::clone(&applied);
    let preview_frame = Arc::new(Mutex::new(None));
    let requested_preview = Arc::clone(&preview_frame);

    let api = ApiState {
        configuration: Arc::clone(&configuration),
        active_configuration,
        administration_endpoint: "127.0.0.1:18080".to_owned(),
        state: state.clone(),
        catalog: Arc::new(ArcSwap::from_pointee(CatalogSnapshot::default())),
        now: Arc::new(|| Timestamp::from_millis(0)),
        persist: Arc::new(move |configuration: &MediaConfiguration| {
            if refusing.load(Ordering::SeqCst) {
                return Err("the disk said no".to_owned());
            }
            writes.lock().unwrap().push(configuration.clone());
            Ok(())
        }),
        apply: Arc::new(move |_| {
            applying.fetch_add(1, Ordering::SeqCst);
        }),
        preview: Arc::new(move |_, _, _| requested_preview.lock().unwrap().clone()),
        diagnostics,
        replays: Arc::new(crate::replay::Replays::new()),
        upload_body_limit: 8 * 1024 * 1024 * 1024 + 1024 * 1024,
    };
    Bench {
        router: router(api),
        output,
        state,
        configuration,
        stored,
        refuse,
        applied,
        preview_frame,
    }
}

pub(crate) async fn send(
    router: &Router,
    request: Request<Body>,
) -> (StatusCode, serde_json::Value) {
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
    };
    (status, value)
}

pub(crate) fn get(uri: String) -> Request<Body> {
    Request::builder().uri(uri).body(Body::empty()).unwrap()
}

pub(crate) fn post(uri: String, body: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_owned()))
        .unwrap()
}
