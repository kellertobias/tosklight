//! Turn received universe frames into semantic emitter values.
//!
//! Only emitters bound to a universe that changed are re-decoded, and only changed semantic
//! parameters reach the render scene.

use crate::colour;
use crate::plan::{ColourBinding, EmitterBinding, ExternalCameraBinding};
use std::collections::HashMap;
use viz_dmx::{DMX_SLOTS, UniverseFrame};
use viz_scene::{
    CellValue, EmitterInstance, EmitterKind, EmitterValues, MotionAxis, PhysicalMotionState,
    PhysicalMotionTarget, Scene, SceneValues,
};

/// Holds the latest frame per logical universe and applies it to the emitter values.
pub struct Decoder {
    bindings: Vec<EmitterBinding>,
    external_camera: Option<ExternalCameraBinding>,
    frames: HashMap<u16, [u8; DMX_SLOTS]>,
    stale: HashMap<u16, bool>,
    /// Emitter indices reading each logical universe.
    readers: HashMap<u16, Vec<usize>>,
    frame_counter: u64,
    newest_input_micros: u64,
    /// When the previous decode happened. A strobe gate is integrated from here to now rather
    /// than sampled at now, which is the difference between a strobe and an aliasing artefact.
    last_time_seconds: Option<f32>,
}

impl Decoder {
    pub fn new(bindings: Vec<EmitterBinding>) -> Self {
        Self::with_external_camera(bindings, None)
    }

    pub fn with_external_camera(
        bindings: Vec<EmitterBinding>,
        external_camera: Option<ExternalCameraBinding>,
    ) -> Self {
        let mut readers: HashMap<u16, Vec<usize>> = HashMap::new();
        for (index, binding) in bindings.iter().enumerate() {
            for universe in &binding.universes {
                readers.entry(*universe).or_default().push(index);
            }
        }
        Self {
            bindings,
            external_camera,
            frames: HashMap::new(),
            stale: HashMap::new(),
            readers,
            frame_counter: 0,
            newest_input_micros: 0,
            last_time_seconds: None,
        }
    }

    /// Logical universes this show actually reads, used to configure the receivers.
    pub fn required_universes(&self) -> Vec<u16> {
        let mut universes: Vec<u16> = self.readers.keys().copied().collect();
        if let Some(camera) = &self.external_camera {
            universes.extend(camera.universes.iter().copied());
        }
        universes.sort_unstable();
        universes.dedup();
        universes
    }

    /// Establish physical home targets from each channel's exact `default_raw` value.
    /// The simulated position remains at the authored local 0-degree pose and travels to home
    /// under the same limits as a later authoritative DMX update.
    pub fn initialize_motion(&self, scene: &Scene, values: &mut SceneValues) {
        values.resize(scene.emitters.len());
        for (index, (binding, emitter)) in self.bindings.iter().zip(&scene.emitters).enumerate() {
            let value = &mut values.emitters[index];
            set_axis_default(
                &mut value.pan_motion,
                binding.pan.as_ref(),
                emitter.pan.as_ref(),
                binding.invert_pan,
            );
            set_axis_default(
                &mut value.tilt_motion,
                binding.tilt.as_ref(),
                emitter.tilt.as_ref(),
                binding.invert_tilt,
            );
            set_declared_rotation_default(
                &mut value.gobo_rotation_motion,
                binding.gobo_rotation.as_ref(),
            );
            set_declared_rotation_default(
                &mut value.prism_rotation_motion,
                binding.prism_rotation.as_ref(),
            );
            set_wheel_default(&mut value.gobo_wheel_motion, binding.gobo.as_ref());
            set_wheel_default(
                &mut value.colour_wheel_motion,
                binding.colour.wheel.as_ref(),
            );
            value.colour_wheel_palette = wheel_palette(binding.colour.wheel.as_ref());
        }
    }

    /// Reconcile a retained camera pose with the newly compiled patch without resetting it.
    ///
    /// Providers call this after carrying values across a scene delta. An absent or ambiguous
    /// binding marks the held pose unavailable for DMX authority while keeping every coordinate
    /// available to local control.
    pub fn reconcile_external_camera(&self, values: &mut SceneValues) {
        let Some(camera) = values.external_camera.as_mut() else {
            return;
        };
        camera.patched = self.external_camera.is_some();
        camera.stale = true;
    }

