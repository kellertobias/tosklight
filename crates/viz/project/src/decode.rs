//! Turn received universe frames into semantic emitter values.
//!
//! Only emitters bound to a universe that changed are re-decoded, and only changed semantic
//! parameters reach the render scene.

use crate::colour;
use crate::plan::{ColourBinding, EmitterBinding, ExternalCameraBinding, PositionPointBinding};
use std::collections::HashMap;
use viz_dmx::{DMX_SLOTS, UniverseFrame};
use viz_scene::{
    CellValue, EmitterInstance, EmitterKind, EmitterValues, MotionAxis, PhysicalMotionState,
    PhysicalMotionTarget, Scene, SceneValues,
};

/// Borrowed final-native record; transport ownership stays outside the projection crate.
pub struct NativeInstanceValues<'a> {
    pub fixture_id: uuid::Uuid,
    pub instance_id: uuid::Uuid,
    pub native_identity: &'a str,
    pub raw: &'a [u32],
    pub owned_channels: Option<&'a [bool]>,
}

/// Holds the latest frame per logical universe and applies it to the emitter values.
pub struct Decoder {
    bindings: Vec<EmitterBinding>,
    physical: Vec<crate::physical::PhysicalRuntime>,
    physical_indices: Vec<Option<usize>>,
    physical_instances: HashMap<uuid::Uuid, usize>,
    native_updated: Vec<bool>,
    external_camera: Option<ExternalCameraBinding>,
    /// The 3D Points with a DMX address, read off the wire like any lantern.
    position_points: Vec<PositionPointBinding>,
    frames: HashMap<u16, [u8; DMX_SLOTS]>,
    stale: HashMap<u16, bool>,
    /// Emitter indices reading each logical universe.
    readers: HashMap<u16, Vec<usize>>,
    frame_counter: u64,
    newest_input_micros: u64,
    /// When the previous decode happened. A strobe gate is integrated from here to now rather
    /// than sampled at now, which is the difference between a strobe and an aliasing artefact.
    last_time_seconds: Option<f32>,
    previous_time_seconds: Option<f32>,
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
        let mut physical = Vec::<crate::physical::PhysicalRuntime>::new();
        let mut by_instance = HashMap::new();
        let physical_indices = bindings
            .iter()
            .map(|binding| {
                binding.physical.as_ref().map(|binding| {
                    *by_instance
                        .entry(binding.plan.instance_id)
                        .or_insert_with(|| {
                            let i = physical.len();
                            physical
                                .push(crate::physical::PhysicalRuntime::new(binding.plan.clone()));
                            i
                        })
                })
            })
            .collect();
        Self {
            native_updated: vec![false; physical.len()],
            physical_instances: by_instance,
            physical,
            physical_indices,
            bindings,
            external_camera,
            position_points: Vec::new(),
            frames: HashMap::new(),
            stale: HashMap::new(),
            readers,
            frame_counter: 0,
            newest_input_micros: 0,
            last_time_seconds: None,
            previous_time_seconds: None,
        }
    }

    fn begin_time(&mut self, now: f32) -> f32 {
        if self.last_time_seconds != Some(now) {
            self.previous_time_seconds = self.last_time_seconds;
            self.last_time_seconds = Some(now);
        }
        self.previous_time_seconds.unwrap_or(now)
    }
    pub fn accepts_native_snapshot<'a>(
        &self,
        identities: impl IntoIterator<Item = (uuid::Uuid, &'a str)>,
    ) -> bool {
        let identities: HashMap<_, _> = identities.into_iter().collect();
        self.physical.iter().all(|p| {
            identities
                .get(&p.plan.instance_id)
                .is_some_and(|id| *id == p.plan.native_identity)
        })
    }

    /// Read complete server-resolved raw values without advertising private fixture buffers as
    /// network universes. Live fills genuinely unpatched channels; Preload replaces only explicitly owned channels.
    pub fn apply_native<'a>(
        &mut self,
        scene: &Scene,
        records: impl IntoIterator<Item = NativeInstanceValues<'a>>,
        preload: bool,
        values: &mut SceneValues,
        time_seconds: f32,
    ) -> usize {
        self.native_updated.fill(false);
        for record in records {
            let Some(&index) = self.physical_instances.get(&record.instance_id) else {
                continue;
            };
            let runtime = &mut self.physical[index];
            if runtime.plan.fixture_id == record.fixture_id
                && runtime.plan.native_identity == record.native_identity
            {
                self.native_updated[index] =
                    runtime.update_native(record.raw, record.owned_channels, preload, &self.frames);
            }
        }
        values.resize(scene.emitters.len());
        values.reconcile_physical_positions(scene);
        let previous = self.begin_time(time_seconds);
        let mut count = 0;
        for (index, binding) in self.bindings.iter().enumerate() {
            let Some(i) = self.physical_indices[index] else {
                continue;
            };
            if !self.native_updated[i] {
                continue;
            }
            if let (Some(physical), Some(emitter), Some(value)) = (
                &binding.physical,
                scene.emitters.get(index),
                values.emitters.get_mut(index),
            ) {
                self.physical[i].decode_native(physical, emitter, value, previous, time_seconds);
                count += 1;
            }
        }
        for (i, runtime) in self.physical.iter().enumerate() {
            if self.native_updated[i] {
                runtime.apply_position(values);
            }
        }
        values.apply_calibrated_motion(scene, 0.);
        self.last_time_seconds = Some(time_seconds);
        count
    }

    /// Read the show's patched 3D Points off the wire as well.
    #[must_use]
    pub fn with_position_points(mut self, points: Vec<PositionPointBinding>) -> Self {
        self.position_points = points;
        self
    }

    /// Logical universes this show actually reads, used to configure the receivers.
    pub fn required_universes(&self) -> Vec<u16> {
        let mut universes: Vec<u16> = self.readers.keys().copied().collect();
        if let Some(camera) = &self.external_camera {
            universes.extend(camera.universes.iter().copied());
        }
        for point in &self.position_points {
            universes.extend(point.universes.iter().copied());
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
        values.reconcile_physical_positions(scene);
        values.resize_physics(scene.physics_scenery.len());
        for (index, (binding, emitter)) in self.bindings.iter().zip(&scene.emitters).enumerate() {
            let value = &mut values.emitters[index];
            if let Some(runtime) = self.physical_indices[index].and_then(|i| self.physical.get(i)) {
                runtime.clear_retired_ownership(value);
            }
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
            for (bindings, wheels) in [
                (&binding.gobo_wheels, &mut value.gobo_wheels),
                (&binding.prism_wheels, &mut value.prism_wheels),
            ] {
                wheels.resize_with(bindings.len(), viz_scene::OpticalWheelValues::default);
                for (binding, wheel) in bindings.iter().zip(wheels) {
                    set_declared_rotation_default(
                        &mut wheel.rotation_motion,
                        binding.rotation.as_ref(),
                    );
                    set_wheel_default(&mut wheel.wheel_motion, binding.selection.as_ref());
                }
            }
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
        for runtime in &self.physical {
            runtime.apply_home(values);
        }
        values.apply_calibrated_motion(scene, 0.);
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
        let points_affected = self.position_points.iter().any(|point| {
            received
                .iter()
                .any(|frame| point.universes.contains(&frame.logical_universe))
        });
        for frame in received {
            self.frames.insert(frame.logical_universe, frame.slots);
            self.stale.insert(frame.logical_universe, frame.stale);
            self.newest_input_micros = self.newest_input_micros.max(frame.received_micros);
            if let Some(readers) = self.readers.get(&frame.logical_universe) {
                affected.extend_from_slice(readers);
            }
        }
        for physical in &mut self.physical {
            if physical.affected(received) {
                physical.update(&self.frames);
            }
        }
        affected.sort_unstable();
        affected.dedup();

        values.resize(scene.emitters.len());
        values.reconcile_physical_positions(scene);
        values.resize_physics(scene.physics_scenery.len());
        let previous_time = self.begin_time(time_seconds);
        for index in &affected {
            let Some(binding) = self.bindings.get(*index) else {
                continue;
            };
            let Some(emitter) = scene.emitters.get(*index) else {
                continue;
            };
            let mut value = values.emitters[*index].clone();
            SlotReader {
                frames: &self.frames,
                stale: &self.stale,
            }
            .decode_emitter(binding, emitter, &mut value, previous_time, time_seconds);
            if let Some(runtime) = self.physical_indices[*index].and_then(|i| self.physical.get(i))
                && let Some(physical) = &binding.physical
            {
                let dimmer = binding
                    .intensity
                    .as_ref()
                    .map(|c| c.normalised(&self.slots(c.logical_universe)));
                runtime.apply(physical, &mut value, dimmer, emitter);
            }
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
            if let Some(window) = &binding.physics_window {
                let frame = self.slots(window.logical_universe);
                if let Some(physics) = values.physics_frames.get_mut(window.body_index) {
                    physics.slots.clear();
                    physics.slots.extend(window.slots.iter().map(|slot| {
                        frame
                            .get(usize::from(*slot).saturating_sub(1))
                            .copied()
                            .unwrap_or(0)
                    }));
                }
            }
        }
        if camera_affected {
            self.decode_external_camera(values);
        }
        if points_affected {
            self.decode_position_points(values);
        }
        for physical in &self.physical {
            physical.apply_position(values);
        }
        values.apply_calibrated_motion(scene, 0.);
        self.last_time_seconds = Some(time_seconds);
        self.frame_counter += 1;
        values.frame = self.frame_counter;
        values.newest_input_micros = self.newest_input_micros;
        affected.len() + usize::from(camera_affected)
    }

    fn slots(&self, universe: u16) -> [u8; DMX_SLOTS] {
        self.frames
            .get(&universe)
            .copied()
            .unwrap_or([0; DMX_SLOTS])
    }
}

pub(crate) struct SlotReader<'a> {
    pub frames: &'a HashMap<u16, [u8; DMX_SLOTS]>,
    pub stale: &'a HashMap<u16, bool>,
}
impl SlotReader<'_> {
    fn slots(&self, universe: u16) -> [u8; DMX_SLOTS] {
        self.frames
            .get(&universe)
            .copied()
            .unwrap_or([0; DMX_SLOTS])
    }

    pub(crate) fn decode_emitter(
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
        value.uv_drive = read(&binding.colour.ultraviolet).unwrap_or(0.);
        value.source_primaries = [
            read(&binding.colour.red).unwrap_or(0.0),
            read(&binding.colour.green).unwrap_or(0.0),
            read(&binding.colour.blue).unwrap_or(0.0),
        ];

        // Additive colour is normalized to hue by the resolver, so its level must still
        // modulate an explicit dimmer. Otherwise RGB black becomes full-brightness white.
        value.intensity = match read(&binding.intensity) {
            Some(level) => level * colour.level,
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
        value.zoom = binding
            .zoom
            .as_ref()
            .map(|channel| channel.zoom_normalised(&self.slots(channel.logical_universe)))
            .unwrap_or(0.5);
        value.iris = read(&binding.iris).unwrap_or(0.0);
        value.frost = read(&binding.frost).unwrap_or(0.0);
        value.focus = read(&binding.focus).unwrap_or(0.5);
        for (bindings, wheels) in [
            (&binding.gobo_wheels, &mut value.gobo_wheels),
            (&binding.prism_wheels, &mut value.prism_wheels),
        ] {
            wheels.resize_with(bindings.len(), viz_scene::OpticalWheelValues::default);
            for (binding, wheel) in bindings.iter().zip(wheels) {
                wheel.position = read(&binding.selection).unwrap_or(0.0);
                wheel.rotation = read(&binding.rotation).unwrap_or(0.0);
                set_wheel_target(&mut wheel.wheel_motion, binding.selection.as_ref(), &reader);
                set_declared_rotation_target(
                    &mut wheel.rotation_motion,
                    binding.rotation.as_ref(),
                    &reader,
                );
            }
        }
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

        self.decode_cells(binding, value);
        value.stale = binding
            .universes
            .iter()
            .any(|universe| self.stale.get(universe).copied().unwrap_or(true));
    }

    fn decode_cells(&self, binding: &EmitterBinding, value: &mut EmitterValues) {
        if binding.cells.is_empty() {
            value.cells.clear();
            return;
        }
        // A repeated per-cell dimmer is not a fixture master. Only multiply an independently
        // bound master; each cell already carries its own colour level.
        let master = binding
            .intensity
            .as_ref()
            .filter(|master| {
                !binding.cells.iter().any(|cell| {
                    cell.intensity.as_ref().is_some_and(|channel| {
                        channel.logical_universe == master.logical_universe
                            && channel.slots == master.slots
                    })
                })
            })
            .map(|channel| channel.normalised(&self.slots(channel.logical_universe)))
            .unwrap_or(1.0);
        value.cells = binding
            .cells
            .iter()
            .enumerate()
            .map(|(index, cell)| {
                let mut decoded = self.decode_cell(cell, master);
                // Preserve the display-state fade tail while rebuilding decoded cell state each
                // DMX frame; otherwise fixtures such as blinders and pixel strips lose it.
                decoded.held_intensity = value
                    .cells
                    .get(index)
                    .map_or(0.0, |previous| previous.held_intensity);
                decoded
            })
            .collect();
    }

    fn decode_cell(&self, cell: &ColourBinding, master: f32) -> CellValue {
        let reader = |universe: u16| self.slots(universe);
        let colour = colour::resolve(cell, &reader);
        let cell_dimmer = cell
            .intensity
            .as_ref()
            .map(|channel| channel.normalised(&self.slots(channel.logical_universe)))
            .unwrap_or(1.0);
        CellValue {
            intensity: (cell_dimmer * colour.level * master).clamp(0.0, 1.0),
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
    } else {
        // Returning from a spin function to an unannotated index must stop the old spin.
        state.target = None;
        state.velocity_degrees_per_second = 0.0;
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
mod reference_objects;

#[cfg(test)]
mod tests_support {
    pub(super) use super::tests::{frame, scene};
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod strobe_and_laser_tests;
