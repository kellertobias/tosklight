//! v2 Cue preview routes: store the picture of a Cue once, serve it to every desk that opens the
//! Cuelist.
//!
//! A desk draws a Cue preview from its own 3D stage when the operator records or edits the Cue,
//! then uploads it here. Every later desk — including one with no 3D renderer of its own — reads
//! the stored picture instead of redrawing the whole Cuelist on open.
//!
//! Previews are derived data. The server never redraws one and never fails an operator action
//! because a picture is missing or stale; a Cue with no stored preview simply has none yet.
//! Uploads carry a client `request_id` absorbed by a replay window (api-rules §3).

use super::show_objects_v2::{active_entry, validate_request_id};
use super::*;
use crate::tolerant_json::TolerantJson;
use axum::body::Body;
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use light_wire::v2::cue_thumbnails as wire;
use std::collections::VecDeque;

const REQUEST_CACHE_ENTRY_LIMIT: usize = 256;
const MAX_BATCH: usize = 512;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v2/cues/thumbnails", get(thumbnail_index))
        .route("/api/v2/cues/thumbnails/update", post(update_thumbnails))
        .route("/api/v2/cues/{cue_id}/thumbnail", get(cue_thumbnail))
}

/// Lists what the show already holds, so a desk can redraw only what it must.
async fn thumbnail_index(
    State(state): State<AppState>,
    context: ShowContext,
    headers: HeaderMap,
) -> Result<Json<wire::CueThumbnailIndex>, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let show_id = context.resolve(&state)?;
    let entry = active_entry(&state, show_id)?;
    let store = ActiveShowRepository::open(&entry.path).map_err(ApiError::store)?;
    let entries = store.cue_thumbnail_index().map_err(ApiError::store)?;
    Ok(Json(wire::CueThumbnailIndex {
        show_id: show_id.0,
        entries: entries
            .into_iter()
            .filter_map(|entry| {
                Some(wire::CueThumbnailEntry {
                    cue_id: Uuid::parse_str(&entry.cue_id).ok()?,
                    state_hash: entry.state_hash,
                    updated_at: entry
                        .updated_at
                        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                })
            })
            .collect(),
    }))
}

/// Serves one stored picture.
///
/// The `state_hash` is the validator: the desk appends it to the URL, so a redrawn preview is a
/// different URL and the browser cache never has to be told to forget the old one.
async fn cue_thumbnail(
    State(state): State<AppState>,
    Path(cue_id): Path<String>,
    context: ShowContext,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let _session = authenticate(&state, &headers)?;
    let show_id = context.resolve(&state)?;
    let entry = active_entry(&state, show_id)?;
    let store = ActiveShowRepository::open(&entry.path).map_err(ApiError::store)?;
    let stored = store
        .cue_thumbnail(&cue_id)
        .map_err(ApiError::store)?
        .ok_or_else(|| ApiError::not_found("cue preview"))?;
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/webp")
        .header(
            header::CACHE_CONTROL,
            "private, max-age=31536000, immutable",
        )
        .header(header::ETAG, format!("\"{}\"", stored.state_hash))
        .header(header::CONTENT_LENGTH, stored.image.len().to_string())
        .header("x-light-image-width", stored.width.to_string())
        .header("x-light-image-height", stored.height.to_string())
        .body(Body::from(stored.image))
        .map_err(|_| ApiError::internal("could not return cue preview"))
}

/// Stores a batch of redrawn previews.
///
/// Editing one Cue restages every Cue that tracks from it, so a desk sends the whole affected run
/// as one request rather than one request per Cue.
async fn update_thumbnails(
    State(state): State<AppState>,
    context: ShowContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<wire::CueThumbnailUpdateRequest>,
) -> Result<Json<wire::CueThumbnailUpdateOutcome>, ApiError> {
    let session = authenticate(&state, &headers)?;
    validate_request(&request)?;
    let show_id = context.resolve(&state)?;
    let key = ReplayKey {
        session_id: session.id.0,
        show_id: show_id.0,
        request_id: request.request_id.clone(),
    };
    let _activation = state.active_show.acquire().await;
    if let Some(outcome) = state
        .replay
        .lookup_cue_thumbnails(&key, &request.thumbnails)
        .await?
    {
        return Ok(Json(outcome));
    }
    let entry = active_entry(&state, show_id)?;
    let store = ActiveShowRepository::open(&entry.path).map_err(ApiError::store)?;
    let live = live_cue_ids(&store)?;

    let mut stored = Vec::new();
    let mut skipped_cue_ids = Vec::new();
    for upload in &request.thumbnails {
        let cue_id = upload.cue_id.to_string();
        // A Cue can be deleted while its picture is still in flight. Dropping the upload is the
        // correct outcome, not an error the operator should see.
        if !live.contains(&cue_id) {
            skipped_cue_ids.push(upload.cue_id);
            continue;
        }
        stored.push(light_show::CueThumbnail {
            cue_id,
            image: decode_image(&upload.image_base64)?,
            state_hash: upload.state_hash.clone(),
            width: upload.width,
            height: upload.height,
            updated_at: chrono::Utc::now(),
        });
    }
    let count = store.put_cue_thumbnails(&stored).map_err(ApiError::store)?;
    // Previews for Cues the show no longer holds would otherwise sit in the file forever, and the
    // file travels to other desks.
    store.prune_cue_thumbnails(&live).map_err(ApiError::store)?;

    let outcome = wire::CueThumbnailUpdateOutcome {
        request_id: request.request_id.clone(),
        correlation_id: Uuid::new_v4(),
        replayed: false,
        show_id: show_id.0,
        stored: u32::try_from(count).unwrap_or(u32::MAX),
        skipped_cue_ids,
    };
    state
        .replay
        .insert_cue_thumbnails(key, request.thumbnails, outcome.clone())
        .await;
    Ok(Json(outcome))
}