    /// Apply received frames. Returns the emitter indices that were re-decoded.
    pub fn apply(
        &mut self,
        scene: &Scene,
        received: &[UniverseFrame],
        values: &mut SceneValues,
        time_seconds: f32,
    ) -> usize {
        if received.is_empty() {
            return 0;
        }
        let mut affected: Vec<usize> = Vec::new();
        let camera_affected = self.external_camera.as_ref().is_some_and(|camera| {
            received
                .iter()
                .any(|frame| camera.universes.contains(&frame.logical_universe))
        });
        for frame in received {
            self.frames.insert(frame.logical_universe, frame.slots);
            self.stale.insert(frame.logical_universe, frame.stale);
            self.newest_input_micros = self.newest_input_micros.max(frame.received_micros);
            if let Some(readers) = self.readers.get(&frame.logical_universe) {
                affected.extend_from_slice(readers);
            }
        }
        affected.sort_unstable();
        affected.dedup();

        values.resize(scene.emitters.len());
        let previous_time = self.last_time_seconds.unwrap_or(time_seconds);
        for index in &affected {
            let Some(binding) = self.bindings.get(*index) else {
                continue;
            };
            let Some(emitter) = scene.emitters.get(*index) else {
                continue;
            };
            let mut value = values.emitters[*index].clone();
            self.decode_emitter(binding, emitter, &mut value, previous_time, time_seconds);
            values.emitters[*index] = value;
            // A laser's script reads raw slots, so the decoder's job for one is to capture the
            // footprint rather than to interpret it. Running the script here would put a
            // JavaScript engine on whatever thread a packet arrived on, and at DMX rate rather
            // than at frame rate; both are wrong, so the engine runs where the frames do.
            if emitter.kind == EmitterKind::Laser
                && let Some(window) = &binding.laser_window
            {
                let frame = self.slots(window.logical_universe);
                let scan = &mut values.laser_scans[*index];
                scan.slots.clear();
                scan.slots.extend(window.slots.iter().map(|slot| {
                    frame
                        .get(usize::from(*slot).saturating_sub(1))
                        .copied()
                        .unwrap_or(0)
                }));
            }
            if emitter.kind == EmitterKind::Effect
                && let Some(window) = &binding.effect_window
            {
                let frame = self.slots(window.logical_universe);
                let effect = &mut values.effect_frames[*index];
                effect.slots.clear();
                effect.slots.extend(window.slots.iter().map(|slot| {
                    frame
                        .get(usize::from(*slot).saturating_sub(1))
                        .copied()
                        .unwrap_or(0)
                }));
            }
        }
        if camera_affected {
            self.decode_external_camera(values);
        }
        self.last_time_seconds = Some(time_seconds);
        self.frame_counter += 1;
        values.frame = self.frame_counter;
        values.newest_input_micros = self.newest_input_micros;
        affected.len() + usize::from(camera_affected)
    }

