//! The routes.
//!
//! Reads return whole-object snapshots. Writes are intent-shaped and go through the application
//! reducer, so the API never decides what a value means — it carries a request in and a projection
//! out. Live-control actions that need no payload are plain `GET` URLs with `Cache-Control:
//! no-store`, so a microcontroller with minimal storage can trigger one.

use std::sync::Arc;

use arc_swap::ArcSwap;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use media_application::MediaConfiguration;
use media_domain::catalog::CatalogSnapshot;
use media_domain::{
    Applied, Command, CommandKind, CommandSource, MediaAddress, MediaState, OutputId, Timestamp,
    apply,
};

use crate::assets;
use crate::error::ApiError;
use crate::tolerant::TolerantJson;
use crate::wire::{CatalogView, Health, OutputView, UpdateLayer, UpdateVisualizer, VisualizerView};

/// Writes an accepted configuration wherever it belongs.
///
/// A function rather than a path, because the API adapter does not touch the filesystem — and
/// because a test needs to prove persistence without one.
pub type PersistConfiguration =
    Arc<dyn Fn(&MediaConfiguration) -> Result<(), String> + Send + Sync>;

/// Everything the routes read and write.
#[derive(Clone)]
pub struct ApiState {
    /// The live configuration. Swapped when an edit is accepted, so a read after a write sees it.
    pub configuration: Arc<ArcSwap<MediaConfiguration>>,
    pub state: Arc<ArcSwap<MediaState>>,
    pub catalog: Arc<ArcSwap<CatalogSnapshot>>,
    /// Stamps commands. Injected so the API's behaviour is testable without real time passing.
    pub now: Arc<dyn Fn() -> Timestamp + Send + Sync>,
    pub persist: PersistConfiguration,
    /// What recent edits produced, so a retry is answered rather than executed again.
    pub replays: Arc<crate::replay::Replays>,
}

impl std::fmt::Debug for ApiState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("ApiState").finish_non_exhaustive()
    }
}

/// The versioned API.
///
/// The version is in the path because an incompatible change gets a new one rather than silently
/// altering what `/v2` means to a client already deployed against it.
pub fn router(state: ApiState) -> Router {
    Router::new()
        .route("/api/v2/health", get(health))
        .route("/api/v2/catalog", get(catalog))
        .route("/api/v2/visualizers", get(visualizers))
        .route(
            "/api/v2/visualizers/{folder}/{file}/update",
            post(update_visualizer),
        )
        .route("/api/v2/outputs", get(outputs))
        .route("/api/v2/outputs/{output}/state", get(output_state))
        .route(
            "/api/v2/outputs/{output}/layers/{layer}/update",
            post(update_layer),
        )
        .route(
            "/api/v2/outputs/{output}/layers/{layer}/reset",
            get(reset_layer),
        )
        .with_state(state)
        // Anything the API did not claim is the administration frontend: its shell, its assets,
        // and its client-side routes.
        .fallback(assets::serve)
}

async fn health(State(state): State<ApiState>) -> impl IntoResponse {
    let catalog = state.catalog.load();
    axum::Json(Health {
        status: "ok".to_owned(),
        instance: state.configuration.load().instance_id.as_str().to_owned(),
        outputs: state.state.load().outputs.len(),
        catalog_revision: catalog.revision.value(),
        catalog_items: catalog.item_count(),
    })
}

async fn catalog(State(state): State<ApiState>) -> impl IntoResponse {
    axum::Json(CatalogView::of(&state.catalog.load()))
}

/// Which generated visualizer answers at which address.
///
/// Configuration rather than state: it changes when an operator reassigns an address, not from
/// frame to frame, so it is read once and not polled.
async fn visualizers(State(state): State<ApiState>) -> impl IntoResponse {
    axum::Json(VisualizerView::all(&state.configuration.load().visualizers))
}

