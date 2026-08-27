//! Stable request, outcome, snapshot, and delta DTOs for the v2 show-patch API.
//!
//! Patch mutations carry only portable, patch-owned state and immutable fixture-profile revision
//! references. Fixture definitions and fixture-library catalog records deliberately do not cross
//! this command boundary.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use super::events::EventSnapshotCursor;

/// Body of the atomic, idempotent `PatchFixtures` POST operation.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PatchFixturesRequest {
    /// Client-generated idempotency identity, scoped to the authenticated desk session.
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    /// Fixture upserts. The application service requires at least one upsert or removal.
    #[serde(default)]
    pub fixtures: Vec<PatchFixtureInput>,
    /// Stable fixture identities removed by the same atomic operation. Already-absent identities
    /// are accepted as the requested desired state.
    #[serde(default)]
    pub remove_fixture_ids: Vec<Uuid>,
    /// Server-resolved placement intents. Empty retains the generic desired-state Patch behavior
    /// where fixture split assignments are already explicit.
    #[serde(default)]
    pub placements: Vec<PatchPlacementIntent>,
    /// Server-resolved coordinate spreads over an explicitly ordered fixture selection.
    #[serde(default)]
    pub vector_spreads: Vec<PatchVectorSpreadIntent>,
}

/// One ordered, server-resolved spread for a patch-owned location or rotation axis.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PatchVectorSpreadIntent {
    #[schemars(length(min = 1))]
    pub fixture_ids: Vec<Uuid>,
    pub kind: PatchVectorKind,
    pub axis: PatchVectorAxis,
    #[schemars(length(min = 2))]
    pub points: Vec<f32>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PatchVectorKind {
    Location,
    Rotation,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PatchVectorAxis {
    X,
    Y,
    Z,
}

/// One sparse operator intent for a patch-owned output policy.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PatchFixturePolicyActionRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    #[serde(flatten)]
    pub action: PatchFixturePolicyAction,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum PatchFixturePolicyAction {
    SetGroupMasters {
        controlled: bool,
    },
    SetGrandMaster {
        controlled: bool,
    },
    SetAxisInversion {
        axis: PatchFixtureAxis,
        inverted: bool,
        /// Absent targets the root physical fixture; present targets one multi-patch instance.
        multipatch_instance_id: Option<Uuid>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PatchFixtureAxis {
    Pan,
    Tilt,
}

/// One typed, replay-safe sparse edit of an existing physical fixture instance.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PatchFixtureUpdateRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    #[ts(type = "number")]
    pub expected_fixture_revision: u64,
    #[ts(type = "number")]
    pub expected_patch_revision: u64,
    #[ts(type = "number")]
    pub expected_show_revision: u64,
    /// Absent targets the root physical fixture; present targets exactly one multi-patch copy.
    pub multipatch_instance_id: Option<Uuid>,
    #[serde(flatten)]
    pub action: PatchFixtureUpdateAction,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum PatchFixtureUpdateAction {
    SetMasters {
        group_masters_enabled: bool,
        grand_master_enabled: bool,
    },
    SetPanTilt {
        invert_pan: bool,
        invert_tilt: bool,
    },
    SetMoveInBlack {
        enabled: bool,
        #[ts(type = "number")]
        delay_millis: u64,
    },
    SetLocationAxis {
        axis: PatchVectorAxis,
        millimetres: i32,
    },
    SetRotationAxis {
        axis: PatchVectorAxis,
        degrees: f32,
    },
    SetBracketAngle {
        degrees: f32,
    },
    SetShaperModuleRotation {
        degrees: Option<f32>,
    },
    SetStaticShaperAngle {
        element: u8,
        degrees: f32,
    },
    SetInstalledAppearance {
        appearance: PatchInstalledFixtureAppearance,
    },
}

/// Portable installed lamp/filter/static-shaper appearance for one physical fixture instance.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PatchInstalledFixtureAppearance {
    #[serde(default)]
    pub light_source: PatchInstalledLightSource,
    #[serde(default)]
    pub color_temperature_kelvin: Option<u32>,
    #[serde(default)]
    pub luminous_output_lumens: Option<f32>,
    #[serde(default)]
    pub gel: PatchGelAssignment,
    #[serde(default)]
    #[ts(type = "[number, number, number, number]")]
    pub shaper_angles_degrees: [f32; 4],
}

impl Default for PatchInstalledFixtureAppearance {
    fn default() -> Self {
        Self {
            light_source: PatchInstalledLightSource::ProfileDefault,
            color_temperature_kelvin: None,
            luminous_output_lumens: None,
            gel: PatchGelAssignment::OpenWhite,
            shaper_angles_degrees: [0.0; 4],
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PatchInstalledLightSource {
    #[default]
    ProfileDefault,
    Tungsten,
    Halogen,
    Discharge,
    Led,
    Fluorescent,
    Arc,
    Other {
        label: String,
    },
}

#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PatchGelAssignment {
    #[default]
    OpenWhite,
    BuiltIn {
        catalog_id: String,
        entry_id: String,
        embedded_fallback: PatchGelDefinitionSnapshot,
    },
    Custom {
        name: String,
        color_srgb: String,
        note: Option<String>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct PatchGelDefinitionSnapshot {
    pub number: String,
    pub name: String,
    pub display_srgb: String,
    pub visualizer_srgb: String,
}

/// One ordered fixture batch whose per-split addresses are resolved by the server.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PatchPlacementIntent {
    #[schemars(length(min = 1))]
    pub fixture_ids: Vec<Uuid>,
    #[schemars(length(min = 1))]
    pub splits: Vec<PatchSplitPlacementIntent>,
}

/// Base address and deterministic assignment mode for one selected-mode split.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PatchSplitPlacementIntent {
    pub split: u16,
    pub universe: Option<u16>,
    pub address: Option<u16>,
    pub mode: PatchSplitPlacementMode,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PatchSplitPlacementMode {
    Consecutive,
    OperatorOverrides {
        #[serde(default)]
        overrides: Vec<PatchOperatorAddressOverride>,
    },
}

/// Sparse operator-selected address replacing the deterministic proposal for one fixture.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct PatchOperatorAddressOverride {
    pub fixture_id: Uuid,
    pub universe: u16,
    pub address: u16,
}

/// One fixture candidate containing only identities and state owned by the portable patch.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PatchFixtureInput {
    /// Stable identity generated once by the caller and retained across an idempotent retry.
    pub fixture_id: Uuid,
    pub fixture_number: Option<u32>,
    pub virtual_fixture_number: Option<u32>,
    pub name: String,
    pub profile_id: Uuid,
    #[ts(type = "number")]
    pub profile_revision: u64,
    pub mode_id: Uuid,
    /// Canonical split assignments. An unpatched split has two `null` address fields.
    #[schemars(length(min = 1))]
    pub split_patches: Vec<PatchSplitAssignment>,
    pub layer_id: String,
    pub direct_control: Option<PatchDirectControlEndpoint>,
    #[serde(default)]
    pub internal_bindings: PatchInternalFixtureBindings,
    pub location: PatchFixtureLocation,
    pub rotation: PatchFixtureRotation,
    /// The 3D Point this fixture is slaved to. Omitted by a client that does not use points, and
    /// by every request written before they existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position_master: Option<Uuid>,
    pub multipatch: Vec<PatchMultiPatchInput>,
    #[serde(default = "default_true")]
    pub group_masters_enabled: bool,
    #[serde(default = "default_true")]
    pub grand_master_enabled: bool,
    #[serde(default)]
    pub invert_pan: bool,
    #[serde(default)]
    pub invert_tilt: bool,
    /// Degrees the mounting bracket is set to, positive nose-down. A mechanical setting the desk
    /// cannot drive, recorded so the visualizer draws the rig as it actually hangs.
    #[serde(default)]
    pub bracket_angle: f32,
    /// Degrees a fitted shaper or barn-door module is turned to, or absent when none is fitted.
    #[serde(default)]
    pub shaper_angle: Option<f32>,
    #[serde(default)]
    pub installed_appearance: PatchInstalledFixtureAppearance,
    pub move_in_black_enabled: bool,
    #[ts(type = "number")]
    pub move_in_black_delay_millis: u64,
    pub highlight_overrides: Vec<PatchHighlightOverrideInput>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct PatchSplitAssignment {
    pub split: u16,
    pub universe: Option<u16>,
    pub address: Option<u16>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct PatchDirectControlEndpoint {
    pub protocol: PatchDirectControlProtocol,
    /// Transport adapters validate this as an IP address before invoking the application service.
    pub ip_address: String,
    pub port: u16,
}

/// Portable names resolved to machine-local audio resources by each desk.
#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct PatchInternalFixtureBindings {
    pub library: Option<String>,
    pub output: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PatchDirectControlProtocol {
    Citp,
}

/// Stage position in integer millimetres.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct PatchFixtureLocation {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// Stage rotation in degrees.
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PatchFixtureRotation {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PatchMultiPatchInput {
    pub id: Uuid,
    pub name: String,
    #[schemars(length(min = 1))]
    pub split_patches: Vec<PatchSplitAssignment>,
    pub location: PatchFixtureLocation,
    pub rotation: PatchFixtureRotation,
    #[serde(default)]
    pub invert_pan: bool,
    #[serde(default)]
    pub invert_tilt: bool,
    #[serde(default)]
    pub bracket_angle: f32,
    #[serde(default)]
    pub shaper_angle: Option<f32>,
    #[serde(default)]
    pub installed_appearance: PatchInstalledFixtureAppearance,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct PatchHighlightOverrideInput {
    pub channel_id: Uuid,
    pub raw_value: u32,
}

/// Successful result of an atomic `PatchFixtures` command.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PatchFixturesOutcome {
    pub request_id: String,
    /// `true` when idempotency replay returned the already committed authoritative result.
    pub replayed: bool,
    /// `false` when the requested desired state was already authoritative and emitted no event.
    pub changed: bool,
    #[serde(flatten)]
    pub delta: PatchDelta,
}

/// Transport error for revisioned Patch operations.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct PatchErrorResponse {
    pub error: String,
    #[ts(as = "Option<f64>", optional = nullable)]
    pub current_revision: Option<u64>,
    pub retryable: bool,
}

/// Authoritative current Patch projection used for initial load and sequence-gap repair.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PatchSnapshot {
    pub show_id: Uuid,
    #[ts(type = "number")]
    pub show_revision: u64,
    #[ts(type = "number")]
    pub patch_revision: u64,
    pub cursor: EventSnapshotCursor,
    pub fixtures: Vec<PatchFixtureProjection>,
    /// Exactly one entry per profile revision referenced by `fixtures`.
    pub profile_revisions: Vec<PatchProfileRevisionProjection>,
}

/// Targeted post-mutation projection suitable for a Patch store and a typed patch-change event.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PatchDelta {
    pub show_id: Uuid,
    #[ts(type = "number")]
    pub show_revision: u64,
    #[ts(type = "number")]
    pub patch_revision: u64,
    /// Sequence of the semantic patch-change event, absent for a no-op desired-state request.
    #[ts(as = "Option<f64>", optional = nullable)]
    pub event_sequence: Option<u64>,
    pub fixtures: Vec<PatchFixtureProjection>,
    pub removed_fixture_ids: Vec<Uuid>,
    /// Unique metadata needed to interpret the fixture projections in this delta.
    pub profile_revisions: Vec<PatchProfileRevisionProjection>,
}

/// Portable, authoritative Patch representation of one fixture without an inline definition.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PatchFixtureProjection {
    pub fixture_id: Uuid,
    #[ts(type = "number")]
    pub fixture_revision: u64,
    pub fixture_number: Option<u32>,
    pub virtual_fixture_number: Option<u32>,
    pub name: String,
    pub profile_id: Uuid,
    #[ts(type = "number")]
    pub profile_revision: u64,
    pub mode_id: Uuid,
    pub split_patches: Vec<PatchSplitAssignment>,
    pub layer_id: String,
    pub direct_control: Option<PatchDirectControlEndpoint>,
    #[serde(default)]
    pub internal_bindings: PatchInternalFixtureBindings,
    pub location: PatchFixtureLocation,
    pub rotation: PatchFixtureRotation,
    pub logical_heads: Vec<PatchLogicalHeadProjection>,
    pub multipatch: Vec<PatchMultiPatchProjection>,
    pub group_masters_enabled: bool,
    pub grand_master_enabled: bool,
    pub invert_pan: bool,
    pub invert_tilt: bool,
    pub bracket_angle: f32,
    pub shaper_angle: Option<f32>,
    pub installed_appearance: PatchInstalledFixtureAppearance,
    pub move_in_black_enabled: bool,
    #[ts(type = "number")]
    pub move_in_black_delay_millis: u64,
    pub highlight_overrides: Vec<PatchHighlightOverrideProjection>,
    #[serde(default)]
    pub freeze_targets: Vec<PatchFixtureFreezeTargetProjection>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum PatchFixtureFreezeFamily {
    Intensity,
    Color,
    Position,
    Beam,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct PatchFixtureFreezeTargetProjection {
    pub fixture_id: Uuid,
    pub full: bool,
    pub families: Vec<PatchFixtureFreezeFamily>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct PatchLogicalHeadProjection {
    /// Stable semantic head identity from the selected immutable profile revision.
    pub profile_head_id: Option<Uuid>,
    pub head_index: u16,
    pub fixture_id: Uuid,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PatchMultiPatchProjection {
    pub id: Uuid,
    pub name: String,
    pub split_patches: Vec<PatchSplitAssignment>,
    pub location: PatchFixtureLocation,
    pub rotation: PatchFixtureRotation,
    pub invert_pan: bool,
    pub invert_tilt: bool,
    pub bracket_angle: f32,
    pub shaper_angle: Option<f32>,
    pub installed_appearance: PatchInstalledFixtureAppearance,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct PatchHighlightOverrideProjection {
    pub channel_id: Uuid,
    pub raw_value: u32,
}

/// Deduplicated, Patch-only metadata for one immutable profile revision.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PatchProfileRevisionProjection {
    pub profile_id: Uuid,
    #[ts(type = "number")]
    pub profile_revision: u64,
    pub content_digest: String,
    pub manufacturer: String,
    pub name: String,
    pub fixture_type: String,
    pub patch_policy: PatchProfilePolicy,
    /// Only modes referenced by fixtures in the containing snapshot or delta, never the catalog.
    #[schemars(length(min = 1))]
    pub referenced_modes: Vec<PatchModeProjection>,
    /// Server-resolved parameterized profile snapshot for this revision. Patch programmer-surface
    /// consumers build head parameters, channels, and control actions from it client-side without
    /// the fixture catalog. Carried server->client only; the patch *request* boundary still
    /// excludes fixture definitions. Absent (null) on older payloads, so clients must tolerate it.
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    #[ts(type = "unknown")]
    pub profile_snapshot: serde_json::Value,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PatchProfilePolicy {
    Dmx,
    VisualOnly,
    Internal,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct PatchModeProjection {
    pub mode_id: Uuid,
    pub name: String,
    pub splits: Vec<PatchModeSplitProjection>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct PatchModeSplitProjection {
    pub split: u16,
    pub footprint: u16,
}

const fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests;