    fn decode_external_camera(&self, values: &mut SceneValues) {
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

    fn slots(&self, universe: u16) -> [u8; DMX_SLOTS] {
        self.frames
            .get(&universe)
            .copied()
            .unwrap_or([0; DMX_SLOTS])
    }

    fn decode_emitter(
        &self,
        binding: &EmitterBinding,
        emitter: &EmitterInstance,
        value: &mut EmitterValues,
        previous_seconds: f32,
        time_seconds: f32,
    ) {
        let reader = |universe: u16| self.slots(universe);
        let read = |channel: &Option<crate::binding::ChannelRef>| -> Option<f32> {
            let channel = channel.as_ref()?;
            Some(channel.normalised(&self.slots(channel.logical_universe)))
        };

        let colour = colour::resolve(&binding.colour, &reader);
        value.colour = colour.rgb;

        // An explicit dimmer wins; otherwise the colour emitters act as a virtual dimmer.
        value.intensity = match read(&binding.intensity) {
            Some(level) => level,
            None if colour.explicit => colour.level,
            None => 0.0,
        };

        value.pan = flip(read(&binding.pan).unwrap_or(0.5), binding.invert_pan);
        value.tilt = flip(read(&binding.tilt).unwrap_or(0.5), binding.invert_tilt);
        set_axis_target(
            &mut value.pan_motion,
            binding.pan.as_ref(),
            emitter.pan.as_ref(),
            &reader,
            binding.invert_pan,
        );
        set_axis_target(
            &mut value.tilt_motion,
            binding.tilt.as_ref(),
            emitter.tilt.as_ref(),
            &reader,
            binding.invert_tilt,
        );
        value.zoom = read(&binding.zoom).unwrap_or(0.5);
        value.iris = read(&binding.iris).unwrap_or(0.0);
        value.frost = read(&binding.frost).unwrap_or(0.0);
        value.focus = read(&binding.focus).unwrap_or(0.5);
        value.gobo = read(&binding.gobo).unwrap_or(0.0);
        set_wheel_target(&mut value.gobo_wheel_motion, binding.gobo.as_ref(), &reader);
        value.gobo_rotation = read(&binding.gobo_rotation).unwrap_or(0.0);
        value.prism = read(&binding.prism).unwrap_or(0.0);
        value.prism_rotation = read(&binding.prism_rotation).unwrap_or(0.0);
        set_declared_rotation_target(
            &mut value.gobo_rotation_motion,
            binding.gobo_rotation.as_ref(),
            &reader,
        );
        set_wheel_target(
            &mut value.colour_wheel_motion,
            binding.colour.wheel.as_ref(),
            &reader,
        );
        value.colour_wheel_palette = wheel_palette(binding.colour.wheel.as_ref());
        set_declared_rotation_target(
            &mut value.prism_rotation_motion,
            binding.prism_rotation.as_ref(),
            &reader,
        );
        for (slot, blade) in value
            .shaper_blades
            .iter_mut()
            .zip(binding.shaper_blades.iter())
        {
            *slot = read(blade).unwrap_or(0.0);
        }
        for (slot, blade) in value
            .shaper_blade_angles_degrees
            .iter_mut()
            .zip(binding.shaper_blade_angles.iter())
        {
            *slot = blade
                .as_ref()
                .map(|channel| channel.physical(&self.slots(channel.logical_universe)))
                .unwrap_or(0.0);
        }
        value.shaper_rotation = read(&binding.shaper_rotation).unwrap_or(0.0);
        value.shaper_rotation_degrees = binding
            .shaper_rotation
            .as_ref()
            .map(|channel| channel.physical(&self.slots(channel.logical_universe)))
            .unwrap_or(0.0);

        let (shutter, strobe_hz) = self.decode_shutter(binding);
        value.strobe_hz = strobe_hz;
        value.shutter = if strobe_hz > 0.0 {
            // How much of the interval since the last decode the gate was actually open, not
            // whether it happened to be open at the instant this decode fell. Point-sampling a
            // square wave against a frame clock is textbook aliasing: a 15 Hz strobe watched at
            // 60 Hz beats against the refresh and reads as an irregular stutter, and a strobe
            // faster than the frame rate mostly disappears. Integrating over the interval gives
            // every flash its real weight however the two rates line up.
            shutter * strobe_openness(previous_seconds, time_seconds, strobe_hz)
        } else {
            shutter
        };

        if emitter.kind == EmitterKind::Atmosphere {
            value.intensity = read(&binding.fog).unwrap_or(value.intensity);
        }

        value.cells = if binding.cells.is_empty() {
            Vec::new()
        } else {
            binding
                .cells
                .iter()
                .enumerate()
                .map(|(index, cell)| {
                    let mut decoded = self.decode_cell(cell, value);
                    // Decoding rebuilds the cell list from scratch every DMX frame, and the tail
                    // a cell is part-way through fading is display state rather than decoded
                    // state. Losing it here would leave per-cell persistence resetting to nothing
                    // at DMX rate, which is exactly the fixtures — blinders, pixel strips — that
                    // need it most.
                    decoded.held_intensity = value
                        .cells
                        .get(index)
                        .map(|previous| previous.held_intensity)
                        .unwrap_or(0.0);
                    decoded
                })
                .collect()
        };
        value.stale = binding
            .universes
            .iter()
            .any(|universe| self.stale.get(universe).copied().unwrap_or(true));
    }

    fn decode_cell(&self, cell: &ColourBinding, emitter: &EmitterValues) -> CellValue {
        let reader = |universe: u16| self.slots(universe);
        let colour = colour::resolve(cell, &reader);
        let cell_level = cell
            .intensity
            .as_ref()
            .map(|channel| channel.normalised(&self.slots(channel.logical_universe)))
            .unwrap_or(if colour.explicit { colour.level } else { 1.0 });
        CellValue {
            intensity: (cell_level
                * emitter
                    .intensity
                    .max(if cell.intensity.is_some() { 0.0 } else { 1.0 }))
            .clamp(0.0, 1.0),
            colour: colour.rgb,
            held_intensity: 0.0,
        }
    }

    /// Shutter gate and strobe rate from the shutter and strobe channels.
    fn decode_shutter(&self, binding: &EmitterBinding) -> (f32, f32) {
        let mut gate = 1.0_f32;
        let mut rate = 0.0_f32;
        if let Some(shutter) = &binding.shutter {
            let slots = self.slots(shutter.logical_universe);
            match shutter.function(&slots) {
                Some(function) => {
                    let name = function.name.to_ascii_lowercase();
                    if name.contains("closed") || name.contains("blackout") {
                        gate = 0.0;
                    }
                    if name.contains("strobe") || name.contains("flash") {
                        rate = shutter
                            .function_physical(&slots)
                            .unwrap_or(6.0)
                            .clamp(0.5, 40.0);
                    }
                }
                None => {
                    // Without function metadata a shutter channel is treated as a proportional
                    // gate, which is the safe generic behaviour.
                    gate = shutter.normalised(&slots);
                }
            }
        }
        if let Some(strobe) = &binding.strobe {
            let slots = self.slots(strobe.logical_universe);
            let level = strobe.normalised(&slots);
            if level > 0.004 {
                let physical = strobe.physical(&slots);
                rate = if strobe.physical_max > 1.5 {
                    physical.clamp(0.5, 40.0)
                } else {
                    (0.5 + level * 24.5).clamp(0.5, 40.0)
                };
            }
        }
        (gate, rate)
    }
}

fn set_axis_target<F>(
    state: &mut PhysicalMotionState,
    channel: Option<&crate::binding::ChannelRef>,
    axis: Option<&MotionAxis>,
    reader: &F,
    invert: bool,
) where
    F: Fn(u16) -> [u8; DMX_SLOTS],
{
    let (Some(channel), Some(axis)) = (channel, axis) else {
        return;
    };
    let frame = reader(channel.logical_universe);
    let target = if channel
        .function(&frame)
        .and_then(|function| function.angular_motion)
        .is_some()
    {
        channel.angular_motion_target(&frame, false).map(|target| {
            if invert {
                invert_motion_target(target)
            } else {
                target
            }
        })
    } else {
        Some(PhysicalMotionTarget::Position {
            degrees: axis.degrees_at(flip(channel.normalised(&frame), invert)),
            max_speed: crate::binding::FALLBACK_ANGULAR_SPEED,
            acceleration: crate::binding::FALLBACK_ANGULAR_ACCELERATION,
            deceleration: crate::binding::FALLBACK_ANGULAR_ACCELERATION,
        })
    };
    if let Some(target) = target {
        state.set_target(target);
    }
}

fn set_axis_default(
    state: &mut PhysicalMotionState,
    channel: Option<&crate::binding::ChannelRef>,
    axis: Option<&MotionAxis>,
    invert: bool,
) {
    let (Some(channel), Some(axis)) = (channel, axis) else {
        return;
    };
    let target = if channel
        .functions
        .iter()
        .find(|function| {
            channel.default_raw >= function.dmx_from && channel.default_raw <= function.dmx_to
        })
        .and_then(|function| function.angular_motion)
        .is_some()
    {
        channel.angular_motion_default_target(false).map(|target| {
            if invert {
                invert_motion_target(target)
            } else {
                target
            }
        })
    } else {
        let mut level = channel.default_raw as f32 / channel.max_raw.max(1) as f32;
        if channel.invert {
            level = 1.0 - level;
        }
        Some(PhysicalMotionTarget::Position {
            degrees: axis.degrees_at(flip(level, invert)),
            max_speed: crate::binding::FALLBACK_ANGULAR_SPEED,
            acceleration: crate::binding::FALLBACK_ANGULAR_ACCELERATION,
            deceleration: crate::binding::FALLBACK_ANGULAR_ACCELERATION,
        })
    };
    if let Some(target) = target {
        state.set_target(target);
    }
}

fn set_declared_rotation_target<F>(
    state: &mut PhysicalMotionState,
    channel: Option<&crate::binding::ChannelRef>,
    reader: &F,
) where
    F: Fn(u16) -> [u8; DMX_SLOTS],
{
    let Some(channel) = channel else { return };
    let frame = reader(channel.logical_universe);
    if let Some(target) = channel.angular_motion_target(&frame, false) {
        state.set_target(target);
    }
}

fn set_declared_rotation_default(
    state: &mut PhysicalMotionState,
    channel: Option<&crate::binding::ChannelRef>,
) {
    let Some(channel) = channel else { return };
    if let Some(target) = channel.angular_motion_default_target(false) {
        state.set_target(target);
    }
}

fn set_wheel_target<F>(
    state: &mut viz_scene::WheelMotionState,
    channel: Option<&crate::binding::ChannelRef>,
    reader: &F,
) where
    F: Fn(u16) -> [u8; DMX_SLOTS],
{
    let Some(channel) = channel else { return };
    let frame = reader(channel.logical_universe);
    if let Some(target) = channel.wheel_target(&frame) {
        state.set_target(
            target.index,
            target.count,
            target.max_speed,
            target.acceleration,
            target.deceleration,
        );
    }
}

fn set_wheel_default(
    state: &mut viz_scene::WheelMotionState,
    channel: Option<&crate::binding::ChannelRef>,
) {
    let Some(channel) = channel else { return };
    if let Some(target) = channel.wheel_default_target() {
        state.set_target(
            target.index,
            target.count,
            target.max_speed,
            target.acceleration,
            target.deceleration,
        );
    }
}

fn wheel_palette(channel: Option<&crate::binding::ChannelRef>) -> Vec<[f32; 3]> {
    let Some(channel) = channel else {
        return Vec::new();
    };
    let mut functions = channel
        .functions
        .iter()
        .filter(|function| {
            matches!(
                function.behavior,
                light_fixture::ChannelFunctionBehavior::Indexed { .. }
                    | light_fixture::ChannelFunctionBehavior::Fixed { .. }
            )
        })
        .collect::<Vec<_>>();
    functions.sort_by_key(|function| function.dmx_from);
    functions
        .into_iter()
        .map(|function| colour::named_colour(&function.name))
        .collect()
}

fn invert_motion_target(target: PhysicalMotionTarget) -> PhysicalMotionTarget {
    match target {
        PhysicalMotionTarget::Position {
            degrees,
            max_speed,
            acceleration,
            deceleration,
        } => PhysicalMotionTarget::Position {
            degrees: -degrees,
            max_speed,
            acceleration,
            deceleration,
        },
        PhysicalMotionTarget::Velocity {
            degrees_per_second,
            acceleration,
            deceleration,
        } => PhysicalMotionTarget::Velocity {
            degrees_per_second: -degrees_per_second,
            acceleration,
            deceleration,
        },
    }
}

fn flip(value: f32, invert: bool) -> f32 {
    if invert { 1.0 - value } else { value }
}

/// Duty cycle of a strobe gate: a flash occupies this much of each period.
///
/// Real shutters and LED strobes fire a short, bright pulse rather than a half-on square. A
/// quarter is a fair middle; what matters far more than the exact figure is that it is integrated
/// rather than sampled.
const STROBE_DUTY: f32 = 0.25;

/// The fraction of `[previous, now]` a strobe gate at `hz` was open.
///
/// Exact rather than stochastic: the open time is a closed-form function of the window, so a
/// flash is never missed because no frame happened to land on it, and never counted twice because
/// two frames both did. An empty window falls back to sampling the instant, which is the right
/// answer when there is no interval to integrate over.
fn strobe_openness(previous: f32, now: f32, hz: f32) -> f32 {
    let span = now - previous;
    if !span.is_finite() || span <= 0.0 {
        return if (now * hz).fract() < STROBE_DUTY {
            1.0
        } else {
            0.0
        };
    }
    (open_since_zero(now, hz) - open_since_zero(previous, hz)) / span
}

/// Total time the gate has been open between zero and `t`.
fn open_since_zero(t: f32, hz: f32) -> f32 {
    let period = 1.0 / hz;
    let open = period * STROBE_DUTY;
    let cycles = (t / period).floor();
    let within = t - cycles * period;
    cycles * open + within.min(open)
}

/// Scene and frame builders shared by both test modules below.
#[cfg(test)]
mod tests_support {
    pub(super) use super::tests::{frame, scene};
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::binding::ChannelRef;
    use crate::plan::ColourBinding;
    use glam::Vec3;
    use light_core::AttributeKey;
    use light_fixture::{
        AngularMotion, AngularMotionKind, ChannelFunction, ChannelFunctionBehavior,
    };
    use viz_scene::{
        EmitterInstance, EmitterLayoutCells, EmitterOptics, FixtureBody, FixtureInstance,
    };

