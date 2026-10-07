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
    /// Optional for older receivers. Observational ingress metadata, never an output-frame
    /// identity to join onto independently retained DMX, Point poses or physical values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub sources: Option<Vec<PsnSourceProjection>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub diagnostics: Option<PsnReceiverDiagnosticsProjection>,
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

/// Identity of a frame accepted by one receiver source resource. Both epochs are needed:
/// rebind/show activation replaces the resource; an explicit sender restart replaces its epoch.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PsnAcceptedSampleProjection {
    pub source: String,
    #[ts(type = "number")]
    pub source_generation: u64,
    #[ts(type = "number")]
    pub source_epoch: u64,
    #[ts(type = "number")]
    pub sequence: u64,
    pub frame_id: u8,
    /// Sender-relative send time, not a synchronized wall-clock time or freshness clock.
    #[ts(type = "number")]
    pub sender_timestamp_micros: u64,
    /// Receiver monotonic time when all accepted parts were available. A particular tracker
    /// may have arrived in an earlier part: retain its independent age_millis for freshness.
    #[ts(type = "number")]
    pub accepted_at_millis: u64,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(default)]
pub struct PsnIngressDiagnosticsProjection {
    #[ts(type = "number")]
    pub duplicate_datagrams: u64,
    #[ts(type = "number")]
    pub rejected_datagrams: u64,
    #[ts(type = "number")]
    pub incomplete_frames: u64,
    #[ts(type = "number")]
    pub ambiguous_datagrams: u64,
    #[ts(type = "number")]
    pub invalid_positions: u64,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PsnSourceProjection {
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub accepted_sample: Option<PsnAcceptedSampleProjection>,
    #[serde(default)]
    pub diagnostics: PsnIngressDiagnosticsProjection,
}

/// Passive receiver limits/data quality, separate from actionable socket/configuration errors.
#[derive(Clone, Copy, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(default)]
pub struct PsnReceiverDiagnosticsProjection {
    #[ts(type = "number")]
    pub source_count: u64,
    #[ts(type = "number")]
    pub source_capacity: u64,
    /// New-source datagrams dropped at capacity; already-held sources are never evicted.
    #[ts(type = "number")]
    pub rejected_source_datagrams: u64,
    /// Current raw finite positions whose calibration could not produce finite show coordinates.
    #[ts(type = "number")]
    pub invalid_calibrated_positions: u64,
    /// Stored rows withheld because their binding UUID is repeated, including disabled rows.
    /// Independent binding identities continue to track; the stored configuration is preserved.
    #[ts(type = "number")]
    pub conflicting_binding_rows: u64,
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
    /// Last finite positional sample; may be older than the source's latest accepted frame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub accepted_sample: Option<PsnAcceptedSampleProjection>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn older_status_and_tracker_payloads_remain_readable_without_provenance() {
        let status: PsnStatusProjection = serde_json::from_value(serde_json::json!({
            "enabled": true,
            "system_names": [],
            "trackers": [{ "tracker_id": 1, "age_millis": 30, "stale": false, "source": "10.0.0.1:56565" }],
            "placements": [], "occupied_zone_ids": [], "frames": 1, "ignored_datagrams": 0
        })).unwrap();
        assert!(status.sources.is_none());
        assert!(status.diagnostics.is_none());
        assert!(status.trackers[0].accepted_sample.is_none());
        let encoded = serde_json::to_value(status).unwrap();
        assert!(encoded.get("sources").is_none());
        assert!(encoded.get("diagnostics").is_none());
        assert!(encoded["trackers"][0].get("accepted_sample").is_none());
    }

    #[test]
    fn partial_diagnostics_default_and_future_fields_are_tolerated() {
        let source: PsnSourceProjection = serde_json::from_value(serde_json::json!({
            "source": "10.0.0.1:56565", "future": true,
            "diagnostics": { "duplicate_datagrams": 4, "future": 9 }
        }))
        .unwrap();
        assert_eq!(source.diagnostics.duplicate_datagrams, 4);
        assert_eq!(source.diagnostics.rejected_datagrams, 0);
        assert!(source.accepted_sample.is_none());
        let receiver: PsnReceiverDiagnosticsProjection =
            serde_json::from_value(serde_json::json!({ "source_count": 2 })).unwrap();
        assert_eq!(receiver.source_count, 2);
        assert_eq!(receiver.rejected_source_datagrams, 0);
        assert_eq!(receiver.conflicting_binding_rows, 0);
    }

    #[test]
    fn provenance_keeps_sender_and_receiver_clocks_distinct_and_uses_ts_numbers() {
        let sample = PsnAcceptedSampleProjection {
            source: "10.0.0.1:56565".into(),
            source_generation: 7,
            source_epoch: 2,
            sequence: 5_000_000_000,
            frame_id: 255,
            sender_timestamp_micros: 6_000_000_000,
            accepted_at_millis: 110,
        };
        let roundtrip: PsnAcceptedSampleProjection =
            serde_json::from_value(serde_json::to_value(&sample).unwrap()).unwrap();
        assert_eq!(roundtrip, sample);
        let config = ts_rs::Config::default();
        for declaration in [
            PsnAcceptedSampleProjection::decl(&config),
            PsnIngressDiagnosticsProjection::decl(&config),
            PsnReceiverDiagnosticsProjection::decl(&config),
        ] {
            assert!(!declaration.contains("bigint"));
        }
    }
}
