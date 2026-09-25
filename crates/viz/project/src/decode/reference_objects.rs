//! Decoding the reference objects a renderer follows rather than lights: the external camera a
//! dedicated Visualizer takes its view from, and the 3D Points other placements are slaved to.

use super::Decoder;
use viz_scene::SceneValues;

impl Decoder {
    /// Every patched 3D Point's pose, read from the universes it is patched to.
    ///
    /// The desk writes each axis in its own axes — across the stage, upstage, up — and the
    /// renderer keeps `x` across, `y` up and `z` towards the audience, so the offset is turned by
    /// `(x, z, -y)` and the rotation by `(x, z, y)`, the same conversion every placement takes.
    /// An axis the mode does not carry, or whose universe has not arrived, reads as no movement:
    /// a point is never put somewhere nothing said it was.
    pub(super) fn decode_position_points(&self, values: &mut SceneValues) {
        for point in &self.position_points {
            let read = |channel: &Option<crate::binding::ChannelRef>, metres: bool| -> f32 {
                let Some(channel) = channel else {
                    return 0.0;
                };
                if !self.frames.contains_key(&channel.logical_universe) {
                    return 0.0;
                }
                let frame = self.slots(channel.logical_universe);
                if metres {
                    channel.point_axis_metres(&frame)
                } else {
                    channel.point_angle_degrees(&frame)
                }
            };
            let [x, y, z] = &point.position;
            let [rx, ry, rz] = &point.rotation;
            let offset = [read(x, true), read(y, true), read(z, true)];
            let turn = [read(rx, false), read(ry, false), read(rz, false)];
            let pose = viz_scene::PointPose {
                fixture_id: point.fixture_id,
                origin_metres: point.origin.to_array(),
                offset_metres: [offset[0], offset[2], -offset[1]],
                rotation_degrees: [turn[0], turn[2], turn[1]],
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

    pub(super) fn decode_external_camera(&self, values: &mut SceneValues) {
        let Some(binding) = &self.external_camera else {
            return;
        };
        // A split camera only becomes authoritative after every part has arrived at least once.
        // Until then an existing pose is retained instead of filling missing axes with zeroes.
        if !binding
            .universes
            .iter()
            .all(|universe| self.frames.contains_key(universe))
        {
            return;
        }
        let slots = |channel: &crate::binding::ChannelRef| self.slots(channel.logical_universe);
        let Some(x) = binding.x.camera_position_metres(&slots(&binding.x)) else {
            return;
        };
        let Some(y) = binding.y.camera_position_metres(&slots(&binding.y)) else {
            return;
        };
        let Some(z) = binding.z.camera_position_metres(&slots(&binding.z)) else {
            return;
        };
        let Some(yaw) = binding.yaw.camera_angle_degrees(&slots(&binding.yaw)) else {
            return;
        };
        let Some(pitch) = binding.pitch.camera_angle_degrees(&slots(&binding.pitch)) else {
            return;
        };
        let Some(roll) = binding.roll.camera_angle_degrees(&slots(&binding.roll)) else {
            return;
        };
        let Some((focal_length, vertical_fov)) = binding.zoom.camera_lens(&slots(&binding.zoom))
        else {
            return;
        };
        values.external_camera = Some(viz_scene::ExternalCameraState {
            fixture_id: binding.fixture_id,
            instance_id: binding.instance_id,
            position_metres: [x, y, z],
            yaw_degrees: yaw,
            pitch_degrees: pitch,
            roll_degrees: roll,
            focal_length_millimetres: focal_length,
            vertical_fov_degrees: vertical_fov,
            patched: true,
            stale: binding
                .universes
                .iter()
                .any(|universe| self.stale.get(universe).copied().unwrap_or(true)),
        });
    }
}