    pub(super) fn channel(slot: u16) -> ChannelRef {
        ChannelRef {
            logical_universe: 1,
            slots: vec![slot],
            max_raw: 255,
            invert: false,
            physical_min: 0.0,
            physical_max: 1.0,
            snap: false,
            default_raw: 0,
            functions: Vec::new(),
        }
    }

    fn camera_channel(first_slot: u16, bytes: usize) -> ChannelRef {
        ChannelRef {
            logical_universe: 1,
            slots: (first_slot..first_slot + bytes as u16).collect(),
            max_raw: match bytes {
                2 => 0xffff,
                3 => 0x00ff_ffff,
                _ => unreachable!("camera channels are U16 or U24"),
            },
            invert: false,
            physical_min: 0.0,
            physical_max: 1.0,
            snap: false,
            default_raw: 0,
            functions: Vec::new(),
        }
    }

    fn external_camera_binding() -> ExternalCameraBinding {
        ExternalCameraBinding {
            fixture_id: uuid::Uuid::from_u128(1),
            instance_id: uuid::Uuid::from_u128(2),
            label: "Camera 1".into(),
            x: camera_channel(1, 3),
            y: camera_channel(4, 3),
            z: camera_channel(7, 3),
            yaw: camera_channel(10, 2),
            pitch: camera_channel(12, 2),
            roll: camera_channel(14, 2),
            zoom: camera_channel(16, 2),
            universes: vec![1],
        }
    }

