//! v2 Cue previews drawn by the addressed Media Server.
//!
//! A Cue that contains only Media Server content is pictured by the server that plays it: the
//! desk folds the Cuelist to the Cue, renders the DMX slots that Cue leaves on the server fixture,
//! and asks the ToskLight Media Server to composite exactly those slots off-screen. A Cue that
//! addresses the master (or several layers) is pictured as the output's Program image; a Cue that
//! addresses one layer is pictured as that layer alone, transparency kept.
//!
//! The index is cheap and carries no pixels; the picture is fetched per Cue. Every entry names
//! the server fixture, output, and layer, and the preview key covers all of them, so a desk never
//! shows one server's image for another server's Cue. Pictures are not stored in the show: they
//! depend on the server's own library, which does not travel with the show file.

use super::media_api::{TOSKLIGHT_MEDIA_HTTP_PORT, native_media_action, native_media_bound_output};
use super::*;
use axum::body::Body;
use light_wire::v2::cue_media_previews as wire;

const DEFAULT_WIDTH: u16 = 320;
const DEFAULT_HEIGHT: u16 = 180;
const MAX_EDGE: u16 = 1_024;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v2/cues/media-previews", get(media_preview_index))
        .route(
            "/api/v2/cues/{cue_id}/media-preview",
            get(cue_media_preview),
        )
}

/// Which Media Server picture a Cue's preview is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct MediaPreviewTarget {
    pub(super) server: light_core::FixtureId,
    /// Zero-based layer and its logical-head fixture, or `None` for the Program image.
    pub(super) layer: Option<(u16, light_core::FixtureId)>,
}

/// Classifies a Cue by the fixtures it addresses itself.
///
/// Only a Cue whose every addressed fixture belongs to one ToskLight Media Server is a media Cue.
/// Exactly one layer (and no master) is a layer preview; the master, or more than one layer, is
/// the Program preview.
pub(super) fn media_preview_target(
    fixtures: &[light_fixture::PatchedFixture],
    addressed: &[light_core::FixtureId],
) -> Option<MediaPreviewTarget> {
    if addressed.is_empty() {
        return None;
    }
    let mut server = None;
    let mut master = false;
    let mut layers = Vec::new();
    for fixture_id in addressed {
        let (owner, layer) = fixtures
            .iter()
            .filter(|fixture| {
                is_media_server_fixture(fixture) && native_media_action(fixture).is_some()
            })
            .find_map(|fixture| {
                if fixture.fixture_id == *fixture_id {
                    return Some((fixture.fixture_id, None));
                }
                let head = fixture
                    .logical_heads
                    .iter()
                    .find(|head| head.fixture_id == *fixture_id)?;
                Some((
                    fixture.fixture_id,
                    Some((layer_index(fixture, head.head_index)?, head.fixture_id)),
                ))
            })?;
        if *server.get_or_insert(owner) != owner {
            return None;
        }
        match layer {
            None => master = true,
            Some(layer) => {
                if !layers.contains(&layer) {
                    layers.push(layer);
                }
            }
        }
    }
    Some(MediaPreviewTarget {
        server: server?,
        layer: (!master && layers.len() == 1).then(|| layers[0]),
    })
}

/// A layer head's zero-based position among the fixture's layer heads.
fn layer_index(fixture: &light_fixture::PatchedFixture, head_index: u16) -> Option<u16> {
    let mut layer_heads = fixture
        .definition
        .heads
        .iter()
        .filter(|head| !head.shared)
        .map(|head| head.index)
        .collect::<Vec<_>>();
    layer_heads.sort_unstable();
    layer_heads
        .iter()
        .position(|index| *index == head_index)
        .and_then(|position| u16::try_from(position).ok())
}

/// Everything needed to ask a Media Server for one Cue's picture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PreparedMediaPreview {
    pub(super) entry: wire::CueMediaPreviewEntry,
    pub(super) slots: Vec<u8>,
    pub(super) endpoint: Option<std::net::IpAddr>,
}

