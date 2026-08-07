//! Typed transport for persisted Cue preview pictures.
//!
//! A preview is drawn once, when the operator records or edits the Cue, and then stored with the
//! show. A desk opening a Cuelist compares the stored `state_hash` against the state it holds and
//! only redraws the Cues whose picture no longer tells the truth.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

/// One entry in the stored-preview index. Deliberately carries no pixels so a desk can decide what
/// to redraw without moving image data.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CueThumbnailEntry {
    pub cue_id: Uuid,
    #[schemars(length(min = 1, max = 128))]
    pub state_hash: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CueThumbnailIndex {
    pub show_id: Uuid,
    pub entries: Vec<CueThumbnailEntry>,
}

/// One picture being stored. `image_base64` is the encoded picture exactly as the desk drew it;
/// the server stores the bytes without re-encoding.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CueThumbnailUpload {
    pub cue_id: Uuid,
    #[schemars(length(min = 1, max = 128))]
    pub state_hash: String,
    #[schemars(length(min = 1, max = 700_000))]
    pub image_base64: String,
    #[schemars(range(min = 1, max = 1024))]
    pub width: u32,
    #[schemars(range(min = 1, max = 1024))]
    pub height: u32,
}

/// Editing an early Cue restages every Cue that tracks from it, so uploads arrive as one batch
/// rather than one request per Cue.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CueThumbnailUpdateRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    #[schemars(length(min = 1, max = 512))]
    pub thumbnails: Vec<CueThumbnailUpload>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CueThumbnailUpdateOutcome {
    pub request_id: String,
    pub correlation_id: Uuid,
    pub replayed: bool,
    pub show_id: Uuid,
    #[ts(type = "number")]
    pub stored: u32,
    /// Uploads discarded because the Cue is no longer in the show. Not an error: an operator can
    /// delete a Cue while its picture is still in flight.
    pub skipped_cue_ids: Vec<Uuid>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CueThumbnailErrorKind {
    Invalid,
    Unauthorized,
    Forbidden,
    NotFound,
    Conflict,
    Unavailable,
    Internal,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CueThumbnailErrorResponse {
    pub kind: CueThumbnailErrorKind,
    pub error: String,
    pub retryable: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn upload() -> serde_json::Value {
        serde_json::json!({
            "cue_id": Uuid::from_u128(1),
            "state_hash": "abc123",
            "image_base64": "UklGRg==",
            "width": 240,
            "height": 135
        })
    }

    #[test]
    fn an_update_request_is_strict_and_keeps_scope_server_authored() {
        let request = serde_json::json!({
            "request_id": "thumbnails-1",
            "thumbnails": [upload()]
        });
        assert!(serde_json::from_value::<CueThumbnailUpdateRequest>(request.clone()).is_ok());
        for forged in ["desk_id", "show_id", "user_id", "cue_list_id"] {
            let mut forged_request = request.clone();
            forged_request[forged] = serde_json::json!("forged");
            assert!(serde_json::from_value::<CueThumbnailUpdateRequest>(forged_request).is_err());
        }
    }

    #[test]
    fn an_upload_rejects_unknown_fields() {
        let mut extended = upload();
        extended["storage_path"] = serde_json::json!("/somewhere");
        assert!(serde_json::from_value::<CueThumbnailUpload>(extended).is_err());
    }

    #[test]
    fn the_index_round_trips_without_pixels() {
        let index = CueThumbnailIndex {
            show_id: Uuid::from_u128(9),
            entries: vec![CueThumbnailEntry {
                cue_id: Uuid::from_u128(1),
                state_hash: "abc123".into(),
                updated_at: "2026-08-07T12:00:00.000Z".into(),
            }],
        };
        let encoded = serde_json::to_value(&index).unwrap();
        assert!(encoded["entries"][0].get("image_base64").is_none());
        assert_eq!(
            serde_json::from_value::<CueThumbnailIndex>(encoded).unwrap(),
            index
        );
    }
}