    pub(super) fn emitter(kind: EmitterKind) -> EmitterInstance {
        EmitterInstance {
            fixture_index: 0,
            head_index: 0,
            label: "Main".into(),
            local_origin: Vec3::ZERO,
            tilt_pivot: Vec3::ZERO,
            local_orientation_degrees: Vec3::ZERO,
            pan: None,
            tilt: None,
            beam_angle_degrees: 10.0,
            field_angle_degrees: 30.0,
            optics: EmitterOptics::default(),
            kind,
            cells: EmitterLayoutCells::single(),
            laser: None,
            effect: None,
            live_shaper_angle_roles: [false; 4],
            shaper_roles: [false; 4],
            live_shaper_rotation_role: false,
        }
    }

    pub(super) fn scene(kinds: &[EmitterKind]) -> Scene {
        let mut scene = Scene::default();
        scene.fixtures.push(FixtureInstance {
            instance_id: uuid::Uuid::nil(),
            fixture_id: uuid::Uuid::nil(),
            name: "Test".into(),
            number: None,
            position: Vec3::ZERO,
            rotation_degrees: Vec3::ZERO,
            bracket_degrees: 0.0,
            shaper_degrees: None,
            installed_colour: [1.0; 3],
            installed_shaper_angles_degrees: [0.0; 4],
            body: FixtureBody::default(),
            patched: true,
            address: None,
            model: None,
            fallback: None,
        });
        for kind in kinds {
            scene.emitters.push(emitter(*kind));
        }
        scene
    }

    pub(super) fn frame(values: &[(usize, u8)]) -> UniverseFrame {
        let mut slots = [0_u8; DMX_SLOTS];
        for (index, value) in values {
            slots[*index] = *value;
        }
        UniverseFrame {
            logical_universe: 1,
            slots,
            received_micros: 1_000,
            stale: false,
        }
    }

