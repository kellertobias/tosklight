//! Typed management of one stored Group: properties, undo, frozen refresh, and derived detach.
//!
//! Scope is deliberately server-authored. A client declares only the operation, the exact Group
//! storage ID it observed, and the revisions it expects; desk, user, session, and Show identity
//! come from the authenticated session and the request path.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct GroupMappingPosition3d {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct GroupMappingVector3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum GroupMappingProjectionPreset {
    Top,
    Front,
    Back,
    Left,
    Right,
}

/// How a Stage position becomes the pair the shape ranks on. Absent means `planar`, which is
/// what every projection stored before the other two kinds existed is.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum GroupMappingProjectionKind {
    #[default]
    Planar,
    Cylindrical,
    Spherical,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum GroupMappingRankDirection {
    Ascending,
    Descending,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum GroupMappingRadialDirection {
    Outward,
    Inward,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum GroupMappingRadarSweep {
    Clockwise,
    CounterClockwise,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct GroupMappingProjection {
    pub anchor: GroupMappingPosition3d,
    pub view_direction: GroupMappingVector3,
    pub rotation_degrees: f64,
    #[ts(optional = nullable)]
    pub preset: Option<GroupMappingProjectionPreset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub kind: Option<GroupMappingProjectionKind>,
    /// Euler degrees about X, Y then Z turning world +Z into the cylinder axis. Cylindrical only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub axis_rotation: Option<GroupMappingVector3>,
    /// Cylindrical: where the spread starts around the axis. Spherical: the centre's azimuth.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub start_angle_degrees: Option<f64>,
    /// Spherical only: the centre's elevation above the plane perpendicular to world +Z.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub elevation_degrees: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GroupMappingShape {
    Grid {
        angle_degrees: f64,
        direction: GroupMappingRankDirection,
    },
    Radial {
        center_u: f64,
        center_v: f64,
        direction: GroupMappingRadialDirection,
    },
    Radar {
        center_u: f64,
        center_v: f64,
        start_angle_degrees: f64,
        sweep: GroupMappingRadarSweep,
    },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct GroupSpatialSelectionMapping {
    pub projection: GroupMappingProjection,
    pub shape: GroupMappingShape,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum GroupMappingProvenanceProjection {
    None {},
    Local { group_id: String },
    Inherited { source_group_ids: Vec<String> },
    MixedSourceMappings {},
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum GroupSpatialWarningProjection {
    MissingPosition { fixture_id: Uuid },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct GroupSpatialRankProjection {
    pub fixture_id: Uuid,
    pub rank: usize,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct GroupProjectedPositionProjection {
    pub fixture_id: Uuid,
    #[ts(optional = nullable)]
    pub u: Option<f64>,
    #[ts(optional = nullable)]
    pub v: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct GroupResolvedSpatialProjection {
    pub source_order: Vec<Uuid>,
    #[ts(optional = nullable)]
    pub effective_mapping: Option<GroupSpatialSelectionMapping>,
    pub mapping_provenance: GroupMappingProvenanceProjection,
    pub ordered_fixture_ids: Vec<Uuid>,
    pub projected_positions: Vec<GroupProjectedPositionProjection>,
    pub ranks: Vec<GroupSpatialRankProjection>,
    pub rank_count: usize,
    pub warnings: Vec<GroupSpatialWarningProjection>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct GroupPropertiesUpdate {
    #[schemars(length(min = 1, max = 256))]
    pub name: String,
    #[schemars(length(max = 64))]
    #[ts(optional = nullable)]
    pub color: Option<String>,
    #[schemars(length(max = 64))]
    #[ts(optional = nullable)]
    pub icon: Option<String>,
}

/// Exact source authority a client observed. A mismatch fails before anything mutates.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct GroupSourceExpectation {
    #[schemars(length(min = 1, max = 256))]
    pub source_group_id: String,
    #[schemars(range(max = 9007199254740991_u64))]
    #[ts(as = "Option<f64>", optional = nullable)]
    pub expected_source_revision: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GroupManagementOperation {
    UpdateProperties {
        properties: GroupPropertiesUpdate,
    },
    /// Declared as an empty struct variant so `deny_unknown_fields` still applies; serde does not
    /// enforce it for an internally tagged unit variant.
    Undo {},
    RefreshFrozen {
        #[ts(optional = nullable)]
        expected_source: Option<GroupSourceExpectation>,
    },
    DetachDerived {
        #[ts(optional = nullable)]
        expected_source: Option<GroupSourceExpectation>,
    },
    SetSpatialMapping {
        mapping: GroupSpatialSelectionMapping,
    },
    RemoveSpatialMapping {},
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct GroupManagementRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    #[schemars(length(min = 1, max = 256))]
    pub group_id: String,
    pub operation: GroupManagementOperation,
    #[schemars(range(max = 9007199254740991_u64))]
    #[ts(type = "number")]
    pub expected_object_revision: u64,
    #[schemars(range(max = 9007199254740991_u64))]
    #[ts(type = "number")]
    pub expected_show_revision: u64,
}

/// Authoritative lossless Group projection. These operations never delete their target.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct GroupManagementObjectProjection {
    #[schemars(length(min = 1, max = 256))]
    pub object_id: String,
    #[ts(type = "number")]
    pub object_revision: u64,
    #[ts(type = "unknown")]
    pub body: Arc<Value>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct GroupSettingsSnapshot {
    pub show_id: Uuid,
    #[ts(type = "number")]
    pub show_revision: u64,
    pub group: GroupManagementObjectProjection,
    pub resolved_spatial: GroupResolvedSpatialProjection,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum GroupManagementOutcome {
    Changed {
        request_id: String,
        correlation_id: Uuid,
        replayed: bool,
        show_id: Uuid,
        #[ts(type = "number")]
        show_revision: u64,
        group: GroupManagementObjectProjection,
        #[ts(type = "number")]
        show_event_sequence: u64,
        #[ts(optional = nullable)]
        persistence_warning: Option<String>,
    },
    NoChange {
        request_id: String,
        correlation_id: Uuid,
        replayed: bool,
        show_id: Uuid,
        #[ts(type = "number")]
        show_revision: u64,
        group: GroupManagementObjectProjection,
        #[ts(optional = nullable)]
        persistence_warning: Option<String>,
    },
}

impl GroupManagementOutcome {
    pub const fn group_revision(&self) -> u64 {
        match self {
            Self::Changed { group, .. } | Self::NoChange { group, .. } => group.object_revision,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum GroupManagementErrorKind {
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
pub struct GroupManagementErrorResponse {
    pub kind: GroupManagementErrorKind,
    pub error: String,
    #[ts(as = "Option<f64>", optional = nullable)]
    pub current_revision: Option<u64>,
    #[ts(as = "Option<f64>", optional = nullable)]
    pub current_related_revision: Option<u64>,
    pub retryable: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_ignores_unknown_scope_fields_so_scope_stays_server_authored() {
        let request = serde_json::json!({
            "request_id":"manage-1",
            "group_id":"house",
            "operation":{
                "type":"update_properties",
                "properties":{"name":"Front wash","color":"#ff0000","icon":"◆"}
            },
            "expected_object_revision":4,
            "expected_show_revision":9
        });
        assert!(serde_json::from_value::<GroupManagementRequest>(request.clone()).is_ok());
        for forged in ["desk_id", "show_id", "user_id", "session_id"] {
            let mut forged_request = request.clone();
            forged_request[forged] = serde_json::json!("forged");
            assert!(serde_json::from_value::<GroupManagementRequest>(forged_request).is_ok());
        }
    }

    #[test]
    fn tagged_operations_accept_unknown_fields_while_outcomes_stay_strict() {
        let operation = serde_json::json!({"type":"undo","group_id":"house"});
        assert!(serde_json::from_value::<GroupManagementOperation>(operation).is_ok());
        let outcome = serde_json::json!({"status":"changed","unexpected":true});
        assert!(serde_json::from_value::<GroupManagementOutcome>(outcome).is_err());
    }

    #[test]
    fn additional_operation_fields_are_ignored_instead_of_becoming_authority() {
        let operation = serde_json::json!({
            "type":"refresh_frozen",
            "expected_source":{"source_group_id":"source","expected_source_revision":2},
            "fixtures":["forged"]
        });
        assert!(matches!(
            serde_json::from_value::<GroupManagementOperation>(operation).unwrap(),
            GroupManagementOperation::RefreshFrozen { .. }
        ));
        let expectation = serde_json::json!({
            "source_group_id":"source",
            "expected_source_revision":2,
            "captured_at":"2020-01-01T00:00:00Z"
        });
        assert_eq!(
            serde_json::from_value::<GroupSourceExpectation>(expectation).unwrap(),
            GroupSourceExpectation {
                source_group_id: "source".into(),
                expected_source_revision: Some(2),
            }
        );
    }

    #[test]
    fn spatial_mapping_intents_are_typed_strict_and_explicit_about_removal() {
        let mapping = serde_json::json!({
            "projection": {
                "anchor": {"x":0.0,"y":1.0,"z":2.0},
                "view_direction": {"x":0.0,"y":0.0,"z":-1.0},
                "rotation_degrees": 15.0,
                "preset": null
            },
            "shape": {"type":"radial","center_u":1.5,"center_v":-2.0,"direction":"outward"}
        });
        assert!(
            serde_json::from_value::<GroupManagementOperation>(serde_json::json!({
                "type":"set_spatial_mapping",
                "mapping":mapping
            }))
            .is_ok()
        );
        assert!(
            serde_json::from_value::<GroupManagementOperation>(serde_json::json!({
                "type":"remove_spatial_mapping"
            }))
            .is_ok()
        );
        assert!(
            serde_json::from_value::<GroupManagementOperation>(serde_json::json!({
                "type":"set_spatial_mapping",
                "mapping": {
                    "projection": {
                        "anchor":{"x":0.0,"y":0.0,"z":0.0},
                        "view_direction":{"x":0.0,"y":0.0,"z":-1.0},
                        "rotation_degrees":0.0,
                        "preset":"top",
                        "future":true
                    },
                    "shape":{"type":"grid","angle_degrees":0.0,"direction":"ascending"}
                }
            }))
            .is_ok()
        );
        assert!(
            serde_json::from_value::<GroupManagementOperation>(serde_json::json!({
                "type":"remove_spatial_mapping", "mapping":mapping
            }))
            .is_ok()
        );
    }
}