/// Edits one configured visualizer.
///
/// An object-intent update: only the fields being changed travel, and the request id makes a
/// retry safe. Unlike a layer selection this is stored configuration, so it is written to disk
/// before it is answered — an operator who tuned a look and restarted must find it there.
async fn update_visualizer(
    State(state): State<ApiState>,
    Path((folder, file)): Path<(u8, u8)>,
    TolerantJson(body): TolerantJson<UpdateVisualizer>,
) -> Result<Response, ApiError> {
    if body.request_id.trim().is_empty() {
        return Err(ApiError::bad_request(
            "missing-request-id",
            "an edit must carry a request id so a retry cannot become a second edit",
        ));
    }
    if let Some(stored) = state.replays.stored(&body.request_id) {
        return Ok(([(header::CONTENT_TYPE, "application/json")], stored).into_response());
    }

    let address = MediaAddress::new(folder, file);
    let mut configuration = MediaConfiguration::clone(&state.configuration.load());
    let entry = configuration
        .visualizers
        .entries
        .iter_mut()
        .find(|entry| entry.address == address)
        .ok_or_else(|| {
            ApiError::not_found(
                "unknown-visualizer",
                format!("no visualizer answers at {address}"),
            )
        })?;

    if let Some(name) = body.name {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(ApiError::bad_request(
                "empty-name",
                "a visualizer needs a name an operator can find it by",
            ));
        }
        entry.configuration.name = trimmed.to_owned();
    }
    if let Some(parameters) = body.parameters {
        entry.configuration.parameters = parameters.into_parameters();
    }

    let view = VisualizerView::of(address, &entry.configuration);
    (state.persist)(&configuration).map_err(|detail| {
        tracing::error!(%detail, "an accepted visualizer edit could not be stored");
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "configuration-not-written",
            "the change could not be saved; it has not been applied",
        )
    })?;
    state.configuration.store(Arc::new(configuration));

    let serialized = serde_json::to_string(&view).unwrap_or_default();
    state.replays.remember(&body.request_id, serialized.clone());
    Ok(([(header::CONTENT_TYPE, "application/json")], serialized).into_response())
}

async fn outputs(State(state): State<ApiState>) -> impl IntoResponse {
    let media = state.state.load();
    let views: Vec<OutputView> = media
        .outputs
        .iter()
        .map(|output| view_of(&state, output, (state.now)()))
        .collect();
    axum::Json(views)
}

async fn output_state(
    State(state): State<ApiState>,
    Path(output): Path<String>,
) -> Result<Response, ApiError> {
    let id = parse_output(&output)?;
    let media = state.state.load();
    let found = media.output(id).ok_or_else(|| unknown_output(id))?;
    Ok(axum::Json(view_of(&state, found, (state.now)())).into_response())
}

async fn update_layer(
    State(state): State<ApiState>,
    Path((output, layer)): Path<(String, usize)>,
    TolerantJson(body): TolerantJson<UpdateLayer>,
) -> Result<Response, ApiError> {
    let id = parse_output(&output)?;
    let now = (state.now)();

    // Both halves of the update are separate commands, so a dimmer change never rewrites a
    // selection and a selection change never rewrites a dimmer.
    let mut commands = Vec::new();
    let current = {
        let media = state.state.load();
        let found = media.output(id).ok_or_else(|| unknown_output(id))?;
        found
            .layer(layer)
            .ok_or_else(|| {
                ApiError::not_found("unknown-layer", format!("this output has no layer {layer}"))
            })?
            .address
    };

    if body.changes_address() {
        commands.push(CommandKind::SelectMedia {
            output: id,
            layer,
            address: body.address(current),
        });
    }
    if let Some(dimmer) = body.dimmer {
        if !dimmer.is_finite() || !(0.0..=1.0).contains(&dimmer) {
            return Err(ApiError::bad_request(
                "dimmer-out-of-range",
                "dimmer must be between 0 and 1",
            ));
        }
        commands.push(CommandKind::SetLayerDimmer {
            output: id,
            layer,
            dimmer,
        });
    }

    submit(&state, commands, now)?;
    let media = state.state.load();
    let found = media.output(id).ok_or_else(|| unknown_output(id))?;
    Ok(axum::Json(view_of(&state, found, now)).into_response())
}

/// Restarts a layer's media. A live-control action with no payload, so it is a `GET` an
/// integrator can trigger from a URL bar or a microcontroller.
async fn reset_layer(
    State(state): State<ApiState>,
    Path((output, layer)): Path<(String, usize)>,
) -> Result<Response, ApiError> {
    let id = parse_output(&output)?;
    let now = (state.now)();
    submit(
        &state,
        vec![CommandKind::ResetLayer { output: id, layer }],
        now,
    )?;

    // A side-effecting GET must never be cached, or a proxy would swallow the second press.
    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        StatusCode::NO_CONTENT,
    )
        .into_response())
}

