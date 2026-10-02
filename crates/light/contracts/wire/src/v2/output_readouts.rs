//! Typed readouts of one accepted output source, and the displayed-source lease a surface uses
//! to name exactly the source it showed when the operator starts an edit (TL-594).
//!
//! A readout snapshot is built from ONE accepted source: the frame identity, the requested
//! intent and every commanded copy/emitter pair come from the same frame, so a consumer reads
//! one coherent captured token. `lease` is opaque and session-scoped: it is never the hub
//! sequence and it names only a source the server actually delivered to that session.

use super::output_control::OutputFrameIdentity;
use super::preload_values::ProgrammingPreloadAttributeValue;
use super::visualization::{VisualizationLane, VisualizationScope};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

/// The displayed source an edit binds its first-edit adoption to. Edits that omit it (OSC,
/// HTTP integrators) keep the latest-accepted adoption; edits that carry it never fall back.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct DisplayedSourceRef {
    pub lane: VisualizationLane,
    #[ts(type = "number")]
    pub lease: u64,
}

/// Why a readout snapshot carries no accepted source. The surface shows "no readout" and
/// re-reads; it never substitutes another lane or a fresh evaluation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum OutputReadoutUnavailable {
    /// No accepted Live frame has been published yet (or output is held).
    NoAcceptedFrame,
    /// No accepted Pending (Preload) publication exists for this session's Programmer.
    NoAcceptedPreload,
    /// The accepted source belongs to another show than the active one.
    ShowChanged,
    /// The engine no longer runs the source's patch generation/layout.
    StaleGeneration,
}

/// Why an edit that named a displayed source was held quietly. The surface re-reads its
/// readout and starts the next gesture from the fresh lease; nothing was mutated.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProgrammingValuesHoldReason {
    DisplayedSourceUnavailable,
    /// TL-554: a Direct (native) edit has no verified reference head, original model or
    /// published premaster output to adopt. Nothing changed; the pages show why passively.
    NativeColorUnavailable,
    /// TL-554: the first semantic edit of a Direct value whose visible appearance is unknown.
    /// Resend with `explicit_color_start`; no white is ever invented.
    ExplicitColorStartRequired,
    /// TL-637 follow-up: a Zoom edit has no seed in degrees. Nothing is authored and the
    /// displayed output's opening is not measurable in a known convention (unknown or
    /// unsupported optics model). The surface shows its quiet unsupported state.
    ZoomUnavailable,
}

/// One commanded joint pair of one actual emitter of one physical copy (root or multipatch).
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct OutputPositionCommandReadout {
    pub destination: Uuid,
    pub emitter_id: Uuid,
    pub pan_degrees: f64,
    pub tilt_degrees: f64,
}

/// Common authoring Angles, present only when every copy/emitter agrees exactly.
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct OutputCommonAngles {
    pub pan_degrees: f32,
    pub tilt_degrees: f32,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct OutputPositionReadout {
    /// False when the owner has no complete commanded pose in this source.
    pub available: bool,
    /// Every copy/emitter pair, root first; divergent copies stay distinct rows.
    pub commands: Vec<OutputPositionCommandReadout>,
    /// `None` on divergence or unavailability. Never an average or the root's pair.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub common: Option<OutputCommonAngles>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct OutputOwnerReadout {
    pub fixture_id: Uuid,
    /// The typed requested Position intent of the same frame. Presentation is client-side.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub requested: Option<ProgrammingPreloadAttributeValue>,
    pub position: OutputPositionReadout,
}

/// Whole-snapshot readout of one accepted source for the requested owners, in caller order
/// with duplicates kept.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct OutputReadoutSnapshot {
    pub lane: VisualizationLane,
    pub scope: VisualizationScope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub frame: Option<OutputFrameIdentity>,
    /// Opaque, session-scoped lease of exactly this source. Absent when unavailable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable, type = "number | null")]
    pub lease: Option<u64>,
    /// Show revision of the source.
    #[ts(type = "number")]
    pub revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub unavailable: Option<OutputReadoutUnavailable>,
    pub owners: Vec<OutputOwnerReadout>,
}

/// The accepted Pending identity a Preload lane message was built from (TL-594): the
/// publication status with its episode, and the published frame whose `sequence` is the gate
/// ticket. Present only while the family-adapter gate is on; never a Live stamp.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct VisualizationPendingStamp {
    pub status: super::output_control::OutputPreloadStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub frame: Option<OutputFrameIdentity>,
}

/// WebSocket readout claim. A new Subscribe replaces it; omitting it clears it.
#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct VisualizationReadoutClaim {
    #[schemars(length(max = 512))]
    pub fixture_ids: Vec<Uuid>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readout_snapshot_round_trips_and_tolerates_unknown_fields() {
        let snapshot = OutputReadoutSnapshot {
            lane: VisualizationLane::Normal,
            scope: VisualizationScope { show_id: None },
            frame: None,
            lease: Some(7),
            revision: 3,
            unavailable: None,
            owners: vec![OutputOwnerReadout {
                fixture_id: Uuid::nil(),
                requested: None,
                position: OutputPositionReadout {
                    available: true,
                    commands: vec![OutputPositionCommandReadout {
                        destination: Uuid::nil(),
                        emitter_id: Uuid::nil(),
                        pan_degrees: 10.0,
                        tilt_degrees: 20.0,
                    }],
                    common: Some(OutputCommonAngles {
                        pan_degrees: 10.0,
                        tilt_degrees: 20.0,
                    }),
                },
            }],
        };
        let mut json = serde_json::to_value(&snapshot).unwrap();
        json["future"] = serde_json::json!(true);
        json["owners"][0]["future"] = serde_json::json!(1);
        let decoded: OutputReadoutSnapshot = serde_json::from_value(json).unwrap();
        assert_eq!(decoded, snapshot);
    }

    #[test]
    fn displayed_source_ref_is_lane_and_lease() {
        let value: DisplayedSourceRef =
            serde_json::from_value(serde_json::json!({"lane":"preload","lease":9,"x":0})).unwrap();
        assert_eq!(
            value,
            DisplayedSourceRef {
                lane: VisualizationLane::Preload,
                lease: 9
            }
        );
    }
}
