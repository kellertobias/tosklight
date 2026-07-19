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
#[serde(deny_unknown_fields)]
pub struct PatchFixturesRequest {
    /// Client-generated idempotency identity, scoped to the authenticated desk session.
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    /// One non-empty candidate batch. The application service validates and applies it atomically.
    #[schemars(length(min = 1))]
    pub fixtures: Vec<PatchFixtureInput>,
}

/// One fixture candidate containing only identities and state owned by the portable patch.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
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
    pub location: PatchFixtureLocation,
    pub rotation: PatchFixtureRotation,
    pub multipatch: Vec<PatchMultiPatchInput>,
    pub move_in_black_enabled: bool,
    #[ts(type = "number")]
    pub move_in_black_delay_millis: u64,
    pub highlight_overrides: Vec<PatchHighlightOverrideInput>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PatchSplitAssignment {
    pub split: u16,
    pub universe: Option<u16>,
    pub address: Option<u16>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PatchDirectControlEndpoint {
    pub protocol: PatchDirectControlProtocol,
    /// Transport adapters validate this as an IP address before invoking the application service.
    pub ip_address: String,
    pub port: u16,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PatchDirectControlProtocol {
    Citp,
}

/// Stage position in integer millimetres.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PatchFixtureLocation {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// Stage rotation in degrees.
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PatchFixtureRotation {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PatchMultiPatchInput {
    pub id: Uuid,
    pub name: String,
    #[schemars(length(min = 1))]
    pub split_patches: Vec<PatchSplitAssignment>,
    pub location: PatchFixtureLocation,
    pub rotation: PatchFixtureRotation,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
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
    #[serde(flatten)]
    pub delta: PatchDelta,
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
    /// Sequence of the single semantic patch-change event produced by this transaction.
    #[ts(type = "number")]
    pub event_sequence: u64,
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
    pub location: PatchFixtureLocation,
    pub rotation: PatchFixtureRotation,
    pub logical_heads: Vec<PatchLogicalHeadProjection>,
    pub multipatch: Vec<PatchMultiPatchProjection>,
    pub move_in_black_enabled: bool,
    #[ts(type = "number")]
    pub move_in_black_delay_millis: u64,
    pub highlight_overrides: Vec<PatchHighlightOverrideProjection>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct PatchLogicalHeadProjection {
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
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct PatchHighlightOverrideProjection {
    pub channel_id: Uuid,
    pub raw_value: u32,
}

/// Deduplicated, Patch-only metadata for one immutable profile revision.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
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
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PatchProfilePolicy {
    Dmx,
    VisualOnly,
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

#[cfg(test)]
mod tests;