/// Applies commands through the reducer and publishes one new snapshot.
fn submit(state: &ApiState, commands: Vec<CommandKind>, now: Timestamp) -> Result<(), ApiError> {
    if commands.is_empty() {
        return Ok(());
    }
    let mut next = MediaState::clone(&state.state.load());
    let mut published = false;

    for kind in commands {
        let command = Command::new(kind, CommandSource::Web, now);
        match apply(&mut next, &command) {
            Applied::Changed => published = true,
            Applied::Unchanged => {}
            Applied::RejectedNotOwner => {
                return Err(ApiError::new(
                    StatusCode::CONFLICT,
                    "dmx-owns-this",
                    "a lighting desk is currently driving this output; its values cannot be set \
                     from here until it stops sending",
                ));
            }
            Applied::RejectedUnknownOutput => {
                return Err(ApiError::not_found("unknown-output", "no such output"));
            }
            Applied::RejectedUnknownLayer => {
                return Err(ApiError::not_found("unknown-layer", "no such layer"));
            }
        }
    }

    if published {
        state.state.store(Arc::new(next));
    }
    Ok(())
}

fn view_of(state: &ApiState, output: &media_domain::OutputState, now: Timestamp) -> OutputView {
    let configuration = state.configuration.load();
    let name = configuration
        .output(output.id)
        .map(|configured| configured.name.to_string())
        .unwrap_or_else(|| output.id.to_string());

    OutputView::of(output, name, output.ownership.dmx_is_active(now))
}

fn parse_output(raw: &str) -> Result<OutputId, ApiError> {
    uuid::Uuid::parse_str(raw)
        .map(OutputId::from_uuid)
        .map_err(|_| ApiError::bad_request("malformed-output-id", "that is not an output id"))
}