pub(super) fn prepare_media_previews(
    snapshot: &EngineSnapshot,
    only: Option<Uuid>,
) -> Vec<PreparedMediaPreview> {
    let mut prepared = Vec::new();
    for cue_list in snapshot.cue_lists.iter() {
        for cue in &cue_list.cues {
            if only.is_some_and(|only| only != cue.id) {
                continue;
            }
            if let Some(preview) = prepare_one(snapshot, cue_list.id, cue.id) {
                prepared.push(preview);
            }
        }
    }
    prepared
}

fn prepare_one(
    snapshot: &EngineSnapshot,
    cue_list_id: light_core::CueListId,
    cue_id: Uuid,
) -> Option<PreparedMediaPreview> {
    let state = light_engine::cue_preview_state(snapshot, cue_list_id, cue_id)?;
    let target = media_preview_target(&snapshot.fixtures, &state.addressed)?;
    let server = snapshot
        .fixtures
        .iter()
        .find(|fixture| fixture.fixture_id == target.server)?;
    let slots = light_engine::render_fixture_slots(server, &state.tracked).ok()?;
    let output_id = server
        .internal_bindings
        .output
        .as_deref()
        .map(str::trim)
        .filter(|output| !output.is_empty() && *output != "default")
        .map(str::to_owned);
    let endpoint = server
        .direct_control
        .as_ref()
        .filter(|endpoint| endpoint.protocol == light_fixture::DirectControlProtocol::Citp)
        .map(|endpoint| endpoint.ip_address);
    let mut digest = Sha256::new();
    digest.update(target.server.0.as_bytes());
    digest.update(format!("{endpoint:?}|{output_id:?}|{:?}|", target.layer).as_bytes());
    digest.update(&slots);
    let preview_key = digest
        .finalize()
        .iter()
        .take(16)
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Some(PreparedMediaPreview {
        entry: wire::CueMediaPreviewEntry {
            cue_id,
            cue_list_id: cue_list_id.0,
            server_fixture_id: target.server.0,
            output_id,
            scope: if target.layer.is_some() {
                wire::CueMediaPreviewScope::Layer
            } else {
                wire::CueMediaPreviewScope::Program
            },
            layer: target.layer.map(|(layer, _)| layer),
            layer_fixture_id: target.layer.map(|(_, fixture)| fixture.0),
            preview_key,
        },
        slots,
        endpoint,
    })
}

/// Lists every Cue a Media Server pictures, without pixels.
async fn media_preview_index(
    State(state): State<AppState>,
    context: ShowContext,
    headers: HeaderMap,
) -> Result<Json<wire::CueMediaPreviewIndex>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let show_id = context.resolve(&state)?;
    let snapshot = state.output.snapshot();
    let entries = tokio::task::spawn_blocking(move || prepare_media_previews(&snapshot, None))
        .await
        .map_err(|_| ApiError::internal("could not prepare Cue media previews"))?
        .into_iter()
        .map(|prepared| prepared.entry)
        .collect();
    Ok(Json(wire::CueMediaPreviewIndex {
        show_id: show_id.0,
        entries,
    }))
}

#[derive(Default, Deserialize)]
pub(super) struct MediaPreviewQuery {
    width: Option<u16>,
    height: Option<u16>,
}

/// A picture, or the operator-facing reason there is none.
#[derive(Debug)]
pub(super) enum SnapshotOutcome {
    Picture { png: Vec<u8>, empty: bool },
    Failed(wire::CueMediaPreviewFailure),
}

fn failure(
    state: wire::CueMediaPreviewFailureState,
    error: impl Into<String>,
    retryable: bool,
) -> SnapshotOutcome {
    SnapshotOutcome::Failed(wire::CueMediaPreviewFailure {
        state,
        error: error.into(),
        retryable,
    })
}

