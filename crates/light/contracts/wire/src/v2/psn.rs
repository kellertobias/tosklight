//! Request and response DTOs for the v2 PosiStageNet API.
//!
//! Two things travel here, and they are not the same kind of thing. The **configuration** is show
//! data an operator edits — which tracker is which 3D Point, where the zones are — and it is
//! written intent-style with only the fields being changed (api-rules §3). The **status** is what
//! is happening right now: who is transmitting, how old each position is, which zones are
//! occupied. It is read once when the tab opens and pushed thereafter, because a desk that polls
//! a 60 Hz source at 1 Hz shows an operator a number that is already wrong.
//!
//! Positions are in the show's own stage space, in metres, calibration applied — the same space
//! the Stage view draws. Nothing here carries the tracking system's raw coordinates: the operator
//! calibrated once, and everything downstream should agree about where the marker is.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

/// The stored tracking configuration, exactly as the show holds it.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PsnConfigurationProjection {
    pub enabled: bool,
    /// The multicast group the desk listens to, as dotted quad.
    pub group: String,
    pub port: u16,
    /// The network card to listen on, when the desk has more than one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub interface: Option<String>,
    #[ts(type = "number")]
    pub stale_after_millis: u64,
    pub calibration: PsnCalibrationProjection,
    pub bindings: Vec<PsnBindingProjection>,
    pub zones: Vec<PsnZoneProjection>,
}

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PsnCalibrationProjection {
    /// Where the tracking system's origin is in the show, in metres.
    pub offset_metres: [f32; 3],
    /// About the show's up axis, applied before the offset.
    pub rotation_degrees: f32,
    pub scale: f32,
}

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PsnBindingProjection {
    pub id: Uuid,
    pub tracker_id: u16,
    /// The 3D Point this tracker is. While the binding exists nothing else writes it.
    pub point_fixture_id: Uuid,
    pub enabled: bool,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PsnZoneProjection {
    pub id: Uuid,
    pub name: String,
    pub min_metres: [f32; 3],
    pub max_metres: [f32; 3],
    /// Empty means every tracker counts.
    pub tracker_ids: Vec<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub enter_macro_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub leave_macro_id: Option<Uuid>,
    /// How long a change has to hold before it counts, in milliseconds.
    #[ts(type = "number")]
    pub dwell_millis: u64,
}

/// What is arriving, at one moment.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PsnStatusProjection {
    pub enabled: bool,
    /// The group and port the desk is listening on, when it is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub listening_on: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub health: Option<PsnHealthProjection>,
    /// What the senders call themselves, once their info packets have said.
    pub system_names: Vec<String>,
    pub trackers: Vec<PsnTrackerProjection>,
    pub placements: Vec<PsnPlacementProjection>,
    pub occupied_zone_ids: Vec<Uuid>,
    #[ts(type = "number")]
    pub frames: u64,
    /// Datagrams on the group that were not PSN, or could not be read. A steady climb here with
    /// frames also arriving means something else is talking on the group.
    #[ts(type = "number")]
    pub ignored_datagrams: u64,
    /// Why the desk is not listening, when it should be but cannot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub error: Option<String>,
}

/// The source's condition in operator language.
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum PsnHealthProjection {
    /// Nothing has ever arrived. A sender that is switched off looks exactly like a desk on the
    /// wrong network, so this is stated rather than diagnosed.
    Silent,
    Receiving,
    Stale {
        #[ts(type = "number")]
        silent_for_millis: u64,
    },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PsnTrackerProjection {
    pub tracker_id: u16,
    /// What the sender calls it. A data packet carries only the number, so a source heard for less
    /// than a second has no name yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub name: Option<String>,
    /// Where it is in the show's stage space, in metres, calibration applied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub position_metres: Option<[f32; 3]>,
    #[ts(type = "number")]
    pub age_millis: u64,
    pub stale: bool,
    /// Which sender this came from, as address and port.
    pub source: String,
}

/// One binding, and where it actually put its point.
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PsnPlacementProjection {
    pub binding_id: Uuid,
    pub point_fixture_id: Uuid,
    pub position_metres: [f32; 3],
    /// The marker is further from where the point was patched than a 3D Point can reach, so the
    /// point stopped at the end of its travel.
    pub out_of_reach: bool,
}

/// A 3D Point a tracker can be bound to.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PsnPointProjection {
    pub fixture_id: Uuid,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable, type = "number | null")]
    pub fixture_number: Option<u32>,
}

/// A Macro a zone can run.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PsnMacroProjection {
    pub id: Uuid,
    pub number: u16,
    pub name: String,
}

/// Configuration, status, and what an operator can pick from — everything a tab that has just
/// been opened needs, in one read.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PsnSnapshot {
    #[ts(type = "number")]
    pub revision: u64,
    pub configuration: PsnConfigurationProjection,
    pub status: PsnStatusProjection,
    /// Every 3D Point in the show. The desk decides what counts as one, not the tab.
    pub points: Vec<PsnPointProjection>,
    /// Every Macro in the show, for a zone's enter and leave.
    pub macros: Vec<PsnMacroProjection>,
}

/// An edit carrying only what changed.
#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PsnUpdateRequest {
    /// Client-generated idempotency identity, scoped to the authenticated desk session.
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub port: Option<u16>,
    /// Present and null clears the interface; absent leaves it alone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub interface: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable, type = "number | null")]
    pub stale_after_millis: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub calibration: Option<PsnCalibrationProjection>,
    /// The whole binding list, when bindings are what changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub bindings: Option<Vec<PsnBindingProjection>>,
    /// The whole zone list, when zones are what changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub zones: Option<Vec<PsnZoneProjection>>,
}

/// What an accepted edit did.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PsnUpdateOutcome {
    pub request_id: String,
    #[ts(type = "number")]
    pub revision: u64,
    pub configuration: PsnConfigurationProjection,
    /// True when the edit asked for what was already stored.
    pub unchanged: bool,
    pub replayed: bool,
}

/// A refusal, in the words the operator used to enter it.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PsnErrorResponse {
    pub error: String,
}
