//! Turn received universe frames into semantic emitter values.
//!
//! Only emitters bound to a universe that changed are re-decoded, and only changed semantic
//! parameters reach the render scene.

use crate::colour;
use crate::plan::{ColourBinding, EmitterBinding};
use std::collections::HashMap;
use viz_dmx::{DMX_SLOTS, UniverseFrame};
use viz_scene::{CellValue, EmitterKind, EmitterValues, Scene, SceneValues};

/// Holds the latest frame per logical universe and applies it to the emitter values.
pub struct Decoder {
    bindings: Vec<EmitterBinding>,
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
        let mut readers: HashMap<u16, Vec<usize>> = HashMap::new();
        for (index, binding) in bindings.iter().enumerate() {
            for universe in &binding.universes {
                readers.entry(*universe).or_default().push(index);
            }
        }
        Self {
            bindings,
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
        universes.sort_unstable();
        universes
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
            self.decode_emitter(
                binding,
                emitter.kind,
                &mut value,
                previous_time,
                time_seconds,
            );
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
        }
        self.last_time_seconds = Some(time_seconds);
        self.frame_counter += 1;
        values.frame = self.frame_counter;
        values.newest_input_micros = self.newest_input_micros;
        affected.len()
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
        kind: EmitterKind,
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
        value.zoom = read(&binding.zoom).unwrap_or(0.5);
        value.iris = read(&binding.iris).unwrap_or(0.0);
        value.frost = read(&binding.frost).unwrap_or(0.0);
        value.focus = read(&binding.focus).unwrap_or(0.5);
        value.gobo = read(&binding.gobo).unwrap_or(0.0);
        value.gobo_rotation = read(&binding.gobo_rotation).unwrap_or(0.0);
        value.prism = read(&binding.prism).unwrap_or(0.0);
        value.prism_rotation = read(&binding.prism_rotation).unwrap_or(0.0);
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

        if kind == EmitterKind::Atmosphere {
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
    use crate::plan::LaserWindow;

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