/// Serves one Cue's Media Server picture.
async fn cue_media_preview(
    State(state): State<AppState>,
    Path(cue_id): Path<Uuid>,
    Query(query): Query<MediaPreviewQuery>,
    context: ShowContext,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let _session = authenticate(&state, &headers)?;
    context.resolve(&state)?;
    let width = query.width.unwrap_or(DEFAULT_WIDTH).clamp(1, MAX_EDGE);
    let height = query.height.unwrap_or(DEFAULT_HEIGHT).clamp(1, MAX_EDGE);
    let snapshot = state.output.snapshot();
    let prepared =
        tokio::task::spawn_blocking(move || prepare_media_previews(&snapshot, Some(cue_id)))
            .await
            .map_err(|_| ApiError::internal("could not prepare the Cue media preview"))?
            .into_iter()
            .next();
    let Some(prepared) = prepared else {
        return Ok(failure_response(wire::CueMediaPreviewFailure {
            state: wire::CueMediaPreviewFailureState::Missing,
            error: "This Cue has no Media Server preview.".into(),
            retryable: false,
        }));
    };
    let server = light_core::FixtureId(prepared.entry.server_fixture_id);
    let Some(ip) = prepared.endpoint else {
        return Ok(failure_response(wire::CueMediaPreviewFailure {
            state: wire::CueMediaPreviewFailureState::Missing,
            error:
                "The Media Server fixture has no network endpoint. Patch it to a discovered server."
                    .into(),
            retryable: false,
        }));
    };
    let base = format!(
        "http://{}/api/v2",
        std::net::SocketAddr::new(ip, TOSKLIGHT_MEDIA_HTTP_PORT)
    );
    let bound = native_media_bound_output(&state, server)?;
    let outcome = fetch_snapshot(
        &base,
        bound.as_deref(),
        prepared.entry.layer,
        &prepared.slots,
        width,
        height,
    )
    .await;
    note_reachability(&state, server, &outcome);
    Ok(match outcome {
        SnapshotOutcome::Picture { png, empty } => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "image/png")
            .header(header::CACHE_CONTROL, "no-store")
            .header(header::ETAG, format!("\"{}\"", prepared.entry.preview_key))
            .header(
                "x-light-media-preview",
                if empty { "empty" } else { "content" },
            )
            .header("x-light-media-preview-key", prepared.entry.preview_key)
            .body(Body::from(png))
            .map_err(|_| ApiError::internal("could not return the Cue media preview"))?,
        SnapshotOutcome::Failed(failed) => failure_response(failed),
    })
}

fn failure_response(failed: wire::CueMediaPreviewFailure) -> Response {
    let status = match failed.state {
        wire::CueMediaPreviewFailureState::Missing => StatusCode::NOT_FOUND,
        wire::CueMediaPreviewFailureState::Offline | wire::CueMediaPreviewFailureState::Loading => {
            StatusCode::SERVICE_UNAVAILABLE
        }
    };
    let mut response = (status, Json(failed)).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    response
}

/// Keeps the patched-server status true and pushes a change, so every surface showing the server
/// (and every Cue preview waiting for it) learns it went offline or came back.
fn note_reachability(state: &AppState, server: light_core::FixtureId, outcome: &SnapshotOutcome) {
    let offline = matches!(
        outcome,
        SnapshotOutcome::Failed(wire::CueMediaPreviewFailure {
            state: wire::CueMediaPreviewFailureState::Offline,
            ..
        })
    );
    let before = state.media.status(server);
    match outcome {
        SnapshotOutcome::Failed(failed) if offline => {
            state
                .media
                .record_status(server, Some(failed.error.clone()));
            if before.online || before.last_error.is_none() {
                emit(
                    state,
                    "media_server_offline",
                    serde_json::json!({"fixture_id":server,"error":failed.error}),
                );
            }
        }
        SnapshotOutcome::Failed(_) => {}
        SnapshotOutcome::Picture { .. } => {
            state.media.record_status(server, None);
            if !before.online {
                emit(
                    state,
                    "media_inspected",
                    serde_json::json!({"fixture_id":server}),
                );
            }
        }
    }
}

