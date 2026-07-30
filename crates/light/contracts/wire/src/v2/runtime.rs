//! Runtime lifecycle snapshots used while a desk connects to the v2 API.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct RuntimeSessionCreateRequest {
    pub username: String,
    #[serde(default)]
    pub desk_id: Option<Uuid>,
    #[serde(default)]
    pub client_id: Option<Uuid>,
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, TS)]
pub struct RuntimeDeskUser {
    pub id: Uuid,
    pub name: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct RuntimePlaybackSurfaceRow {
    pub first_playback_slot: u8,
    pub has_fader: bool,
    pub button_count: u8,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct RuntimePlaybackSurfaceLayout {
    pub playbacks_per_row: u8,
    pub rows: Vec<RuntimePlaybackSurfaceRow>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct RuntimeControlDesk {
    pub id: Uuid,
    pub name: String,
    pub osc_alias: String,
    pub columns: u8,
    pub rows: u8,
    pub buttons: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub playback_layout: Option<RuntimePlaybackSurfaceLayout>,
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, TS)]
pub struct RuntimeSessionResponse {
    pub session_id: Uuid,
    pub client_id: Uuid,
    pub token: String,
    pub user: RuntimeDeskUser,
    pub desk: RuntimeControlDesk,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct RuntimeRevisionCopySource {
    pub show_id: Uuid,
    pub show_name: String,
    #[ts(type = "number")]
    pub revision: u64,
    pub revision_name: String,
    pub copied_at: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct RuntimeShowEntry {
    pub id: Uuid,
    pub name: String,
    pub path: String,
    #[ts(type = "number")]
    pub revision: u64,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision_copy: Option<RuntimeRevisionCopySource>,
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, TS)]
pub struct RuntimeOutputHealth {
    #[ts(type = "number")]
    pub frames_sent: u64,
    #[ts(type = "number")]
    pub packets_sent: u64,
    #[ts(type = "number")]
    pub send_errors: u64,
    #[ts(type = "number")]
    pub deadline_misses: u64,
    #[ts(type = "number")]
    pub maximum_lateness_micros: u64,
    pub frame_hz: f32,
    #[ts(type = "number")]
    pub last_tick_micros: u64,
    #[ts(type = "number")]
    pub maximum_tick_micros: u64,
    #[ts(type = "number[]")]
    pub tick_duration_bucket_bounds_micros: Vec<u64>,
    #[ts(type = "number[]")]
    pub tick_duration_bucket_counts: Vec<u64>,
    pub scheduler_utilization: f32,
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, TS)]
pub struct RuntimeClientSummary {
    pub client_id: Uuid,
    pub name: String,
    pub connected: bool,
    pub last_connected_at: Option<String>,
    pub desk: RuntimeControlDesk,
    pub can_remove: bool,
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, TS)]
pub struct RuntimeAttributeDescriptor {
    pub id: String,
    pub label: String,
    pub family: String,
    pub value_type: String,
    pub default_unit: Option<String>,
    pub display_unit: Option<String>,
    pub physical_unit: Option<String>,
    pub normalized_min: Option<f32>,
    pub normalized_max: Option<f32>,
    pub domain_min: Option<f32>,
    pub domain_max: Option<f32>,
    pub cyclic: bool,
    pub recordable: bool,
    pub encoder_group: super::attribute_configuration::AttributeEncoderGroup,
    pub encoder_page: u16,
    pub encoder_slot: u8,
    pub built_in: bool,
    pub retired: bool,
    pub activation_group_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct RuntimeHighlightFixture {
    pub fixture_id: Uuid,
    pub name: Option<String>,
    pub number: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct RuntimeHighlightState {
    pub active: bool,
    pub mode: String,
    pub output_enabled: bool,
    pub capture_only: bool,
    pub remembered: Vec<RuntimeHighlightFixture>,
    pub active_index: Option<usize>,
    pub active_fixture: Option<RuntimeHighlightFixture>,
    pub can_previous: bool,
    pub can_next: bool,
    pub owner_user_id: Option<Uuid>,
    pub owner_user_name: Option<String>,
    pub message: Option<String>,
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, TS)]
pub struct RuntimeBootstrapHighlightState {
    pub session_id: Uuid,
    pub desk_id: Uuid,
    pub user_id: Uuid,
    pub state: RuntimeHighlightState,
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, TS)]
pub struct RuntimeBootstrapSnapshot {
    pub api_version: String,
    pub attribute_registry: Vec<RuntimeAttributeDescriptor>,
    pub users: Vec<RuntimeDeskUser>,
    pub desks: Vec<RuntimeControlDesk>,
    pub clients: Vec<RuntimeClientSummary>,
    pub active_show: Option<RuntimeShowEntry>,
    /// Retained as an empty compatibility collection until the facade is removed.
    #[ts(type = "unknown[]")]
    pub active_programmers: Vec<serde_json::Value>,
    pub highlight_states: Vec<RuntimeBootstrapHighlightState>,
    pub frame_rate_hz: u16,
    pub output_health: RuntimeOutputHealth,
    pub active_timecode_source: Option<String>,
    pub active_timecode: Option<String>,
    pub active_show_error: Option<String>,
    pub hardware_connected: bool,
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, TS)]
pub struct RuntimeReadinessSnapshot {
    pub status: String,
    pub active_show: Option<Uuid>,
    pub active_show_error: Option<String>,
    pub recovery_mode: bool,
    #[ts(type = "number")]
    pub snapshot_revision: u64,
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, TS)]
pub struct RuntimeVisualizationDiagnostics {
    #[ts(type = "number")]
    pub normal_subscribers: u64,
    #[ts(type = "number")]
    pub preload_subscribers: u64,
    #[ts(type = "number")]
    pub projections: u64,
    #[ts(type = "number")]
    pub projection_micros: u64,
    #[ts(type = "number")]
    pub payload_bytes: u64,
    #[ts(type = "number")]
    pub source_age_millis: u64,
    #[ts(type = "number")]
    pub skipped_source_frames: u64,
    #[ts(type = "number")]
    pub snapshot_requests: u64,
    #[ts(type = "number")]
    pub snapshot_projection_micros: u64,
    #[ts(type = "number")]
    pub snapshot_serialization_micros: u64,
    #[ts(type = "number")]
    pub snapshot_payload_bytes: u64,
    #[ts(type = "number")]
    pub snapshot_source_frame: u64,
    #[ts(type = "number")]
    pub snapshot_source_age_millis: u64,
    #[ts(type = "number")]
    pub stream_serializations: u64,
    #[ts(type = "number")]
    pub stream_serialization_micros: u64,
    #[ts(type = "number")]
    pub stream_payload_bytes: u64,
    #[ts(type = "number")]
    pub stream_sends: u64,
    #[ts(type = "number")]
    pub stream_send_micros: u64,
    #[ts(type = "number")]
    pub stream_send_failures: u64,
    #[ts(type = "number")]
    pub stream_queue_depth: u64,
    #[ts(type = "number")]
    pub stream_queue_drops: u64,
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, TS)]
pub struct RuntimeDiagnosticsSnapshot {
    pub output: RuntimeOutputHealth,
    pub output_bind_ip: String,
    #[ts(type = "unknown")]
    pub output_routes: serde_json::Value,
    #[ts(type = "unknown")]
    pub route_send_errors: serde_json::Value,
    #[ts(type = "unknown")]
    pub active_programmers: serde_json::Value,
    #[ts(type = "unknown")]
    pub active_playbacks: serde_json::Value,
    #[ts(type = "unknown")]
    pub move_in_black: serde_json::Value,
    pub timecode_source: Option<String>,
    #[ts(type = "unknown")]
    pub media_servers: serde_json::Value,
    #[ts(type = "number")]
    pub snapshot_revision: u64,
    #[ts(type = "unknown")]
    pub programmer_action_timing: serde_json::Value,
    pub visualization: RuntimeVisualizationDiagnostics,
}

#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, TS)]
pub struct RuntimePerformanceDiagnosticsSnapshot {
    pub output: RuntimeOutputHealth,
    #[ts(type = "unknown")]
    pub programmer_action_timing: serde_json::Value,
    pub visualization: RuntimeVisualizationDiagnostics,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_request_accepts_unknown_fields_and_validates_known_fields() {
        let request: RuntimeSessionCreateRequest = serde_json::from_value(serde_json::json!({
            "username": "Operator",
            "future_client_hint": true
        }))
        .unwrap();
        assert_eq!(request.username, "Operator");
        assert_eq!(request.desk_id, None);

        let error = serde_json::from_value::<RuntimeSessionCreateRequest>(serde_json::json!({
            "username": 7
        }))
        .unwrap_err();
        assert!(error.to_string().contains("string"));
    }
}
