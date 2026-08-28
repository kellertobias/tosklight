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

use light_core::{AttributeKey, AttributeValue, FixtureId};

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
