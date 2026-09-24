//! The generated visualizers, and tuning one.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use media_application::MediaConfiguration;
use media_domain::MediaAddress;
use media_domain::visualizer::{VisualizerConfiguration, VisualizerKind};

use crate::error::ApiError;
use crate::routes::edit::{self, Proceed};
use crate::routes::snapshot::{MAX_SNAPSHOT_EDGE, SnapshotFailure};
use crate::routes::{ApiState, OutputPreviewFrame};
use crate::tolerant::TolerantJson;
use crate::wire::{CreateVisualizer, UpdateVisualizer, VisualizerView};

/// Which generated visualizer answers at which address.
///
/// Configuration rather than state: it changes when an operator reassigns an address, not from
/// frame to frame, so it is read once and not polled.
pub(super) async fn visualizers(State(state): State<ApiState>) -> impl IntoResponse {
    axum::Json(VisualizerView::all(&state.configuration.load().visualizers))
}

/// Draws one frame of the visualizer at an address, as its stored parameters make it look now.
///
/// The renderer keeps each previewed visualizer's memory of the beat between frames, so a client
/// asking for frame after frame sees it move with the room, and an edit shows on the next frame.
pub type RenderVisualizerPreview = Arc<
    dyn Fn(
            MediaAddress,
            u16,
            u16,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<Output = Result<OutputPreviewFrame, SnapshotFailure>>
                    + Send,
            >,
        > + Send
        + Sync,
>;

/// A process that renders nothing previews no visualizer.
pub fn previews_no_visualizer() -> RenderVisualizerPreview {
    Arc::new(|_, _, _| {
        Box::pin(std::future::ready(Err(SnapshotFailure::Unavailable(
            "this Media Server process has no renderer".to_owned(),
        ))))
    })
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PreviewQuery {
    width: Option<u16>,
    height: Option<u16>,
}

/// `GET /api/v2/visualizers/{folder}/{file}/preview`: one live frame, never cached.
///
/// A read, not a live-control action: it draws off-screen and changes no output. A client polls
/// it for as long as an operator is looking at the visualizer's editor.
pub(super) async fn visualizer_preview(
    State(state): State<ApiState>,
    Path((folder, file)): Path<(u8, u8)>,
    Query(query): Query<PreviewQuery>,
) -> Result<Response, ApiError> {
    let address = MediaAddress::new(folder, file);
    if state
        .configuration
        .load()
        .visualizers
        .resolve(address)
        .is_none()
    {
        return Err(ApiError::not_found(
            "unknown-visualizer",
            format!("no visualizer answers at {address}"),
        ));
    }
    let width = query.width.unwrap_or(480);
    let height = query.height.unwrap_or(270);
    if !(1..=MAX_SNAPSHOT_EDGE).contains(&width) || !(1..=MAX_SNAPSHOT_EDGE).contains(&height) {
        return Err(ApiError::bad_request(
            "preview-size",
            format!("width and height must be 1-{MAX_SNAPSHOT_EDGE} pixels"),
        ));
    }
    let frame = (state.visualizer_preview)(address, width, height)
        .await
        .map_err(|failure| match failure {
            SnapshotFailure::Invalid(message) => ApiError::bad_request("preview-invalid", message),
            SnapshotFailure::Unavailable(message) | SnapshotFailure::NotReady(message) => {
                ApiError::new(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "preview-unavailable",
                    message,
                )
            }
        })?;
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CACHE_CONTROL, "no-store")
        .header(header::CONTENT_TYPE, frame.content_type)
        .header("x-tosklight-preview-sequence", frame.sequence)
        .header("x-tosklight-preview-width", frame.width)
        .header("x-tosklight-preview-height", frame.height)
        .body(axum::body::Body::from(frame.bytes))
        .expect("a preview response has valid static headers"))
}

