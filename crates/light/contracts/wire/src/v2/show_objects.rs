//! Typed v2 active-show object snapshots and output-route mutation intents.

use super::{
    dynamics::{
        DynamicActivationBoundaryProjection, DynamicPhaseSpreadModeProjection,
        DynamicRationalProjection, DynamicRunModeProjection,
    },
    events::{OutputDeliveryMode, OutputProtocol, OutputRoute, OutputRouteChange},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ShowObjectRecord {
    pub kind: String,
    pub id: String,
    #[ts(type = "number")]
    pub revision: u64,
    pub updated_at: String,
    #[ts(type = "unknown")]
    pub body: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub validation_error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ShowObjectCollectionSnapshot {
    pub show_id: Uuid,
    #[ts(type = "number")]
    pub show_revision: u64,
    pub kind: String,
    pub objects: Vec<ShowObjectRecord>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ShowObjectExactSnapshot {
    pub show_id: Uuid,
    #[ts(type = "number")]
    pub show_revision: u64,
    pub kind: String,
    pub object_id: String,
    pub object: Option<ShowObjectRecord>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct OutputRouteActionRequest {
    pub request_id: String,
    pub action: OutputRouteAction,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutputRouteAction {
    Create {
        route_id: String,
        route: OutputRoute,
    },
    CreateRange {
        range_id: Uuid,
        route: OutputRoute,
        logical_universe_end: u16,
        destination_universe_end: u16,
    },
    Update {
        route_id: String,
        #[ts(type = "number")]
        expected_revision: u64,
        patch: OutputRoutePatch,
    },
    Delete {
        route_id: String,
        #[ts(type = "number")]
        expected_revision: u64,
    },
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct OutputRoutePatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub protocol: Option<OutputProtocol>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub logical_universe: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub destination_universe: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub delivery_mode: Option<OutputDeliveryMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub destination: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub minimum_slots: Option<u16>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct OutputRouteActionOutcome {
    pub request_id: String,
    pub replayed: bool,
    pub changes: Vec<OutputRouteChange>,
    #[ts(type = "number")]
    pub event_sequence: u64,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct UserLayoutActionRequest {
    pub request_id: String,
    pub action: UserLayoutAction,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UserLayoutAction {
    Update {
        #[ts(type = "number")]
        expected_revision: u64,
        patch: UserLayoutPatch,
    },
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct UserLayoutPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "unknown[] | null", optional = nullable)]
    pub desks: Option<Vec<serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub active_desk_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "unknown | null", optional = nullable)]
    pub window_settings: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PatchLayerActionRequest {
    pub request_id: String,
    pub action: PatchLayerAction,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PatchLayerAction {
    Save {
        #[ts(type = "number")]
        expected_revision: u64,
        layer: PatchLayerInput,
    },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PatchLayerInput {
    pub name: String,
    pub order: i32,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PreloadRecordActionRequest {
    pub request_id: String,
    pub action: PreloadRecordAction,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PreloadRecordAction {
    Preset {
        target_id: String,
        #[ts(type = "number")]
        expected_revision: u64,
        name: String,
        mode: PreloadPresetMode,
        family: PreloadPresetFamily,
    },
    Cue {
        cue_list_id: String,
        #[ts(type = "number")]
        expected_revision: u64,
        cue_number: f64,
        name: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PreloadPresetMode {
    Merge,
    Overwrite,
    AddMissingFixtures,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PreloadPresetFamily {
    Mixed,
    Intensity,
    Color,
    Position,
    Beam,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ShowObjectActionOutcome {
    pub request_id: String,
    pub replayed: bool,
    pub show_id: Uuid,
    #[ts(type = "number")]
    pub show_revision: u64,
    pub object: ShowObjectRecord,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(as = "Option<f64>", optional = nullable)]
    pub event_sequence: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct DynamicCreateActionRequest {
    pub request_id: String,
    /// Complete candidate definition. The server owns UUID, revision, and atomic slot conflict
    /// validation; the first lane must already be present and valid.
    #[ts(type = "unknown")]
    pub definition: serde_json::Value,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct DynamicPoolActionRequest {
    pub request_id: String,
    #[ts(type = "number")]
    pub expected_revision: u64,
    pub pool_number: u16,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct DynamicDeleteActionRequest {
    pub request_id: String,
    #[ts(type = "number")]
    pub expected_revision: u64,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct DynamicUpdateActionRequest {
    pub request_id: String,
    #[ts(type = "number")]
    pub expected_revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub mutation_group: Option<String>,
    pub intent: DynamicUpdateIntent,
}

/// One deliberate editor mutation. Domain-shaped payloads remain JSON at the transport package
/// boundary and are decoded and validated into `light-dynamics` types by the server adapter.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DynamicUpdateIntent {
    SetName {
        name: String,
    },
    SetColor {
        color: Option<String>,
    },
    SetIcon {
        icon: Option<String>,
    },
    SetTargetBinding {
        #[ts(type = "unknown")]
        target_binding: serde_json::Value,
    },
    AddLane {
        #[ts(type = "unknown")]
        lane: serde_json::Value,
        index: Option<usize>,
    },
    ReplaceLane {
        lane_id: Uuid,
        #[ts(type = "unknown")]
        lane: serde_json::Value,
    },
    DeleteLane {
        lane_id: Uuid,
    },
    MoveLane {
        lane_id: Uuid,
        index: usize,
    },
    SetPhase {
        #[ts(type = "unknown")]
        phase: serde_json::Value,
    },
    SetPhaseMode {
        phase_mode: DynamicPhaseSpreadModeProjection,
    },
    SetSpeed {
        #[ts(type = "unknown")]
        speed: serde_json::Value,
    },
    SetOverallSpeedMultiplier {
        multiplier: DynamicRationalProjection,
    },
    SetRunMode {
        run_mode: DynamicRunModeProjection,
    },
    SetActivation {
        #[ts(type = "unknown")]
        activation: serde_json::Value,
    },
    SetActivationBoundary {
        boundary: DynamicActivationBoundaryProjection,
    },
    AddRandomGroup {
        #[ts(type = "unknown")]
        group: serde_json::Value,
    },
    ReplaceRandomGroup {
        group_id: Uuid,
        #[ts(type = "unknown")]
        group: serde_json::Value,
    },
    DeleteRandomGroup {
        group_id: Uuid,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_route_update_is_partial_and_tolerant() {
        let request: OutputRouteActionRequest = serde_json::from_value(serde_json::json!({
            "request_id": "route-1",
            "action": {
                "type": "update",
                "route_id": "main",
                "expected_revision": 3,
                "patch": {"enabled": false, "future_patch": true},
                "future_action": true
            },
            "future_root": true
        }))
        .unwrap();
        assert!(matches!(
            request.action,
            OutputRouteAction::Update { patch, .. } if patch.enabled == Some(false)
        ));
    }
}
