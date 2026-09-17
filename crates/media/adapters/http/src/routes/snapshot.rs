//! Render-on-request snapshots of a programmed state.
//!
//! A desk previewing a stored Cue needs the picture that Cue *would* produce, not the picture the
//! output shows now. The desk sends the DMX slots it would transmit for the output's personality;
//! the renderer decodes them exactly as it decodes Art-Net or sACN, composites them off-screen
//! without touching the live output, and answers with a PNG:
//!
//! - no `layer`: the composited Program image of the whole output, master included;
//! - a `layer`: that one layer's composited image with its transparency kept.
//!
//! Snapshots are derived, bounded, and cached by content: the same slots, scope, size, and
//! library revision answer from memory instead of rendering again.

use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, PoisonError};

use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::Response;
use media_domain::OutputId;
use serde::Deserialize;

use super::ApiState;
use crate::error::ApiError;
use crate::tolerant::TolerantJson;

/// Largest picture a snapshot may be asked for. A Cue preview is a thumbnail, not a program feed.
pub const MAX_SNAPSHOT_EDGE: u16 = 1_024;
/// Pictures remembered by content. Each entry is a small PNG, so this bounds memory to a few MB.
const CACHE_ENTRIES: usize = 64;

/// What the renderer is asked to draw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotRequest {
    pub output: OutputId,
    /// `None` for the whole Program output, otherwise the zero-based layer index.
    pub layer: Option<usize>,
    /// The output's complete personality footprint, layers followed by the master.
    pub slots: Vec<u8>,
    pub width: u16,
    pub height: u16,
}

/// A rendered snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotImage {
    pub png: Arc<Vec<u8>>,
    pub width: u16,
    pub height: u16,
    /// Nothing in scope draws: no content is selected, or it is faded to zero. The picture is
    /// still valid (black, or fully transparent for a layer), but a desk shows an explicit "empty"
    /// state rather than an unexplained black tile.
    pub empty: bool,
}

/// Why a snapshot could not be drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotFailure {
    /// The slots do not match the output's personality or the layer does not exist.
    Invalid(String),
    /// This process cannot render right now (no graphics device, or no renderer at all).
    Unavailable(String),
    /// The selected sources did not finish loading within the snapshot's time budget.
    NotReady(String),
}