    #[test]
    fn an_intensity_channel_drives_the_emitter_level() {
        let binding = EmitterBinding {
            intensity: Some(channel(1)),
            universes: vec![1],
            ..EmitterBinding::default()
        };
        let mut decoder = Decoder::new(vec![binding]);
        let scene = scene(&[EmitterKind::Beam]);
        let mut values = SceneValues::default();
        decoder.apply(&scene, &[frame(&[(0, 255)])], &mut values, 0.0);
        assert_eq!(values.emitters[0].intensity, 1.0);
        decoder.apply(&scene, &[frame(&[(0, 0)])], &mut values, 0.0);
        assert_eq!(values.emitters[0].intensity, 0.0);
    }

    #[test]
    fn external_camera_decodes_the_exact_seventeen_slot_contract_and_holds_stale_pose() {
        let mut decoder =
            Decoder::with_external_camera(Vec::new(), Some(external_camera_binding()));
        let scene = Scene::default();
        let mut values = SceneValues::default();
        let mut slots = [0_u8; DMX_SLOTS];
        // X = exact zero, Y = +1 m, Z = minimum. Orientation = -360, near zero, +360.
        slots[0..3].copy_from_slice(&[0x80, 0x00, 0x00]);
        slots[3..6].copy_from_slice(&[0x80, 0x07, 0xd0]);
        slots[6..9].copy_from_slice(&[0x00, 0x00, 0x00]);
        slots[9..11].copy_from_slice(&[0x00, 0x00]);
        slots[11..13].copy_from_slice(&[0x80, 0x00]);
        slots[13..15].copy_from_slice(&[0xff, 0xff]);
        slots[15..17].copy_from_slice(&[0x00, 0x00]);
        let live = UniverseFrame {
            logical_universe: 1,
            slots,
            received_micros: 1_000,
            stale: false,
        };
        assert_eq!(
            decoder.apply(&scene, std::slice::from_ref(&live), &mut values, 0.0),
            1
        );
        let camera = values.external_camera.expect("camera decoded");
        assert_eq!(camera.position_metres, [0.0, 1.0, -4_194.304]);
        assert_eq!(camera.yaw_degrees, -360.0);
        assert!(camera.pitch_degrees.abs() < 0.006);
        assert_eq!(camera.roll_degrees, 360.0);
        assert!((camera.focal_length_millimetres - 18.0).abs() < 1e-5);
        assert!((camera.vertical_fov_degrees - 67.380_135).abs() < 1e-4);
        assert!(!camera.stale);

        let stale = UniverseFrame {
            stale: true,
            ..live
        };
        decoder.apply(&scene, &[stale], &mut values, 1.0);
        let held = values.external_camera.expect("last pose retained");
        assert_eq!(held.position_metres, camera.position_metres);
        assert!(held.stale);

        Decoder::new(Vec::new()).reconcile_external_camera(&mut values);
        let unpatched = values
            .external_camera
            .expect("unpatch retains the last pose");
        assert_eq!(unpatched.position_metres, camera.position_metres);
        assert!(!unpatched.patched);
        assert!(unpatched.stale);
    }

    #[test]
    fn colour_channels_act_as_a_virtual_dimmer_without_an_intensity_channel() {
        let binding = EmitterBinding {
            colour: ColourBinding {
                red: Some(channel(1)),
                green: Some(channel(2)),
                blue: Some(channel(3)),
                ..ColourBinding::default()
            },
            universes: vec![1],
            ..EmitterBinding::default()
        };
        let mut decoder = Decoder::new(vec![binding]);
        let scene = scene(&[EmitterKind::Beam]);
        let mut values = SceneValues::default();
        decoder.apply(
            &scene,
            &[frame(&[(0, 255), (1, 0), (2, 0)])],
            &mut values,
            0.0,
        );
        assert_eq!(values.emitters[0].intensity, 1.0);
        assert_eq!(values.emitters[0].colour, [1.0, 0.0, 0.0]);
    }

    #[test]
    fn axis_inversion_flips_pan_and_tilt() {
        let binding = EmitterBinding {
            pan: Some(channel(1)),
            tilt: Some(channel(2)),
            invert_pan: true,
            universes: vec![1],
            ..EmitterBinding::default()
        };
        let mut decoder = Decoder::new(vec![binding]);
        let scene = scene(&[EmitterKind::Beam]);
        let mut values = SceneValues::default();
        decoder.apply(&scene, &[frame(&[(0, 255), (1, 255)])], &mut values, 0.0);
        assert_eq!(values.emitters[0].pan, 0.0);
        assert_eq!(values.emitters[0].tilt, 1.0);
    }

