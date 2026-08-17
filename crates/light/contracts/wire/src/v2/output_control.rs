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

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct NativeMediaTextSlot {
    pub folder: u8,
    pub file: u8,
    pub name: String,
    pub enabled: bool,
    pub kind: String,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub text: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct NativeMediaEffectParameter {
    pub id: String,
    pub label: String,
    pub value: f32,
    pub default_value: f32,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct NativeMediaEffectSlot {
    pub index: usize,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub effect_type: Option<String>,
    pub label: String,
    pub enabled: bool,
    pub mix: f32,
    pub supported: bool,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub capability_detail: Option<String>,
    pub parameters: Vec<NativeMediaEffectParameter>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct NativeMediaSnapshot {
    pub endpoint: String,
    pub status: String,
    pub instance: String,
    pub outputs: usize,
    #[ts(type = "number")]
    pub catalog_revision: u64,
    pub catalog_items: usize,
    pub text_slots: Vec<NativeMediaTextSlot>,
    pub effect_controls_available: bool,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub output_id: Option<String>,
    #[serde(default)]
    pub effect_layers: Vec<Vec<NativeMediaEffectSlot>>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct NativeMediaEffectUpdateRequest {
    pub request_id: String,
    pub control_id: String,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub number_value: Option<f32>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub string_value: Option<String>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub boolean_value: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct NativeMediaTextUpdateRequest {
    pub request_id: String,
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct DiscoveredMediaAddressUpdateRequest {
    pub request_id: String,
    pub host: String,
    pub output_id: Uuid,
    pub universe: u16,
    pub start_address: u16,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct DiscoveredMediaOutput {
    pub id: Uuid,
    pub name: String,
    pub personality: String,
    pub protocol: String,
    pub universe: u16,
    pub start_address: u16,
    pub dmx_pending_restart: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct DiscoveredMediaServer {
    pub key: String,
    pub name: String,
    pub host: String,
    pub citp_port: u16,
    pub status: String,
    #[serde(default)]
    pub instance: Option<String>,
    pub outputs: Vec<DiscoveredMediaOutput>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub struct MediaServerDiscovery {
    pub servers: Vec<DiscoveredMediaServer>,
    #[serde(default)]
    pub discovery_error: Option<String>,
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

        let address: DiscoveredMediaAddressUpdateRequest =
            serde_json::from_value(serde_json::json!({
                "requestId": "media-address-1",
                "host": "127.0.0.1",
                "outputId": "6b1f0c2a-1111-4a2b-8c3d-000000000001",
                "universe": 9,
                "startAddress": 177,
                "future": true
            }))
            .unwrap();
        assert_eq!(address.start_address, 177);
    }
}