/// Creates another instance of a shipped visualizer in the empty slot the operator selected.
pub(super) async fn create_visualizer(
    State(state): State<ApiState>,
    TolerantJson(body): TolerantJson<CreateVisualizer>,
) -> Result<Response, ApiError> {
    let _edit = match edit::begin(&state, &body.request_id).await? {
        Proceed::Replay(response) => return Ok(response),
        Proceed::Fresh(guard) => guard,
    };

    let kind = VisualizerKind::from_type_id(body.type_id).ok_or_else(|| {
        ApiError::bad_request(
            "unknown-visualizer-kind",
            "choose one of the visualizer kinds published by this Media Server",
        )
    })?;
    let mut visualizer = VisualizerConfiguration::new(kind);
    if let Some(name) = body.name {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(ApiError::bad_request(
                "empty-name",
                "a visualizer needs a name an operator can find it by",
            ));
        }
        visualizer.name = trimmed.to_owned();
    }

    let mut configuration = MediaConfiguration::clone(&state.configuration.load());
    let address = MediaAddress::new(body.folder, body.file);
    configuration
        .visualizers
        .assign(address, visualizer)
        .map_err(|error| ApiError::bad_request("visualizer-not-created", error.to_string()))?;

    let created = configuration
        .visualizers
        .resolve(address)
        .expect("the assigned visualizer is immediately addressable");
    let view = VisualizerView::of(address, created);
    edit::commit(&state, configuration, &body.request_id, &view)
}