pub type RenderSnapshot = Arc<
    dyn Fn(
            SnapshotRequest,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<SnapshotImage, SnapshotFailure>> + Send>,
        > + Send
        + Sync,
>;

/// A renderer for a process that presents nothing.
pub fn renders_nothing() -> RenderSnapshot {
    Arc::new(|_| {
        Box::pin(std::future::ready(Err(SnapshotFailure::Unavailable(
            "this Media Server process has no renderer".to_owned(),
        ))))
    })
}

/// Remembered snapshots, keyed by everything that shapes the picture.
#[derive(Default)]
pub struct SnapshotCache {
    inner: Mutex<CacheInner>,
}

#[derive(Default)]
struct CacheInner {
    entries: HashMap<u64, SnapshotImage>,
    order: VecDeque<u64>,
}

impl SnapshotCache {
    fn get(&self, key: u64) -> Option<SnapshotImage> {
        let inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        inner.entries.get(&key).cloned()
    }

    fn put(&self, key: u64, image: SnapshotImage) {
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        if inner.entries.insert(key, image).is_none() {
            inner.order.push_back(key);
        }
        while inner.entries.len() > CACHE_ENTRIES {
            let Some(oldest) = inner.order.pop_front() else {
                break;
            };
            inner.entries.remove(&oldest);
        }
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entries
            .len()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SnapshotBody {
    /// The personality's slots, one number 0-255 per slot.
    slots: Vec<u16>,
    #[serde(default)]
    layer: Option<usize>,
    #[serde(default)]
    width: Option<u16>,
    #[serde(default)]
    height: Option<u16>,
}

/// `POST /api/v2/outputs/{output}/snapshot`
pub(super) async fn output_snapshot(
    State(state): State<ApiState>,
    Path(output): Path<String>,
    TolerantJson(body): TolerantJson<SnapshotBody>,
) -> Result<Response, ApiError> {
    let id = uuid::Uuid::parse_str(&output)
        .map(OutputId::from_uuid)
        .map_err(|_| ApiError::bad_request("malformed-output-id", "that is not an output id"))?;
    let media = state.state.load();
    let found = media
        .output(id)
        .ok_or_else(|| ApiError::not_found("unknown-output", format!("no output {id}")))?;
    let required = usize::from(found.personality.footprint().total());
    if body.slots.len() != required {
        return Err(ApiError::bad_request(
            "snapshot-slots-mismatch",
            format!(
                "this output's personality needs exactly {required} slots, not {}",
                body.slots.len()
            ),
        ));
    }
    let slots = body
        .slots
        .iter()
        .map(|slot| u8::try_from(*slot))
        .collect::<Result<Vec<u8>, _>>()
        .map_err(|_| ApiError::bad_request("snapshot-slot-range", "every slot must be 0-255"))?;
    if let Some(layer) = body.layer
        && found.layer(layer).is_none()
    {
        return Err(ApiError::not_found(
            "layer-not-found",
            format!("output {id} has no layer {}", layer + 1),
        ));
    }
    let width = body.width.unwrap_or(320);
    let height = body.height.unwrap_or(180);
    if !(1..=MAX_SNAPSHOT_EDGE).contains(&width) || !(1..=MAX_SNAPSHOT_EDGE).contains(&height) {
        return Err(ApiError::bad_request(
            "snapshot-size",
            format!("width and height must be 1-{MAX_SNAPSHOT_EDGE} pixels"),
        ));
    }
    let request = SnapshotRequest {
        output: id,
        layer: body.layer,
        slots,
        width,
        height,
    };
    let key = cache_key(&request, &state);
    let (image, cached) = match state.snapshots.get(key) {
        Some(image) => (image, true),
        None => {
            let image = (state.snapshot)(request).await.map_err(failure)?;
            state.snapshots.put(key, image.clone());
            (image, false)
        }
    };
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/png")
        .header(header::CACHE_CONTROL, "no-store")
        .header(header::ETAG, format!("\"{key:016x}\""))
        .header(
            "x-tosklight-snapshot-content",
            if image.empty { "empty" } else { "content" },
        )
        .header(
            "x-tosklight-snapshot-cache",
            if cached { "hit" } else { "miss" },
        )
        .header("x-tosklight-preview-width", image.width)
        .header("x-tosklight-preview-height", image.height)
        .body(axum::body::Body::from(image.png.as_ref().clone()))
        .expect("a snapshot response has valid static headers"))
}

/// Everything that shapes the picture: the request, the library it resolves against, and the
/// output configuration (resolution, effects, visualizers) it is drawn with.
fn cache_key(request: &SnapshotRequest, state: &ApiState) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    request.output.to_string().hash(&mut hasher);
    request.layer.hash(&mut hasher);
    request.slots.hash(&mut hasher);
    request.width.hash(&mut hasher);
    request.height.hash(&mut hasher);
    format!("{:?}", state.catalog.load().revision).hash(&mut hasher);
    // Effects, visualizers, text, models, and the output's own resolution all live in the
    // configuration document; any accepted edit changes the picture a snapshot may draw.
    serde_json::to_string(state.configuration.load().as_ref())
        .unwrap_or_default()
        .hash(&mut hasher);
    hasher.finish()
}

fn failure(failure: SnapshotFailure) -> ApiError {
    match failure {
        SnapshotFailure::Invalid(message) => ApiError::bad_request("snapshot-invalid", message),
        SnapshotFailure::Unavailable(message) => ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "snapshot-unavailable",
            message,
        ),
        SnapshotFailure::NotReady(message) => ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "snapshot-not-ready",
            message,
        ),
    }
}

#[cfg(test)]
#[path = "snapshot_tests.rs"]
mod tests;