    #[test]
    fn pan_uses_a_functions_exact_raw_span_and_declared_dynamics() {
        let mut pan = channel(1);
        pan.default_raw = 128;
        pan.functions = vec![ChannelFunction {
            id: uuid::Uuid::nil(),
            name: "finite pan".into(),
            dmx_from: 64,
            dmx_to: 191,
            attribute: AttributeKey("pan".into()),
            priority: 0,
            angular_motion: Some(AngularMotion {
                kind: AngularMotionKind::AbsolutePosition,
                max_speed_degrees_per_second: Some(180.0),
                acceleration_degrees_per_second_squared: Some(360.0),
                deceleration_degrees_per_second_squared: Some(240.0),
            }),
            behavior: ChannelFunctionBehavior::Continuous {
                physical_min: -270.0,
                physical_max: 270.0,
                unit: Some("deg".into()),
            },
        }];
        let binding = EmitterBinding {
            pan: Some(pan),
            universes: vec![1],
            ..EmitterBinding::default()
        };
        let mut rig = scene(&[EmitterKind::Beam]);
        rig.emitters[0].pan = Some(MotionAxis {
            axis: Vec3::Y,
            min_degrees: -270.0,
            max_degrees: 270.0,
        });
        let mut values = SceneValues::default();
        let mut decoder = Decoder::new(vec![binding]);
        decoder.initialize_motion(&rig, &mut values);
        assert!(matches!(
            values.emitters[0].pan_motion.target,
            Some(PhysicalMotionTarget::Position { degrees, .. }) if degrees.abs() < 3.0
        ));
        decoder.apply(&rig, &[frame(&[(0, 191)])], &mut values, 0.0);
        assert_eq!(values.emitters[0].pan_motion.position_degrees, 0.0);
        assert_eq!(
            values.emitters[0].pan_motion.target,
            Some(PhysicalMotionTarget::Position {
                degrees: 270.0,
                max_speed: 180.0,
                acceleration: 360.0,
                deceleration: 240.0,
            })
        );
    }

    #[test]
    fn legacy_pan_gets_fast_physical_fallback_instead_of_teleporting() {
        let binding = EmitterBinding {
            pan: Some(channel(1)),
            universes: vec![1],
            ..EmitterBinding::default()
        };
        let mut rig = scene(&[EmitterKind::Beam]);
        rig.emitters[0].pan = Some(MotionAxis {
            axis: Vec3::Y,
            min_degrees: -270.0,
            max_degrees: 270.0,
        });
        let mut values = SceneValues::default();
        Decoder::new(vec![binding]).apply(&rig, &[frame(&[(0, 255)])], &mut values, 0.0);
        assert_eq!(values.emitters[0].pan_motion.position_degrees, 0.0);
        values.apply_physical_motion(0.1);
        assert!(values.emitters[0].pan_motion.position_degrees > 0.0);
        assert!(values.emitters[0].pan_motion.position_degrees < 270.0);
    }

    #[test]
    fn shaper_angles_decode_in_physical_degrees() {
        let mut blade = channel(1);
        blade.physical_min = -90.0;
        blade.physical_max = 90.0;
        let mut rotation = channel(2);
        rotation.physical_min = -180.0;
        rotation.physical_max = 180.0;
        let binding = EmitterBinding {
            shaper_blade_angles: [Some(blade), None, None, None],
            shaper_rotation: Some(rotation),
            universes: vec![1],
            ..EmitterBinding::default()
        };
        let mut decoder = Decoder::new(vec![binding]);
        let scene = scene(&[EmitterKind::Beam]);
        let mut values = SceneValues::default();
        decoder.apply(&scene, &[frame(&[(0, 255), (1, 0)])], &mut values, 0.0);
        assert_eq!(values.emitters[0].shaper_blade_angles_degrees[0], 90.0);
        assert_eq!(values.emitters[0].shaper_rotation_degrees, -180.0);
    }

    #[test]
    fn hazer_output_is_decoded_but_never_touches_the_atmosphere() {
        let hazer = EmitterBinding {
            fog: Some(channel(1)),
            universes: vec![1],
            ..EmitterBinding::default()
        };
        let mut decoder = Decoder::new(vec![hazer]);
        let scene = scene(&[EmitterKind::Atmosphere]);
        let mut values = SceneValues::default();
        values.atmosphere.density = 0.5;
        decoder.apply(&scene, &[frame(&[(0, 255)])], &mut values, 0.0);
        assert_eq!(
            values.emitters[0].intensity, 1.0,
            "the hazer output decodes"
        );
        assert_eq!(
            values.atmosphere.density, 0.5,
            "haze is the renderer's own setting, not the hazer's output"
        );
    }

    #[test]
    fn only_emitters_reading_a_changed_universe_are_re_decoded() {
        let first = EmitterBinding {
            intensity: Some(channel(1)),
            universes: vec![1],
            ..EmitterBinding::default()
        };
        let mut second = EmitterBinding {
            intensity: Some(channel(1)),
            universes: vec![7],
            ..EmitterBinding::default()
        };
        if let Some(channel) = second.intensity.as_mut() {
            channel.logical_universe = 7;
        }
        let mut decoder = Decoder::new(vec![first, second]);
        let scene = scene(&[EmitterKind::Beam, EmitterKind::Beam]);
        let mut values = SceneValues::default();
        let touched = decoder.apply(&scene, &[frame(&[(0, 255)])], &mut values, 0.0);
        assert_eq!(touched, 1);
        assert_eq!(values.emitters[0].intensity, 1.0);
        assert_eq!(values.emitters[1].intensity, 0.0);
    }