#[derive(Deserialize)]
struct NativeOutput {
    id: String,
}

#[derive(Deserialize)]
struct NativeError {
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    message: String,
}

/// Asks a ToskLight Media Server at `base` (`http://host:port/api/v2`) to render `slots`.
pub(super) async fn fetch_snapshot(
    base: &str,
    bound_output: Option<&str>,
    layer: Option<u16>,
    slots: &[u8],
    width: u16,
    height: u16,
) -> SnapshotOutcome {
    use wire::CueMediaPreviewFailureState as State;
    let client = match reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(10))
        .build()
    {
        Ok(client) => client,
        Err(_) => {
            return failure(
                State::Offline,
                "The Media Server client is unavailable.",
                true,
            );
        }
    };
    let output = match bound_output {
        Some(output) => output.to_owned(),
        None => {
            let outputs = match client.get(format!("{base}/outputs")).send().await {
                Ok(response) if response.status().is_success() => {
                    response.json::<Vec<NativeOutput>>().await.ok()
                }
                Ok(_) => None,
                Err(_) => {
                    return failure(State::Offline, "The Media Server did not answer.", true);
                }
            };
            match outputs.and_then(|outputs| outputs.into_iter().next()) {
                Some(output) => output.id,
                None => {
                    return failure(
                        State::Missing,
                        "The Media Server has no output to draw.",
                        false,
                    );
                }
            }
        }
    };
    let response = match client
        .post(format!("{base}/outputs/{output}/snapshot"))
        .json(&serde_json::json!({
            "slots": slots,
            "layer": layer,
            "width": width,
            "height": height,
        }))
        .send()
        .await
    {
        Ok(response) => response,
        Err(_) => return failure(State::Offline, "The Media Server did not answer.", true),
    };
    let status = response.status();
    if status.is_success() {
        let empty = response
            .headers()
            .get("x-tosklight-snapshot-content")
            .is_some_and(|value| value == "empty");
        return match response.bytes().await {
            Ok(png) => SnapshotOutcome::Picture {
                png: png.to_vec(),
                empty,
            },
            Err(_) => failure(State::Offline, "The Media Server stopped answering.", true),
        };
    }
    let refused = response.json::<NativeError>().await.ok();
    let code = refused.as_ref().and_then(|error| error.code.as_deref());
    let detail = refused
        .as_ref()
        .map(|error| error.message.clone())
        .unwrap_or_default();
    match (status, code) {
        (_, Some("snapshot-not-ready")) => failure(
            State::Loading,
            "The Media Server is still loading this media.",
            true,
        ),
        (_, Some("unknown-output" | "malformed-output-id")) => failure(
            State::Missing,
            "The Media Server no longer has the patched output. Refresh discovery and patch the output again.",
            false,
        ),
        (_, Some("snapshot-slots-mismatch")) => failure(
            State::Missing,
            "The patched mode does not match the Media Server output's personality. Patch the output again.",
            false,
        ),
        (reqwest::StatusCode::NOT_FOUND, None) => failure(
            State::Missing,
            "This Media Server cannot draw Cue previews. Update ToskLight Media to the current version.",
            false,
        ),
        (_, Some("snapshot-unavailable")) => failure(
            State::Missing,
            format!("The Media Server cannot draw Cue previews: {detail}"),
            false,
        ),
        _ => failure(
            State::Missing,
            format!("The Media Server refused the Cue preview ({status}). {detail}")
                .trim()
                .to_owned(),
            false,
        ),
    }
}

#[cfg(test)]
#[path = "cue_media_previews_http_tests.rs"]
pub(super) mod tests;
