//! What an external tracking system owns outright.
//!
//! A tracking system — OpenFollow over PosiStageNet, or anything else a receiver can turn into
//! coordinates — does not program the desk. It says where something on stage is, and the operator
//! decides which 3D Point that is. From then on the point *is* the marker: the desk holds the
//! attributes that place it, and no cue, group, or programmer value writes them.
//!
//! That is why this is an override rather than another contribution. A contribution can be
//! outbid, and a follow-spot that a cue can pull off the performer is worse than no follow-spot
//! at all. Taking a point back is unbinding it, which is an act the operator can see and undo,
//! not a value that quietly wins one frame and loses the next.
//!
//! Held here rather than in the show because it changes 60 times a second and means nothing when
//! the desk restarts: what is stored is the *binding*, and the receiver reinstates the position
//! as soon as a packet arrives. When packets stop, whatever was last written stays written —
//! the engine is not told that the source went quiet, because holding is exactly what an
//! unchanged override already does.

use std::sync::Arc;

use light_core::{AttributeKey, AttributeValue, FixtureId, ShowId};
use uuid::Uuid;

/// Receiver-owned inputs captured atomically with an output frame. World coordinates remain in
/// world coordinates here: the captured patch generation supplies their Point origins later.
/// None of this data belongs to the portable show or depends on a particular tracking protocol.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TrackedInputFrame {
    pub show_id: Option<ShowId>,
    pub configuration_generation: u64,
    /// Identifies the receiver's Point membership view; never used as the origin for rendering.
    pub point_generation: u64,
    pub source_generation: u64,
    pub accepted_sequence: u64,
    pub sampled_at_millis: u64,
    pub legacy_overrides: Arc<[TrackedOverride]>,
    pub points: Arc<[TrackedPointInput]>,
}

/// Identity of the finite position sample actually held, which can be older than the receiver's
/// most recently accepted metadata packet. Receiver epochs distinguish explicit reconnects.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrackedSampleIdentity {
    pub source_id: Arc<str>,
    pub source_generation: u64,
    pub source_epoch: u64,
    pub sequence: u64,
    pub sender_timestamp_micros: u64,
    pub accepted_at_millis: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrackedPointInput {
    pub binding_id: Uuid,
    pub fixture_id: FixtureId,
    /// Calibrated world position before the Point's finite offset range is applied.
    pub position_metres: [f32; 3],
    pub sample: TrackedSampleIdentity,
    pub position_received_at_millis: u64,
}

impl TrackedPointInput {
    /// Encode this world position using the origin in the output frame's captured generation.
    /// A deleted Point has no origin and is skipped by the caller. Non-finite input never enters
    /// the output. A finite target outside the Point's range stops at its nearest axis limit.
    pub fn normalized_position(&self, origin_metres: [f32; 3]) -> Option<[f32; 3]> {
        if !self
            .position_metres
            .iter()
            .chain(origin_metres.iter())
            .all(|value| value.is_finite())
        {
            return None;
        }
        Some(std::array::from_fn(|axis| {
            let extent = crate::POINT_AXIS_METRES;
            let offset = (self.position_metres[axis] - origin_metres[axis]).clamp(-extent, extent);
            ((offset + extent) / (2.0 * extent)).clamp(0.0, 1.0)
        }))
    }

    pub fn position_age_millis(&self, sampled_at_millis: u64) -> u64 {
        sampled_at_millis.saturating_sub(self.position_received_at_millis)
    }
}

/// One attribute an external tracking source holds while its binding exists.
#[derive(Clone, Debug, PartialEq)]
pub struct TrackedOverride {
    pub fixture_id: FixtureId,
    pub attribute: AttributeKey,
    pub value: AttributeValue,
}

impl TrackedOverride {
    #[must_use]
    pub fn new(fixture_id: FixtureId, attribute: AttributeKey, value: AttributeValue) -> Self {
        Self {
            fixture_id,
            attribute,
            value,
        }
    }
}