    #[test]
    fn a_strobe_channel_gates_the_shutter_over_time() {
        let mut strobe = channel(2);
        strobe.physical_max = 20.0;
        let binding = EmitterBinding {
            intensity: Some(channel(1)),
            strobe: Some(strobe),
            universes: vec![1],
            ..EmitterBinding::default()
        };
        let mut decoder = Decoder::new(vec![binding]);
        let scene = scene(&[EmitterKind::Beam]);
        let mut values = SceneValues::default();
        let packet = frame(&[(0, 255), (1, 128)]);
        decoder.apply(&scene, std::slice::from_ref(&packet), &mut values, 0.0);
        assert!(values.emitters[0].strobe_hz > 0.5);
        let on = values.emitters[0].shutter;
        let half_period = 0.5 / values.emitters[0].strobe_hz;
        decoder.apply(&scene, &[packet], &mut values, half_period);
        let off = values.emitters[0].shutter;
        assert!(on != off, "the gate must change across the strobe period");
    }
}

#[cfg(test)]
mod strobe_and_laser_tests {
    use super::tests_support::*;
    use super::*;
    use crate::plan::{EffectWindow, LaserWindow};

    /// The reason the gate is integrated at all: no rate may vanish because the frames happened to
    /// fall in its dark half, and none may read as continuously lit either. Averaged over a
    /// second, a quarter-duty strobe delivers a quarter of the light at every rate.
    #[test]
    fn a_strobe_delivers_its_duty_cycle_at_every_rate_against_every_frame_clock() {
        for hz in [2.0_f32, 15.0, 25.0, 47.0, 90.0] {
            for frame_rate in [30.0_f32, 60.0, 144.0] {
                let step = 1.0 / frame_rate;
                let frames = frame_rate as usize;
                let total: f32 = (0..frames)
                    .map(|index| {
                        let now = index as f32 * step;
                        strobe_openness(now, now + step, hz)
                    })
                    .sum();
                let mean = total / frames as f32;
                assert!(
                    (mean - STROBE_DUTY).abs() < 0.02,
                    "{hz} Hz at {frame_rate} fps averaged {mean}, wanted {STROBE_DUTY}"
                );
            }
        }
    }

    /// A strobe well below the frame rate has to still look like a strobe: some frames fully lit,
    /// some fully dark. An average that was right but never varied would be a dimmer.
    #[test]
    fn a_slow_strobe_still_produces_light_and_dark_frames() {
        let step = 1.0 / 60.0;
        let samples: Vec<f32> = (0..60)
            .map(|index| {
                let now = index as f32 * step;
                strobe_openness(now, now + step, 5.0)
            })
            .collect();
        assert!(
            samples.iter().any(|value| *value >= 0.99),
            "no frame caught a full flash"
        );
        assert!(
            samples.iter().any(|value| *value <= 0.01),
            "no frame was fully dark"
        );
    }

    /// A laser is handed its fixture's raw slots, in patch order, and nothing else.
    #[test]
    fn a_laser_emitter_captures_its_fixture_footprint() {
        let binding = EmitterBinding {
            universes: vec![1],
            laser_window: Some(LaserWindow {
                logical_universe: 1,
                slots: vec![1, 2, 3, 4],
            }),
            ..EmitterBinding::default()
        };
        let mut decoder = Decoder::new(vec![binding]);
        let scene = scene(&[EmitterKind::Laser]);
        let mut values = SceneValues::default();
        decoder.apply(
            &scene,
            &[frame(&[(0, 10), (1, 20), (2, 30), (3, 40)])],
            &mut values,
            0.0,
        );
        assert_eq!(values.laser_scans[0].slots, vec![10, 20, 30, 40]);
    }

    #[test]
    fn an_effect_program_receives_the_exact_fixture_slots_in_patch_order() {
        let binding = EmitterBinding {
            universes: vec![1],
            effect_window: Some(EffectWindow {
                logical_universe: 1,
                slots: vec![1, 2, 3],
            }),
            ..EmitterBinding::default()
        };
        let mut decoder = Decoder::new(vec![binding]);
        let scene = scene(&[EmitterKind::Effect]);
        let mut values = SceneValues::default();
        decoder.apply(
            &scene,
            &[frame(&[(0, 17), (1, 91), (2, 203)])],
            &mut values,
            0.0,
        );
        assert_eq!(values.effect_frames[0].slots, vec![17, 91, 203]);
    }

    /// A head that is not a laser must never grow a footprint, or every fixture in the show would
    /// carry a copy of its own DMX for nothing.
    #[test]
    fn a_beam_emitter_captures_no_footprint() {
        let binding = EmitterBinding {
            universes: vec![1],
            laser_window: Some(LaserWindow {
                logical_universe: 1,
                slots: vec![1],
            }),
            ..EmitterBinding::default()
        };
        let mut decoder = Decoder::new(vec![binding]);
        let scene = scene(&[EmitterKind::Beam]);
        let mut values = SceneValues::default();
        decoder.apply(&scene, &[frame(&[(0, 255)])], &mut values, 0.0);
        assert!(values.laser_scans[0].slots.is_empty());
    }
}
