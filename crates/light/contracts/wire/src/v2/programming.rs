//! User-scoped, recordable Programmer value projections and repair snapshots.

use super::dynamics::{
    DynamicDefinitionProjection, DynamicInstanceOverridesProjection, DynamicReferenceProjection,
    DynamicValueAddressProjection, DynamicValueTimingProjection,
};
use super::events::EventSnapshotCursor;
use super::programming_intent::{
    ProgrammingColorProgram, ProgrammingComponent, ProgrammingGroupFamilyAssignment,
    ProgrammingPositionIntent, ProgrammingZoomIntent,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingColorXyz {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ProgrammingAttributeValue {
    Normalized(f32),
    Spread(Vec<f32>),
    Discrete(String),
    ColorXyz(ProgrammingColorXyz),
    ColorProgram(ProgrammingColorProgram),
    Position(ProgrammingPositionIntent),
    Zoom(ProgrammingZoomIntent),
    GroupFamily(Box<ProgrammingGroupFamilyAssignment>),
    RawDmx(u8),
    RawDmxExact(u32),
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ReplacementProfileContext {
    pub profile_id: Uuid,
    #[ts(type = "number")]
    pub profile_revision: u64,
    pub mode_id: Uuid,
}
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ReplacementHeadTarget {
    pub profile_head_id: Uuid,
    pub fixture_id: Uuid,
}
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ReplacementProgramProjection {
    pub source_owner: Uuid,
    pub source_profile: ReplacementProfileContext,
    pub source_head_id: Uuid,
    pub target_profile: ReplacementProfileContext,
    pub targets: Vec<ReplacementHeadTarget>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingFixtureValue {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub replacement_projection: Option<ReplacementProgramProjection>,
    pub fixture_id: Uuid,
    pub attribute: String,
    pub value: ProgrammingAttributeValue,
    #[ts(type = "number")]
    pub programmer_order: u64,
    pub fade: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable, type = "number | null")]
    pub fade_millis: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable, type = "number | null")]
    pub delay_millis: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingGroupValue {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub replacement_projections:
        Option<std::collections::HashMap<Uuid, ReplacementProgramProjection>>,
    pub group_id: String,
    pub attribute: String,
    pub value: ProgrammingAttributeValue,
    #[ts(type = "number")]
    pub programmer_order: u64,
    pub fade: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable, type = "number | null")]
    pub fade_millis: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable, type = "number | null")]
    pub delay_millis: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProgrammingDynamicSemanticValue {
    Static {
        value: ProgrammingAttributeValue,
        timing: DynamicValueTimingProjection,
    },
    DynamicOn {
        instance_link: Uuid,
        dynamic: Box<DynamicReferenceProjection>,
        lane_id: Uuid,
        overrides: DynamicInstanceOverridesProjection,
        timing: DynamicValueTimingProjection,
    },
    DynamicOff {
        instance_link: Uuid,
        timing: DynamicValueTimingProjection,
    },
    FixAt {
        value: f32,
        timing: DynamicValueTimingProjection,
    },
    ProgrammingFixAt {
        mask: ProgrammingFamilyFixAt,
        timing: DynamicValueTimingProjection,
    },
    ProgrammingRelease {
        component: Option<ProgrammingComponent>,
    },
    Release,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingFamilyFixAt {
    pub address: DynamicValueAddressProjection,
    pub family: ProgrammingAttributeValue,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingDynamicValue {
    pub fixture_id: Uuid,
    pub attribute: String,
    pub value: ProgrammingDynamicSemanticValue,
    #[ts(type = "number")]
    pub programmer_order: u64,
    #[ts(type = "number")]
    pub changed_at_millis: u64,
}

/// Full retained projection of the Programmer's normal, recordable values.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingValuesProjection {
    #[ts(type = "number")]
    pub revision: u64,
    pub fixture_values: Vec<ProgrammingFixtureValue>,
    pub group_values: Vec<ProgrammingGroupValue>,
    /// Embedded Dynamic fallbacks deduplicated across every Dynamic-controlled address.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dynamic_definitions: Vec<DynamicDefinitionProjection>,
    pub dynamic_values: Vec<ProgrammingDynamicValue>,
}

/// Authoritative capture routing for the Programmer.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingCaptureModeProjection {
    #[ts(type = "number")]
    pub revision: u64,
    pub blind: bool,
    pub preview: bool,
    pub preload_capture_programmer: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingCaptureModeChange {
    pub projection: ProgrammingCaptureModeProjection,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingCaptureModeSnapshot {
    pub cursor: EventSnapshotCursor,
    pub projection: ProgrammingCaptureModeProjection,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingValuesChange {
    #[ts(type = "number")]
    pub revision: u64,
    pub fixture_values: Vec<ProgrammingFixtureValue>,
    pub removed_fixture_values: Vec<ProgrammingFixtureValueAddress>,
    pub group_values: Vec<ProgrammingGroupValue>,
    pub removed_group_values: Vec<ProgrammingGroupValueAddress>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dynamic_definitions: Vec<DynamicDefinitionProjection>,
    pub dynamic_values: Vec<ProgrammingDynamicValue>,
    pub removed_dynamic_values: Vec<ProgrammingDynamicValueAddress>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingFixtureValueAddress {
    pub fixture_id: Uuid,
    pub attribute: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingDynamicValueAddress {
    pub fixture_id: Uuid,
    pub attribute: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub instance_link: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub lane_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub component: Option<ProgrammingComponent>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingGroupValueAddress {
    pub group_id: String,
    pub attribute: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingValuesSnapshot {
    pub cursor: EventSnapshotCursor,
    pub projection: ProgrammingValuesProjection,
}

/// One authenticated, idempotent mutation of normal, recordable Programmer values.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingValuesActionRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    #[ts(type = "number")]
    pub expected_revision: u64,
    #[ts(type = "number")]
    pub expected_capture_mode_revision: u64,
    pub action: ProgrammingValuesAction,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingIndexedPresetTarget {
    pub fixture_id: Uuid,
    pub function_id: Uuid,
    pub expected_profile_revision: u32,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProgrammingValuesAction {
    /// Ends only this session/desk/lane's retained semantic gesture. Does not change values.
    /// Stale revisions and an already-ended gesture return a quiet no-change outcome.
    FinishGesture {
        attribute: String,
        #[schemars(length(min = 1, max = 128))]
        undo_group: String,
    },
    /// One application-owned fixture/head value intent. The server resolves ordered spreads,
    /// relative steps, and any configured linked-attribute captures atomically.
    ApplyIntent {
        #[serde(default)]
        #[schemars(length(max = 10_000))]
        fixture_ids: Vec<Uuid>,
        /// Live Group target. Exactly one of `fixture_ids` or `group_id` must be supplied.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional = nullable)]
        group_id: Option<String>,
        attribute: String,
        operation: ProgrammingValueOperation,
        /// Optional identity shared by samples from one continuous encoder gesture. The server
        /// keeps all samples in one Programmer undo entry; taps and wheel ticks omit it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional = nullable)]
        undo_group: Option<String>,
        #[serde(default)]
        timing: ProgrammingValueTiming,
        /// TL-594: the exact accepted source the surface displayed. First-edit adoption uses
        /// only that leased source and holds quietly when it is gone; omitted, the edit keeps
        /// the latest-accepted adoption (OSC and HTTP integrators).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional = nullable)]
        displayed_source: Option<super::output_readouts::DisplayedSourceRef>,
        /// TL-554: the reference head a Direct (`native`) edit names. Absent, the server takes
        /// the first verified head of the ordered selection.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional = nullable)]
        native_reference: Option<super::native_color::NativeColorReferenceRef>,
        /// TL-554: the operator's explicit starting colour when the first semantic edit of a
        /// Direct value cannot adopt its (unknown) visible appearance.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional = nullable)]
        explicit_color_start: Option<super::native_color::ExplicitColorStart>,
    },
    /// Resolves authored fixed/indexed functions from the active show's embedded fixture
    /// revisions after checking that the selection shown by the modal is still current.
    ApplyIndexedPreset {
        #[ts(type = "number")]
        expected_selection_revision: u64,
        attribute: String,
        #[schemars(length(min = 1, max = 10_000))]
        targets: Vec<ProgrammingIndexedPresetTarget>,
    },
    /// Server-side fan-out over an explicitly ordered selection (see
    /// `ProgrammingValueMutation::SetSelection`).
    SetSelection {
        #[schemars(length(max = 10_000))]
        fixture_ids: Vec<Uuid>,
        attribute: String,
        value: ProgrammingAttributeValue,
        #[serde(default)]
        timing: ProgrammingValueTiming,
    },
    SetFixture {
        fixture_id: Uuid,
        attribute: String,
        value: ProgrammingAttributeValue,
        #[serde(default)]
        timing: ProgrammingValueTiming,
    },
    ReleaseFixture {
        fixture_id: Uuid,
        attribute: String,
    },
    SetGroup {
        group_id: String,
        attribute: String,
        value: ProgrammingAttributeValue,
        #[serde(default)]
        timing: ProgrammingValueTiming,
    },
    ReleaseGroup {
        group_id: String,
        attribute: String,
    },
    /// Server-side color-range fan-out over an explicitly ordered selection: the picker
    /// endpoints interpolate hue-aware across the fixture order — `hue_travel` carries the
    /// gesture's total signed hue distance in revolutions (1.0 = once around the wheel), so
    /// long-way-around and multi-revolution ranges are expressible — and every fixture stores
    /// only the color channels its heads support.
    SetSelectionColorRange {
        #[schemars(length(max = 10_000))]
        fixture_ids: Vec<Uuid>,
        start: ProgrammingPickerColor,
        end: ProgrammingPickerColor,
        hue_travel: f32,
        #[schemars(range(min = 0.0, max = 1.0))]
        brightness: f32,
        #[serde(default)]
        timing: ProgrammingValueTiming,
    },
    Batch {
        #[schemars(length(max = 10_000))]
        mutations: Vec<ProgrammingValueMutation>,
    },
    Clear,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProgrammingValueOperation {
    ComponentEdits {
        #[schemars(length(max = 512))]
        edits: Vec<super::programming_intent::ProgrammingComponentEdit>,
    },
    AbsoluteSet {
        value: ProgrammingAttributeValue,
    },
    RelativeStep {
        delta: f32,
    },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProgrammingValueMutation {
    /// Server-side fan-out over an explicitly ordered selection: `Spread` control points
    /// interpolate across the given fixture order; any other value applies uniformly. This is the
    /// shared fan-out vocabulary (ordered selection + typed operation payload) also used by the
    /// color and position fan-out families.
    SetSelection {
        #[schemars(length(max = 10_000))]
        fixture_ids: Vec<Uuid>,
        attribute: String,
        value: ProgrammingAttributeValue,
        #[serde(default)]
        timing: ProgrammingValueTiming,
    },
    /// Server-side color-range fan-out over an explicitly ordered selection: the picker
    /// endpoints interpolate hue-aware across the fixture order — `hue_travel` carries the
    /// gesture's total signed hue distance in revolutions (1.0 = once around the wheel), so
    /// long-way-around and multi-revolution ranges are expressible — and every fixture stores
    /// only the color channels its heads support.
    SetSelectionColorRange {
        #[schemars(length(max = 10_000))]
        fixture_ids: Vec<Uuid>,
        start: ProgrammingPickerColor,
        end: ProgrammingPickerColor,
        hue_travel: f32,
        #[schemars(range(min = 0.0, max = 1.0))]
        brightness: f32,
        #[serde(default)]
        timing: ProgrammingValueTiming,
    },
    SetFixture {
        fixture_id: Uuid,
        attribute: String,
        value: ProgrammingAttributeValue,
        #[serde(default)]
        timing: ProgrammingValueTiming,
    },
    ReleaseFixture {
        fixture_id: Uuid,
        attribute: String,
    },
    SetGroup {
        group_id: String,
        attribute: String,
        value: ProgrammingAttributeValue,
        #[serde(default)]
        timing: ProgrammingValueTiming,
    },
    ReleaseGroup {
        group_id: String,
        attribute: String,
    },
}

/// Hue/saturation picker coordinates (both 0..1) as captured by the operator color dialog.
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingPickerColor {
    #[schemars(range(min = 0.0, max = 1.0))]
    pub hue: f32,
    #[schemars(range(min = 0.0, max = 1.0))]
    pub saturation: f32,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingValueTiming {
    #[serde(default)]
    pub fade: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable, type = "number | null")]
    pub fade_millis: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable, type = "number | null")]
    pub delay_millis: Option<u64>,
}

/// Typed result for one Programmer-values action. No-change results deliberately omit the full
/// projection so interaction-only actions do not force projection materialization or transport.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingValuesActionOutcome {
    pub request_id: String,
    pub correlation_id: Uuid,
    #[ts(type = "number")]
    pub revision: u64,
    #[ts(type = "number")]
    pub capture_mode_revision: u64,
    #[serde(flatten)]
    pub outcome: ProgrammingValuesActionState,
    pub replayed: bool,
    /// TL-594: present when an edit that named a displayed source was held quietly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub hold: Option<super::output_readouts::ProgrammingValuesHoldReason>,
    /// TL-554: the first semantic edit of a Direct value adopted this starting value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub color_adoption: Option<super::native_color::ColorAdoptionReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub warning: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ProgrammingValuesActionState {
    Changed {
        projection: ProgrammingValuesProjection,
        #[ts(type = "number")]
        event_sequence: u64,
    },
    NoChange,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingValuesErrorResponse {
    pub kind: ProgrammingValuesErrorKind,
    pub error: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(as = "Option<f64>", optional = nullable)]
    pub current_revision: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(as = "Option<f64>", optional = nullable)]
    pub current_capture_mode_revision: Option<u64>,
    pub retryable: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProgrammingValuesErrorKind {
    Invalid,
    Unauthorized,
    Forbidden,
    NotFound,
    Conflict,
    Unavailable,
    Internal,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacement_projection_wire_preserves_dormant_mapping_and_legacy_absence() {
        let profile = ReplacementProfileContext {
            profile_id: Uuid::from_u128(1),
            profile_revision: 1,
            mode_id: Uuid::from_u128(2),
        };
        let projection = ReplacementProgramProjection {
            source_owner: Uuid::from_u128(3),
            source_profile: profile.clone(),
            source_head_id: Uuid::from_u128(4),
            target_profile: profile,
            targets: Vec::new(),
        };
        let value = ProgrammingFixtureValue {
            replacement_projection: Some(projection.clone()),
            fixture_id: projection.source_owner,
            attribute: "intensity".into(),
            value: ProgrammingAttributeValue::Normalized(0.5),
            programmer_order: 9,
            fade: false,
            fade_millis: None,
            delay_millis: None,
        };
        let raw = serde_json::to_value(&value).unwrap();
        assert_eq!(
            raw["replacement_projection"]["targets"],
            serde_json::json!([])
        );
        assert_eq!(
            serde_json::from_value::<ProgrammingFixtureValue>(raw).unwrap(),
            value
        );
        let mut legacy = value.clone();
        legacy.replacement_projection = None;
        let raw = serde_json::to_value(&legacy).unwrap();
        assert!(raw.get("replacement_projection").is_none());
        assert_eq!(
            serde_json::from_value::<ProgrammingFixtureValue>(raw).unwrap(),
            legacy
        );
        let group = ProgrammingGroupValue {
            replacement_projections: Some(std::collections::HashMap::from([(
                projection.source_owner,
                projection,
            )])),
            group_id: "Front".into(),
            attribute: "intensity".into(),
            value: ProgrammingAttributeValue::Normalized(0.7),
            programmer_order: 10,
            fade: true,
            fade_millis: Some(100),
            delay_millis: Some(20),
        };
        assert_eq!(
            serde_json::from_value::<ProgrammingGroupValue>(serde_json::to_value(&group).unwrap())
                .unwrap(),
            group
        );
    }

    #[test]
    fn projection_keeps_addresses_order_and_timing() {
        let value = ProgrammingFixtureValue {
            replacement_projection: None,
            fixture_id: Uuid::from_u128(1),
            attribute: "intensity".into(),
            value: ProgrammingAttributeValue::Normalized(0.5),
            programmer_order: 7,
            fade: true,
            fade_millis: Some(1_000),
            delay_millis: Some(250),
        };
        let json = serde_json::to_value(value).unwrap();
        assert_eq!(json["fixture_id"], Uuid::from_u128(1).to_string());
        assert_eq!(json["programmer_order"], 7);
        assert_eq!(json["fade"], true);
        assert_eq!(json["fade_millis"], 1_000);
        assert_eq!(json["delay_millis"], 250);
        assert_eq!(json["value"]["kind"], "normalized");
    }

    #[test]
    fn actions_accept_fields_outside_the_known_recordable_values_contract() {
        let value = serde_json::json!({
            "request_id": "request-1",
            "expected_revision": 0,
            "expected_capture_mode_revision": 0,
            "action": {
                "type": "set_fixture",
                "fixture_id": Uuid::from_u128(1),
                "attribute": "intensity",
                "value": {"kind": "normalized", "value": 0.5},
                "mode": "preload"
            }
        });
        assert!(serde_json::from_value::<ProgrammingValuesActionRequest>(value).is_ok());
    }

    #[test]
    fn action_requires_the_capture_mode_revision() {
        let value = serde_json::json!({
            "request_id": "request-1",
            "expected_revision": 0,
            "action": {"type": "clear"}
        });
        assert!(serde_json::from_value::<ProgrammingValuesActionRequest>(value).is_err());
    }

    #[test]
    fn capture_mode_projection_rejects_unknown_fields() {
        let value = serde_json::json!({
            "revision": 2,
            "blind": false,
            "preview": false,
            "preload_capture_programmer": true,
            "selection": []
        });
        assert!(serde_json::from_value::<ProgrammingCaptureModeProjection>(value).is_err());
    }

    #[test]
    fn no_change_outcome_does_not_serialize_a_projection_or_event_sequence() {
        let outcome = ProgrammingValuesActionOutcome {
            request_id: "request-2".into(),
            correlation_id: Uuid::from_u128(2),
            revision: 7,
            capture_mode_revision: 3,
            outcome: ProgrammingValuesActionState::NoChange,
            replayed: false,
            hold: None,
            color_adoption: None,
            warning: None,
        };
        let json = serde_json::to_value(outcome).unwrap();
        assert_eq!(json["status"], "no_change");
        assert!(json.get("projection").is_none());
        assert!(json.get("event_sequence").is_none());
    }
}
