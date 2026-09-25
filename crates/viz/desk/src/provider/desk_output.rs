//! The desk's own output, for a renderer drawing the Stage inside the desk's window.
//!
//! Universes are decoded through exactly the path a real packet takes, and the desk's word on
//! where its 3D Points are is laid over the result, so a point with no address still moves what
//! is hung on it.

use super::{DeskProvider, ProviderEvent, SceneValues};
use crate::desk_output_frame::{desk_output_signature, stamp_desk_output_frame};

impl DeskProvider {
    /// Fold the desk's own output into the values, and present it when it changed the picture.
    pub(super) fn apply_desk_output(&mut self, events: &mut Vec<ProviderEvent>) {
        // The desk's own output, for a renderer drawing inside the desk's window. Decoded through
        // exactly the path a real packet takes, so nothing downstream can tell the difference —
        // the numbers are the same numbers, read from the desk instead of heard from the wire.
        // Applied on every read rather than when a revision moves. The desk's output revision
        // counts structural changes, not frames: it sits still while every level in the show is
        // moving, so gating on it showed the rig as it was at the moment the pane opened and never
        // again.
        if let (Some(output), Some(decoder), Some(scene)) =
            (self.desk_output.take(), &mut self.decoder, &self.scene)
        {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|since| since.as_micros() as u64)
                .unwrap_or_default();
            let frames: Vec<viz_dmx::UniverseFrame> = output
                .universes
                .iter()
                .map(|universe| {
                    let mut slots = [0_u8; 512];
                    let length = universe.slots.len().min(512);
                    slots[..length].copy_from_slice(&universe.slots[..length]);
                    viz_dmx::UniverseFrame {
                        logical_universe: universe.universe,
                        slots,
                        received_micros: now,
                        stale: false,
                    }
                })
                .collect();
            if !frames.is_empty() {
                decoder.apply(
                    scene,
                    &frames,
                    &mut self.values,
                    self.epoch.elapsed().as_secs_f32(),
                );
            }
            // The desk's own word on where its 3D Points are. A point patched to a universe was
            // also just decoded from its slots; the resolved pose the desk states is the same
            // number without the quantisation, and a point with no address has only this.
            apply_point_poses(scene, &output.points, &mut self.values);
            // The preload sits on top of the live picture rather than replacing it: a fixture
            // nobody preloaded goes on showing what it is doing. This also applies when the desk
            // has no patched universes; unpatched fixtures are still part of the show.
            if self.following_preload {
                let overlay: Vec<crate::preload_overlay::PreloadValue> = self
                    .preload_projection
                    .fixture_values
                    .iter()
                    .filter_map(|entry| match entry.value {
                        crate::wire::PreloadAttributeValue::Normalized(value) => {
                            Some(crate::preload_overlay::PreloadValue {
                                fixture_id: entry.fixture_id,
                                attribute: entry.attribute.clone(),
                                value,
                            })
                        }
                        crate::wire::PreloadAttributeValue::Other => None,
                    })
                    .collect();
                crate::preload_overlay::apply(scene, &overlay, &mut self.values);
            }
            // An empty output snapshot is still an authoritative source frame. A show may retain
            // a complete unpatched rig, and the Stage must keep presenting it instead of treating
            // the absence of network universes as the absence of the desk. But a snapshot that
            // says what the last one said is not a new frame: the pane holds its picture.
            let signature = desk_output_signature(
                &output,
                self.following_preload.then_some(&self.preload_projection),
                scene.revision,
            );
            if self.presented_desk_output != Some(signature) {
                self.presented_desk_output = Some(signature);
                stamp_desk_output_frame(&mut self.values, &mut self.value_frame, now);
                events.push(ProviderEvent::Values(Box::new(self.values.clone())));
            }
        }
    }
}

/// Put the desk's 3D Point poses onto the values, in renderer axes.
///
/// A point's origin is where its own fixture instance stands in the scene, which is the same
/// resolution every placement takes, so the point and its slaves agree on where "here" is. The
/// desk's `(x, y, z)` — across, upstage, up — becomes the renderer's `(x, z, -y)`, and its
/// rotation `(x, z, y)`, exactly as [`crate::transform`] converts a placement. A pose for a point
/// the scene does not hold is dropped: nothing can be slaved to a fixture that is not there.
pub(super) fn apply_point_poses(
    scene: &viz_scene::Scene,
    points: &[crate::wire::OutputPointPose],
    values: &mut SceneValues,
) {
    for point in points {
        let Some(instance) = scene
            .fixtures
            .iter()
            .find(|instance| instance.fixture_id == point.fixture_id)
        else {
            continue;
        };
        let [x, y, z] = point.offset_metres;
        let [rx, ry, rz] = point.rotation_degrees;
        let pose = viz_scene::PointPose {
            fixture_id: point.fixture_id,
            origin_metres: instance.position.to_array(),
            offset_metres: crate::transform::to_world(x, y, z).to_array(),
            rotation_degrees: crate::transform::rotation_to_world(rx, ry, rz).to_array(),
        };
        match values
            .position_points
            .iter_mut()
            .find(|held| held.fixture_id == point.fixture_id)
        {
            Some(held) => *held = pose,
            None => values.position_points.push(pose),
        }
    }
}