/// Edits one configured visualizer.
///
/// An object-intent update: only the fields being changed travel, and the request id makes a
/// retry safe. Unlike a layer selection this is stored configuration, so it is written to disk
/// before it is answered — an operator who tuned a look and restarted must find it there.
pub(super) async fn update_visualizer(
    State(state): State<ApiState>,
    Path((folder, file)): Path<(u8, u8)>,
    TolerantJson(body): TolerantJson<UpdateVisualizer>,
) -> Result<Response, ApiError> {
    let _edit = match edit::begin(&state, &body.request_id).await? {
        Proceed::Replay(response) => return Ok(response),
        Proceed::Fresh(guard) => guard,
    };

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
    if let Some(type_id) = body.type_id {
        let kind = VisualizerKind::from_type_id(type_id).ok_or_else(|| {
            ApiError::bad_request(
                "unknown-visualizer-kind",
                "choose one of the visualizer kinds published by this Media Server",
            )
        })?;
        let name = entry.configuration.name.clone();
        entry.configuration = VisualizerConfiguration::new(kind);
        entry.configuration.name = name;
    }
    if let Some(parameters) = body.parameters {
        entry.configuration.parameters = parameters.into_parameters();
    }

    let view = VisualizerView::of(address, &entry.configuration);
    edit::commit(&state, configuration, &body.request_id, &view)
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use media_domain::visualizer::VisualizerKind;

    use crate::routes::bench::{bench, get, post, send};

    #[tokio::test]
    async fn every_shipped_visualizer_is_published_with_its_address_and_its_controls() {
        let bench = bench();
        let (status, body) = send(&bench.router, get("/api/v2/visualizers".into())).await;

        assert_eq!(status, StatusCode::OK);
        let entries = body.as_array().expect("a list");
        assert_eq!(entries.len(), 25);

        let first = &entries[0];
        assert_eq!(first["address"]["folder"], 250);
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

        let city = entries
            .iter()
            .find(|entry| entry["typeId"] == 52)
            .expect("City Tunnel is a shipped built-in");
        assert_eq!(city["kind"], "City Tunnel");
        assert_eq!(city["address"]["folder"], 250);
        assert!(
            city["uses"]
                .as_array()
                .expect("a list")
                .contains(&serde_json::json!("speed")),
            "the operator can tune tunnel travel speed"
        );

        let landscape = entries
            .iter()
            .find(|entry| entry["typeId"] == 53)
            .expect("Grid Landscape is a shipped built-in");
        assert_eq!(landscape["kind"], "Grid Landscape");
        for control in ["speed", "radius", "mode", "iterations"] {
            assert!(
                landscape["uses"]
                    .as_array()
                    .expect("a list")
                    .contains(&serde_json::json!(control)),
                "Grid Landscape publishes {control}"
            );
        }
    }

    #[tokio::test]
    async fn another_instance_of_a_built_in_kind_uses_the_selected_empty_address() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            post(
                "/api/v2/visualizers/create".into(),
                r#"{"requestId":"new-bars","folder":251,"file":7,"typeId":0,"name":"Thin bars"}"#,
            ),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["typeId"], 0);
        assert_eq!(body["name"], "Thin bars");
        assert_eq!(body["address"]["folder"], 251);
        assert_eq!(body["address"]["file"], 7);

        let stored = bench.stored.lock().unwrap();
        assert_eq!(stored.len(), 1);
        let equalizers = stored[0]
            .visualizers
            .entries
            .iter()
            .filter(|entry| entry.configuration.kind == VisualizerKind::EqualizerBars)
            .count();
        assert_eq!(equalizers, 2);
    }

    #[tokio::test]
    async fn an_occupied_visualizer_address_is_not_reassigned() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            post(
                "/api/v2/visualizers/create".into(),
                r#"{"requestId":"occupied","folder":250,"file":1,"typeId":0}"#,
            ),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "visualizer-not-created");
        assert!(bench.stored.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn changing_the_built_in_kind_keeps_the_address_and_resets_safe_defaults() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            post(
                "/api/v2/visualizers/250/1/update".into(),
                r#"{"requestId":"change-kind","typeId":22}"#,
            ),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["address"]["folder"], 250);
        assert_eq!(body["address"]["file"], 1);
        assert_eq!(body["typeId"], 22);
        assert_eq!(body["kind"], "Starfield");
        assert_eq!(body["name"], "Equalizer Bars");
        assert!(
            body["uses"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!("speed"))
        );
    }

    #[tokio::test]
    async fn a_visualizer_preview_is_a_fresh_frame_of_that_visualizer() {
        use tower::ServiceExt;
        let bench = bench();
        let response = bench
            .router
            .clone()
            .oneshot(get(
                "/api/v2/visualizers/250/1/preview?width=320&height=180".into(),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["content-type"], "image/jpeg");
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert_eq!(response.headers()["x-tosklight-preview-width"], "320");
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(&body[..], b"frame of 250/001");
    }

    #[tokio::test]
    async fn an_unassigned_or_oversized_preview_is_refused() {
        let bench = bench();
        let (status, _) = send(
            &bench.router,
            get("/api/v2/visualizers/250/200/preview".into()),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, _) = send(
            &bench.router,
            get("/api/v2/visualizers/250/1/preview?width=5000&height=10".into()),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn an_edited_visualizer_is_stored_before_it_is_answered() {
        let bench = bench();
        let uri = "/api/v2/visualizers/250/1/update".to_owned();
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
            .resolve(media_domain::MediaAddress::new(250, 1))
            .expect("still there");
        assert_eq!(saved.name, "House bars");
        assert_eq!(saved.parameters.count, 64);
    }

    #[tokio::test]
    async fn resending_an_edit_answers_it_rather_than_doing_it_twice() {
        let bench = bench();
        let uri = "/api/v2/visualizers/250/1/update".to_owned();
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
        bench
            .refuse
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let uri = "/api/v2/visualizers/250/1/update".to_owned();

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
        assert_eq!(
            bench.applied.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "and nothing running was told to honour it"
        );
    }

    #[tokio::test]
    async fn an_edit_without_a_request_id_or_for_an_unknown_address_is_refused() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            post(
                "/api/v2/visualizers/250/1/update".into(),
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
                "/api/v2/visualizers/250/1/update".into(),
                r#"{"requestId":"d","name":"   "}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "empty-name");
    }
}