fn validate_request(request: &wire::CueThumbnailUpdateRequest) -> Result<(), ApiError> {
    validate_request_id(&request.request_id)?;
    if request.thumbnails.is_empty() {
        return Err(ApiError::bad_request("thumbnails must not be empty"));
    }
    if request.thumbnails.len() > MAX_BATCH {
        return Err(ApiError::bad_request(format!(
            "thumbnails must contain at most {MAX_BATCH} entries"
        )));
    }
    Ok(())
}

/// Decodes and sniffs the upload.
///
/// The server owns this check rather than trusting the desk: the bytes go into the show file and
/// travel to other desks, so only a real WebP picture is allowed in.
fn decode_image(encoded: &str) -> Result<Vec<u8>, ApiError> {
    let bytes = BASE64
        .decode(encoded)
        .map_err(|_| ApiError::bad_request("image_base64 is not valid base64"))?;
    let is_webp = bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP";
    if !is_webp {
        return Err(ApiError::bad_request("cue preview must be a WebP image"));
    }
    Ok(bytes)
}

/// Every Cue id the show still holds, across every Cuelist.
fn live_cue_ids(store: &ActiveShowRepository) -> Result<Vec<String>, ApiError> {
    let mut ids = Vec::new();
    for object in store.objects("cue_list").map_err(ApiError::store)? {
        let Some(cues) = object.body.get("cues").and_then(|cues| cues.as_array()) else {
            continue;
        };
        for cue in cues {
            if let Some(id) = cue.get("id").and_then(|id| id.as_str()) {
                ids.push(id.to_string());
            }
        }
    }
    Ok(ids)
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct ReplayKey {
    session_id: Uuid,
    show_id: Uuid,
    request_id: String,
}

struct ReplayEntry {
    thumbnails: Vec<wire::CueThumbnailUpload>,
    outcome: wire::CueThumbnailUpdateOutcome,
}

/// Session-scoped idempotency window for preview uploads.
///
/// The entry limit is deliberately smaller than other caches: each entry retains the uploaded
/// pictures, so this window trades replay depth for memory.
#[derive(Default)]
pub(super) struct CueThumbnailReplayCache {
    entries: HashMap<ReplayKey, ReplayEntry>,
    order: VecDeque<ReplayKey>,
}

impl CueThumbnailReplayCache {
    pub(super) fn get(
        &self,
        key: &ReplayKey,
        thumbnails: &[wire::CueThumbnailUpload],
    ) -> Result<Option<wire::CueThumbnailUpdateOutcome>, ApiError> {
        let Some(entry) = self.entries.get(key) else {
            return Ok(None);
        };
        if entry.thumbnails != thumbnails {
            return Err(ApiError::conflict(
                "request_id was already used for a different cue preview upload",
            ));
        }
        let mut replay = entry.outcome.clone();
        replay.replayed = true;
        Ok(Some(replay))
    }

    pub(super) fn insert(
        &mut self,
        key: ReplayKey,
        thumbnails: Vec<wire::CueThumbnailUpload>,
        outcome: wire::CueThumbnailUpdateOutcome,
    ) {
        if !self.entries.contains_key(&key) {
            self.order.push_back(key.clone());
        }
        self.entries.insert(
            key,
            ReplayEntry {
                thumbnails,
                outcome,
            },
        );
        while self.entries.len() > REQUEST_CACHE_ENTRY_LIMIT {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }
}
