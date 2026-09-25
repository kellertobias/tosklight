//! When a desk-output read is a new frame for the Stage, and when it is the same picture again.

use viz_scene::SceneValues;

/// What a desk-output read would put on screen, as one number: every universe's slots, every 3D
/// Point's pose, the preload overlay while it is followed, and the scene the slots are decoded
/// through.
pub(crate) fn desk_output_signature(
    output: &crate::wire::OutputDmxSnapshot,
    preload: Option<&crate::wire::PreloadProjection>,
    scene_revision: u64,
) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::hash::DefaultHasher::new();
    scene_revision.hash(&mut hasher);
    output.universes.len().hash(&mut hasher);
    for universe in &output.universes {
        universe.universe.hash(&mut hasher);
        universe.slots.hash(&mut hasher);
    }
    // A 3D Point moved from the encoders changes the picture without changing a single slot, so
    // its pose is part of what makes a read a new frame.
    output.points.len().hash(&mut hasher);
    for point in &output.points {
        point.fixture_id.hash(&mut hasher);
        for value in point.offset_metres.iter().chain(&point.rotation_degrees) {
            value.to_bits().hash(&mut hasher);
        }
    }
    if let Some(preload) = preload {
        preload.fixture_values.len().hash(&mut hasher);
        for entry in &preload.fixture_values {
            entry.fixture_id.hash(&mut hasher);
            entry.attribute.hash(&mut hasher);
            match entry.value {
                crate::wire::PreloadAttributeValue::Normalized(value) => {
                    value.to_bits().hash(&mut hasher)
                }
                crate::wire::PreloadAttributeValue::Other => u32::MAX.hash(&mut hasher),
            }
        }
    }
    hasher.finish()
}

pub(crate) fn stamp_desk_output_frame(values: &mut SceneValues, value_frame: &mut u64, now: u64) {
    *value_frame = value_frame.saturating_add(1);
    values.newest_input_micros = now;
    values.frame = *value_frame;
}
