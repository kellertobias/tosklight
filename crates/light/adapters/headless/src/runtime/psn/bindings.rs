//! Turning a marker's position into the value a 3D Point actually holds.
//!
//! A 3D Point is patched somewhere and then *offset* from there, and the offset is what the show
//! stores as `point.position.x/y/z`. So placing a point at a world position is arithmetic against
//! where it was patched, and the axis range has to be the one the resolved value is read back
//! with — otherwise the point would sit at a different place than the number says.
//!
//! Nothing here knows about sockets or freshness. It is given the last position the receiver is
//! willing to stand behind and produces what the engine should hold; a source that has gone quiet
//! reaches this with the same position as before, which is how "hold the last point" happens
//! without anything special being done about it.

use light_core::{AttributeKey, AttributeValue, FixtureId};
use light_engine::TrackedOverride;
use std::collections::HashMap;
use uuid::Uuid;

use super::config::{POINT_AXIS_METRES, PsnConfiguration};

/// The axis attributes of a 3D Point, in the order the coordinates arrive.
pub(in crate::runtime) const POINT_AXIS_ATTRIBUTES: [&str; 3] =
    ["point.position.x", "point.position.y", "point.position.z"];

/// What one binding is doing right now, for the operator to read.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::runtime) struct BindingPlacement {
    pub binding_id: Uuid,
    pub point_fixture_id: Uuid,
    /// Where the point was put, in show metres.
    pub position_metres: [f32; 3],
    /// True when the marker is further from the point's patched position than a 3D Point can
    /// reach, so the point stopped at the end of its travel rather than following.
    pub out_of_reach: bool,
}

/// Everything the engine should hold, and what to tell the operator about it.
///
/// A binding whose 3D Point is no longer in the show is skipped rather than refused: deleting a
/// point is an ordinary edit, and the desk keeps tracking whatever is still there.
pub(in crate::runtime) fn placements(
    configuration: &PsnConfiguration,
    held_positions: &HashMap<Uuid, [f32; 3]>,
    point_locations: &HashMap<Uuid, [f32; 3]>,
) -> (Vec<TrackedOverride>, Vec<BindingPlacement>) {
    let mut overrides = Vec::new();
    let mut placements = Vec::new();
    for binding in configuration.active_bindings() {
        let Some(target) = held_positions.get(&binding.id) else {
            continue;
        };
        let Some(patched) = point_locations.get(&binding.point_fixture_id) else {
            continue;
        };
        let fixture_id = FixtureId(binding.point_fixture_id);
        let mut out_of_reach = false;
        let mut reached = [0.0_f32; 3];
        for (axis, attribute) in POINT_AXIS_ATTRIBUTES.iter().enumerate() {
            let offset = target[axis] - patched[axis];
            let clamped = offset.clamp(-POINT_AXIS_METRES, POINT_AXIS_METRES);
            out_of_reach |= (clamped - offset).abs() > f32::EPSILON;
            reached[axis] = patched[axis] + clamped;
            overrides.push(TrackedOverride::new(
                fixture_id,
                AttributeKey((*attribute).into()),
                AttributeValue::Normalized(normalized_axis(clamped)),
            ));
        }
        placements.push(BindingPlacement {
            binding_id: binding.id,
            point_fixture_id: binding.point_fixture_id,
            position_metres: reached,
            out_of_reach,
        });
    }
    (overrides, placements)
}

/// A 3D Point axis offset in metres, as the normalized value the show stores.
#[must_use]
pub(in crate::runtime) fn normalized_axis(metres: f32) -> f32 {
    ((metres + POINT_AXIS_METRES) / (2.0 * POINT_AXIS_METRES)).clamp(0.0, 1.0)
}

#[cfg(test)]
#[path = "bindings_tests.rs"]
mod tests;
