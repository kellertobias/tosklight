//! Typed v2 requests for volatile output, Highlight, and media-server control.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct DmxOverrideRequest {
    pub request_id: String,
    pub universe: u16,
    pub address: u16,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub value: Option<u8>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum HighlightAction {
    On,
    Off,
    Toggle,
    Next,
    Previous,
    All,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct HighlightActionRequest {
    pub request_id: String,
    pub action: HighlightAction,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PatchPreviewHighlightRequest {
    pub request_id: String,
    pub active: bool,
    #[serde(default)]
    pub fixture_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct MediaThumbnailRefreshRequest {
    #[serde(default = "default_library_type")]
    pub library_type: u8,
    #[serde(default)]
    pub library_level: u8,
    #[serde(default)]
    pub library_1: u8,
    #[serde(default)]
    pub library_2: u8,
    #[serde(default)]
    pub library_3: u8,
    pub elements: Vec<u8>,
    #[serde(default = "default_width")]
    pub width: u16,
    #[serde(default = "default_height")]
    pub height: u16,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct MediaPreviewRefreshRequest {
    pub source: u16,
    #[serde(default = "default_width")]
    pub width: u16,
    #[serde(default = "default_height")]
    pub height: u16,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum MediaLibraryKind {
    Content,
    Mask,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct MediaLibrarySelectionRequest {
    pub request_id: String,
    pub expected_library_revision: String,
    pub layer_fixture_id: Uuid,
    pub kind: MediaLibraryKind,
    pub folder: u8,
    pub file: u8,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct MediaLibrarySelectionOutcome {
    pub request_id: String,
    pub library_revision: String,
    #[ts(type = "number")]
    pub programmer_revision: u64,
}

fn default_library_type() -> u8 {
    1
}

fn default_width() -> u16 {
    320
}

fn default_height() -> u16 {
    180
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_tolerate_future_fields_but_keep_known_fields_typed() {
        let request: HighlightActionRequest = serde_json::from_value(serde_json::json!({
            "request_id": "highlight-1",
            "action": "next",
            "future": true
        }))
        .unwrap();
        assert_eq!(request.action, HighlightAction::Next);
        assert!(
            serde_json::from_value::<DmxOverrideRequest>(serde_json::json!({
                "request_id": "dmx-1",
                "universe": 1,
                "address": "one",
                "value": 255
            }))
            .is_err()
        );
    }
}