fn unknown_output(id: OutputId) -> ApiError {
    ApiError::not_found("unknown-output", format!("no output {id}"))
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};

    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt as _;
    use media_application::configuration::OutputConfiguration;
    use media_domain::{LayerPersonality, MediaAddress, OutputState};
    use tower::ServiceExt as _;

    use super::*;

    struct Bench {
        stored: Arc<Mutex<Vec<MediaConfiguration>>>,
        refuse: Arc<AtomicBool>,
        router: Router,
        output: OutputId,
        state: Arc<ArcSwap<MediaState>>,
    }

    fn bench() -> Bench {
        let mut configured = OutputConfiguration::new("Main");
        configured.personality = LayerPersonality::TwoLayers;
        let output = configured.id;
        let configuration = MediaConfiguration {
            outputs: vec![configured],
            ..Default::default()
        };
        let state = Arc::new(ArcSwap::from_pointee(MediaState::with_outputs(vec![
            OutputState::new(output, LayerPersonality::TwoLayers),
        ])));

        // What was written, so a test can prove an edit reached storage without a filesystem.
        let stored: Arc<Mutex<Vec<MediaConfiguration>>> = Arc::new(Mutex::new(Vec::new()));
        let writes = Arc::clone(&stored);
        let refuse = Arc::new(AtomicBool::new(false));
        let refusing = Arc::clone(&refuse);

        let api = ApiState {
            configuration: Arc::new(ArcSwap::from_pointee(configuration)),
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
            replays: Arc::new(crate::replay::Replays::new()),
        };
        Bench {
            router: router(api),
            output,
            state,
            stored,
            refuse,
        }
    }

    async fn send(router: &Router, request: Request<Body>) -> (StatusCode, serde_json::Value) {
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

    fn get(uri: String) -> Request<Body> {
        Request::builder().uri(uri).body(Body::empty()).unwrap()
    }

    fn post(uri: String, body: &str) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_owned()))
            .unwrap()
    }

    #[tokio::test]
    async fn health_reports_what_the_process_is_running() {
        let bench = bench();
        let (status, body) = send(&bench.router, get("/api/v2/health".into())).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "ok");
        assert_eq!(body["outputs"], 1);
    }

    #[tokio::test]
    async fn an_output_returns_its_whole_state() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            get(format!("/api/v2/outputs/{}/state", bench.output)),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["name"], "Main");
        assert_eq!(body["layers"].as_array().unwrap().len(), 2);
        assert_eq!(body["layers"][0]["playMode"], "Loop");
        assert_eq!(body["dmxActive"], false);
    }

    #[tokio::test]
    async fn an_update_carries_only_what_it_changes() {
        let bench = bench();
        let uri = format!("/api/v2/outputs/{}/layers/0/update", bench.output);

        let (status, body) =
            send(&bench.router, post(uri.clone(), r#"{"folder":3,"file":7}"#)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["layers"][0]["address"]["folder"], 3);
        assert_eq!(body["layers"][0]["address"]["file"], 7);
        assert_eq!(body["layers"][0]["dimmer"], 1.0);

        // A dimmer change must not disturb the selection.
        let (_, body) = send(&bench.router, post(uri, r#"{"dimmer":0.25}"#)).await;
        assert_eq!(body["layers"][0]["dimmer"], 0.25);
        assert_eq!(
            body["layers"][0]["address"]["folder"], 3,
            "the selection survived"
        );
        assert_eq!(body["layers"][0]["address"]["file"], 7);
    }

    #[tokio::test]
    async fn unknown_fields_are_accepted_rather_than_rejected() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            post(
                format!("/api/v2/outputs/{}/layers/0/update", bench.output),
                r#"{"dimmer":0.5,"somethingNewer":true,"nested":{"deep":1}}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "a newer client must not be refused");
        assert_eq!(body["layers"][0]["dimmer"], 0.5);
    }

    #[tokio::test]
    async fn a_malformed_body_names_the_problem_rather_than_crashing() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            post(
                format!("/api/v2/outputs/{}/layers/0/update", bench.output),
                r#"{"dimmer":"loud"}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "body-invalid");
        assert!(body["message"].as_str().unwrap().contains("dimmer"));
    }

    #[tokio::test]
    async fn a_dimmer_outside_its_range_is_refused_with_a_stable_code() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            post(
                format!("/api/v2/outputs/{}/layers/0/update", bench.output),
                r#"{"dimmer":4.0}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "dimmer-out-of-range");
    }

    #[tokio::test]
    async fn a_reset_is_a_payload_free_get_that_must_not_be_cached() {
        let bench = bench();
        let response = bench
            .router
            .clone()
            .oneshot(get(format!(
                "/api/v2/outputs/{}/layers/0/reset",
                bench.output
            )))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert_eq!(
            bench.state.load().output(bench.output).unwrap().layers[0].reset_trigger_id,
            1
        );
    }

    #[tokio::test]
    async fn a_live_desk_makes_the_web_ui_read_only_with_a_reason() {
        let bench = bench();
        // A desk starts sending.
        let mut next = MediaState::clone(&bench.state.load());
        next.outputs[0]
            .ownership
            .observe_dmx(CommandSource::ArtNet, Timestamp::from_millis(0));
        bench.state.store(Arc::new(next));

        let (status, body) = send(
            &bench.router,
            post(
                format!("/api/v2/outputs/{}/layers/0/update", bench.output),
                r#"{"dimmer":0.5}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["code"], "dmx-owns-this");

        // Reading still works, and reports why.
        let (status, body) = send(
            &bench.router,
            get(format!("/api/v2/outputs/{}/state", bench.output)),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["dmxActive"], true);
    }

    #[tokio::test]
    async fn a_reset_still_works_while_a_desk_is_driving() {
        let bench = bench();
        let mut next = MediaState::clone(&bench.state.load());
        next.outputs[0]
            .ownership
            .observe_dmx(CommandSource::ArtNet, Timestamp::from_millis(0));
        bench.state.store(Arc::new(next));

        let response = bench
            .router
            .clone()
            .oneshot(get(format!(
                "/api/v2/outputs/{}/layers/0/reset",
                bench.output
            )))
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::NO_CONTENT,
            "an administrative action stays available"
        );
    }

    #[tokio::test]
    async fn unknown_outputs_and_layers_are_reported_distinctly() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            get(format!("/api/v2/outputs/{}/state", OutputId::new())),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "unknown-output");

        let (status, body) = send(
            &bench.router,
            post(
                format!("/api/v2/outputs/{}/layers/9/update", bench.output),
                r#"{"dimmer":0.5}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "unknown-layer");
    }

    #[tokio::test]
    async fn a_malformed_output_id_is_a_bad_request_not_a_not_found() {
        let bench = bench();
        let (status, body) =
            send(&bench.router, get("/api/v2/outputs/nonsense/state".into())).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "malformed-output-id");
    }

    #[tokio::test]
    async fn an_edited_visualizer_is_stored_before_it_is_answered() {
        let bench = bench();
        let uri = "/api/v2/visualizers/220/1/update".to_owned();
        let (status, body) = send(
            &bench.router,
            post(
                uri,
                r#"{"requestId":"a","name":"House bars","parameters":{"count":64,"size":0.1,"speed":1.0,"amount":1.0,"radius":0.3,"thickness":0.01,"reactivity":1.0,"decay":0.1,"zoom":1.0,"iterations":64,"threshold":0.5,"smoothing":0.5,"gravity":0.5,"lifetime":2.0,"curvature":0.2,"primaryRed":1.0,"primaryGreen":0.0,"primaryBlue":0.0,"secondaryRed":0.0,"secondaryGreen":0.0,"secondaryBlue":1.0,"mirror":true,"filled":false,"wireframe":false,"mode":0}}"#,
            ),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["name"], "House bars");
        assert_eq!(body["parameters"]["count"], 64);
        assert_eq!(body["parameters"]["mirror"], true);

        let stored = bench.stored.lock().unwrap();
        assert_eq!(stored.len(), 1, "the edit was written, not just answered");
        let saved = stored[0]
            .visualizers
            .resolve(media_domain::MediaAddress::new(220, 1))
            .expect("still there");
        assert_eq!(saved.name, "House bars");
        assert_eq!(saved.parameters.count, 64);
    }

    #[tokio::test]
    async fn resending_an_edit_answers_it_rather_than_doing_it_twice() {
        let bench = bench();
        let uri = "/api/v2/visualizers/220/1/update".to_owned();
        let edit = r#"{"requestId":"same","name":"First"}"#;

        let (_, first) = send(&bench.router, post(uri.clone(), edit)).await;
        let (status, second) = send(&bench.router, post(uri, edit)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            first, second,
            "a retry gets the outcome of the first attempt"
        );
        assert_eq!(
            bench.stored.lock().unwrap().len(),
            1,
            "and the edit was executed once"
        );
    }

    #[tokio::test]
    async fn an_edit_that_could_not_be_stored_is_not_applied() {
        let bench = bench();
        bench.refuse.store(true, Ordering::SeqCst);
        let uri = "/api/v2/visualizers/220/1/update".to_owned();

        let (status, body) = send(
            &bench.router,
            post(uri, r#"{"requestId":"b","name":"Never"}"#),
        )
        .await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body["code"], "configuration-not-written");

        let (_, published) = send(&bench.router, get("/api/v2/visualizers".into())).await;
        assert_eq!(
            published[0]["name"], "Equalizer Bars",
            "a change that was not saved must not be live either"
        );
    }

    #[tokio::test]
    async fn an_edit_without_a_request_id_or_for_an_unknown_address_is_refused() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            post(
                "/api/v2/visualizers/220/1/update".into(),
                r#"{"requestId":"  ","name":"No"}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "missing-request-id");

        let (status, body) = send(
            &bench.router,
            post(
                "/api/v2/visualizers/1/1/update".into(),
                r#"{"requestId":"c","name":"No"}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "unknown-visualizer");
        assert!(bench.stored.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_visualizer_cannot_be_left_nameless() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            post(
                "/api/v2/visualizers/220/1/update".into(),
                r#"{"requestId":"d","name":"   "}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "empty-name");
    }

    #[tokio::test]
    async fn every_shipped_visualizer_is_published_with_its_address_and_its_controls() {
        let bench = bench();
        let (status, body) = send(&bench.router, get("/api/v2/visualizers".into())).await;

        assert_eq!(status, StatusCode::OK);
        let entries = body.as_array().expect("a list");
        assert_eq!(entries.len(), 20);

        let first = &entries[0];
        assert_eq!(first["address"]["folder"], 220);
        assert_eq!(first["address"]["file"], 1);
        assert_eq!(first["address"]["class"], "generated-visualizer");
        assert_eq!(first["typeId"], 0);
        assert_eq!(first["kind"], "Equalizer Bars");
        assert!(
            first["uses"]
                .as_array()
                .expect("a list")
                .contains(&serde_json::json!("count")),
            "an editor is told which controls do something"
        );
        assert_eq!(first["parameters"]["count"], 32);
    }

    #[tokio::test]
    async fn a_layer_reports_its_mask_even_when_the_mask_is_doing_nothing() {
        let bench = bench();
        let (_, body) = send(
            &bench.router,
            get(format!("/api/v2/outputs/{}/state", bench.output)),
        )
        .await;

        let mask = &body["layers"][0]["mask"];
        assert_eq!(mask["address"]["class"], "blank");
        assert_eq!(mask["opacity"], 0.0);
        assert_eq!(mask["source"], "luminance");
        assert_eq!(
            mask["active"], false,
            "selected-but-faded and not-selected must be tellable apart"
        );
    }

    #[tokio::test]
    async fn the_catalog_is_served_as_the_same_snapshot_everything_else_reads() {
        let bench = bench();
        let (status, body) = send(&bench.router, get("/api/v2/catalog".into())).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["revision"], 0);
        assert_eq!(body["itemCount"], 0);
        assert!(body["folders"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn selecting_a_blank_address_is_allowed_because_it_clears_a_layer() {
        let bench = bench();
        let uri = format!("/api/v2/outputs/{}/layers/0/update", bench.output);
        send(&bench.router, post(uri.clone(), r#"{"folder":1,"file":1}"#)).await;

        let (status, body) = send(&bench.router, post(uri, r#"{"file":0}"#)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["layers"][0]["address"]["file"], 0);
        assert_eq!(
            bench.state.load().output(bench.output).unwrap().layers[0].address,
            MediaAddress::new(1, 0)
        );
    }
}
