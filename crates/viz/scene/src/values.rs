//! Live per-emitter values. These change at DMX rate and are kept separate from the structural
//! scene so a value frame never rebuilds geometry.

use crate::atmosphere::Atmosphere;
use crate::persistence::PersistencePreference;
use crate::scene::Scene;
use std::collections::HashMap;
use uuid::Uuid;

/// Latest decoded state for every emitter in the current scene, parallel to
/// [`crate::Scene::emitters`].
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct SceneValues {
    pub emitters: Vec<EmitterValues>,
    /// Scan paths, parallel to [`Self::emitters`]. Every entry exists; only laser emitters ever
    /// have points in theirs.
    pub laser_scans: Vec<LaserScan>,
    /// Declarative particle emitters, parallel to [`Self::emitters`].
    #[serde(default)]
    pub effect_frames: Vec<EffectFrame>,
    /// Atmosphere to render with. Renderer-owned rather than decoded: the application fills it in
    /// from its own haze setting before each frame.
    pub atmosphere: Atmosphere,
    /// Monotonic frame counter, incremented whenever any emitter value changed.
    pub frame: u64,
    /// Receive timestamp of the newest input frame folded into these values, in monotonic
    /// microseconds since the renderer started. Used for packet-to-visible latency.
    pub newest_input_micros: u64,
    /// Last decoded pose of the one dedicated external-Visualizer DMX camera.
    ///
    /// This is deliberately not part of [`crate::ViewConfiguration`]: embedded desk Stages use
    /// their own view and can never be taken over by a patched fixture. Removing the binding or
    /// losing its input leaves this value intact so the external Visualizer can hold its pose.
    #[serde(default)]
    pub external_camera: Option<ExternalCameraState>,
    /// Which fixtures the operator has selected.
    ///
    /// Live state rather than scene structure, which is why it sits here: a selection changes
    /// several times a second while an operator works and the rig it refers to does not change at
    /// all. The renderer draws it and never decides it — what is selected is the desk's, and a
    /// renderer holding its own idea of it would be a second answer to the one question an
    /// operator has to be able to trust.
    pub selected_fixtures: std::collections::HashSet<uuid::Uuid>,
}

/// Absolute external-camera state decoded from the transferable 17-slot fixture.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExternalCameraState {
    pub fixture_id: Uuid,
    pub instance_id: Uuid,
    pub position_metres: [f32; 3],
    pub yaw_degrees: f32,
    pub pitch_degrees: f32,
    pub roll_degrees: f32,
    pub focal_length_millimetres: f32,
    pub vertical_fov_degrees: f32,
    /// Whether the selected camera fixture still has a complete live patch binding.
    #[serde(default)]
    pub patched: bool,
    /// True when the newest authoritative universe frame is stale. The pose remains usable.
    pub stale: bool,
}

impl SceneValues {
    pub fn resize(&mut self, emitters: usize) {
        self.emitters.resize_with(emitters, EmitterValues::default);
        self.laser_scans.resize_with(emitters, LaserScan::default);
        self.effect_frames
            .resize_with(emitters, EffectFrame::default);
    }

    /// Advance persistence of vision by `elapsed` seconds.
    ///
    /// Driven by the display rather than by the decoder, and deliberately so: a strobe that has
    /// stopped sending, a universe that has gone quiet, and a desk running slower than the screen
    /// all still have to fade out in real time rather than freezing at whatever the last packet
    /// said. Calling this with the actual time between presented frames is the whole contract.
    pub fn apply_persistence(&mut self, preference: &PersistencePreference, elapsed: f32) {
        for emitter in &mut self.emitters {
            let current = emitter.visible_intensity();
            emitter.held_intensity = preference.hold(emitter.held_intensity, current, elapsed);
            for cell in &mut emitter.cells {
                cell.held_intensity = preference.hold(cell.held_intensity, cell.intensity, elapsed);
            }
        }
    }

    /// Advance visual fixture mechanics on the display clock.
    ///
    /// The decoded values remain the desk's immediate targets. Only these parallel physical
    /// states are rate limited, so output and network semantics are never delayed by a renderer.
    pub fn apply_physical_motion(&mut self, elapsed: f32) {
        for emitter in &mut self.emitters {
            emitter.pan_motion.advance(elapsed);
            emitter.tilt_motion.advance(elapsed);
            emitter.gobo_rotation_motion.advance(elapsed);
            emitter.prism_rotation_motion.advance(elapsed);
            emitter.gobo_wheel_motion.advance(elapsed);
            if let Some(index) = emitter.gobo_wheel_motion.visible_slot() {
                emitter.gobo =
                    (index as f32 + 0.5) / emitter.gobo_wheel_motion.slot_count.max(1) as f32;
            }
            emitter.colour_wheel_motion.advance(elapsed);
            if let Some(index) = emitter.colour_wheel_motion.visible_slot()
                && let Some(colour) = emitter.colour_wheel_palette.get(index)
            {
                emitter.colour = *colour;
            }
        }
    }

    /// Whether another display-clock tick can change the picture without a new input frame.
    ///
    /// This is deliberately about visible temporal state, not merely about having authored
    /// motion metadata. A position target which has settled is static; a velocity target is
    /// time-driven even while it is still accelerating from zero.
    pub fn is_time_driven(&self, persistence: &PersistencePreference) -> bool {
        self.emitters.iter().any(|emitter| {
            (persistence.is_active()
                && (emitter.held_intensity > emitter.visible_intensity() + f32::EPSILON
                    || emitter
                        .cells
                        .iter()
                        .any(|cell| cell.held_intensity > cell.intensity + f32::EPSILON)))
                || emitter.pan_motion.is_moving()
                || emitter.tilt_motion.is_moving()
                || emitter.gobo_rotation_motion.is_moving()
                || emitter.prism_rotation_motion.is_moving()
                || emitter.gobo_wheel_motion.motion.is_moving()
                || emitter.colour_wheel_motion.motion.is_moving()
        }) || self.laser_scans.iter().any(|scan| !scan.points.is_empty())
            || self
                .effect_frames
                .iter()
                .any(|frame| !frame.emitters.is_empty())
    }

    /// Carry the values across a structural change to the scene.
    ///
    /// Emitters are addressed by index, and a fixture added, removed or repatched moves every
    /// index after it. A head is therefore matched by what it actually is — the physical instance
    /// it belongs to and which head of that instance it is — so a rig edited during a show keeps
    /// its look instead of going black for however long the desk holds the same frame. A head
    /// that is genuinely new starts at its defaults.
    pub fn carry_over(&mut self, previous: &Scene, next: &Scene) {
        let mut held: HashMap<(Uuid, u16), (EmitterValues, LaserScan, EffectFrame)> =
            HashMap::with_capacity(previous.emitters.len());
        for (index, emitter) in previous.emitters.iter().enumerate() {
            let Some(fixture) = previous.fixtures.get(emitter.fixture_index as usize) else {
                continue;
            };
            let Some(values) = self.emitters.get(index) else {
                continue;
            };
            let scan = self.laser_scans.get(index).cloned().unwrap_or_default();
            let effect = self.effect_frames.get(index).cloned().unwrap_or_default();
            held.insert(
                (fixture.instance_id, emitter.head_index),
                (values.clone(), scan, effect),
            );
        }
        let carried: Vec<_> = next
            .emitters
            .iter()
            .map(|emitter| {
                next.fixtures
                    .get(emitter.fixture_index as usize)
                    .and_then(|fixture| held.remove(&(fixture.instance_id, emitter.head_index)))
                    .unwrap_or_default()
            })
            .collect();
        self.emitters = carried.iter().map(|value| value.0.clone()).collect();
        self.laser_scans = carried.iter().map(|value| value.1.clone()).collect();
        self.effect_frames = carried.into_iter().map(|value| value.2).collect();
    }
}

/// Validated output of one Effect fixture's script for one frame.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct EffectFrame {
    pub version: u16,
    /// Authoritative effect timeline used for deterministic particle placement.
    pub timeline_seconds: f32,
    pub emitters: Vec<ParticleEmitter>,
    /// Exact raw fixture footprint supplied to the script.
    pub slots: Vec<u8>,
    pub fault: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParticleFamily {
    Flame,
    Spark,
    /// Reserved integration seam for TL-107; version 1 validation rejects this family.
    Debris,
}

/// One bounded local-space source returned by an Effect script.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ParticleEmitter {
    pub family: ParticleFamily,
    pub origin: [f32; 3],
    pub direction: [f32; 3],
    pub width_metres: f32,
    pub reach_metres: f32,
    pub intensity: f32,
    pub density: f32,
    pub lifetime_seconds: f32,
    pub colour: [f32; 3],
    pub trigger: ParticleTrigger,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParticleTrigger {
    #[default]
    Off,
    Trigger,
    Hold,
    Release,
    Retrigger,
}

/// One emitter's decoded semantic parameters.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EmitterValues {
    /// Dimmer after shutter and strobe gating, `0..=1`.
    pub intensity: f32,
    /// Linear RGB, `0..=1` per component.
    pub colour: [f32; 3],
    /// Pan parameter `0..=1` mapped through the emitter's pan axis.
    pub pan: f32,
    /// Simulated physical Pan. Its authored zero is the geometry node's local transform.
    #[serde(default)]
    pub pan_motion: PhysicalMotionState,
    /// Tilt parameter `0..=1` mapped through the emitter's tilt axis.
    pub tilt: f32,
    /// Simulated physical Tilt. Its authored zero is the geometry node's local transform.
    #[serde(default)]
    pub tilt_motion: PhysicalMotionState,
    /// Zoom parameter `0..=1`; `0` is the narrow beam angle, `1` the wide field angle.
    pub zoom: f32,
    /// Iris closure `0..=1`; `0` is fully open.
    pub iris: f32,
    /// Frost/diffusion `0..=1`.
    pub frost: f32,
    /// Focus `0..=1`; `0.5` is a sharp field edge and either extreme softens it.
    pub focus: f32,
    /// Gobo wheel position `0..=1`. `0` is the open slot.
    pub gobo: f32,
    /// Physical ordered-slot traversal for the first gobo wheel.
    #[serde(default)]
    pub gobo_wheel_motion: WheelMotionState,
    /// Gobo rotation as a signed rate, `-1..=1`, or a fixed index position.
    pub gobo_rotation: f32,
    #[serde(default)]
    pub gobo_rotation_motion: PhysicalMotionState,
    /// Prism wheel position `0..=1`. `0` is out of the beam.
    pub prism: f32,
    pub prism_rotation: f32,
    #[serde(default)]
    pub prism_rotation_motion: PhysicalMotionState,
    /// Physical ordered-slot traversal for the first colour wheel.
    #[serde(default)]
    pub colour_wheel_motion: WheelMotionState,
    #[serde(default)]
    pub colour_wheel_palette: Vec<[f32; 3]>,
    /// Framing-shutter blade insertions `0..=1`, `0` fully open.
    pub shaper_blades: [f32; 4],
    /// Blade rotations in physical degrees. Only values backed by a live profile attribute are
    /// read; installed/static fallbacks stay in the structural scene.
    pub shaper_blade_angles_degrees: [f32; 4],
    /// Rotation of the whole shutter module, `0..=1` of a full turn.
    pub shaper_rotation: f32,
    /// Rotation of the whole shutter module in physical degrees.
    pub shaper_rotation_degrees: f32,
    /// Strobe rate in hertz. `0` means no strobe.
    pub strobe_hz: f32,
    /// Shutter gate after strobe evaluation, `0..=1`.
    pub shutter: f32,
    /// What an observer still sees, after persistence of vision.
    ///
    /// Never below [`Self::visible_intensity`], and above it for as long as a light that has just
    /// gone dark is still being seen. This is the level the renderer draws with; the decoded
    /// fields above remain the desk's literal instruction, which is what the diagnostics and the
    /// fixture readouts have to keep showing.
    pub held_intensity: f32,
    /// Per-cell values for multi-cell emitters. Empty means the emitter is uniform.
    pub cells: Vec<CellValue>,
    /// Whether the owning universe is currently stale.
    pub stale: bool,
}

impl Default for EmitterValues {
    fn default() -> Self {
        Self {
            intensity: 0.0,
            colour: [1.0, 1.0, 1.0],
            pan: 0.5,
            pan_motion: PhysicalMotionState::default(),
            tilt: 0.5,
            tilt_motion: PhysicalMotionState::default(),
            zoom: 0.5,
            iris: 0.0,
            frost: 0.0,
            focus: 0.5,
            gobo: 0.0,
            gobo_wheel_motion: WheelMotionState::default(),
            gobo_rotation: 0.0,
            gobo_rotation_motion: PhysicalMotionState::default(),
            prism: 0.0,
            prism_rotation: 0.0,
            prism_rotation_motion: PhysicalMotionState::default(),
            colour_wheel_motion: WheelMotionState::default(),
            colour_wheel_palette: Vec::new(),
            shaper_blades: [0.0; 4],
            shaper_blade_angles_degrees: [0.0; 4],
            shaper_rotation: 0.0,
            shaper_rotation_degrees: 0.0,
            strobe_hz: 0.0,
            shutter: 1.0,
            held_intensity: 0.0,
            cells: Vec::new(),
            stale: false,
        }
    }
}

/// A Visualizer-only physical target decoded from one DMX function range.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PhysicalMotionTarget {
    Position {
        degrees: f32,
        max_speed: f32,
        acceleration: f32,
        deceleration: f32,
    },
    Velocity {
        degrees_per_second: f32,
        acceleration: f32,
        deceleration: f32,
    },
}

/// Current rendered position and velocity for one rotating physical attribute.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PhysicalMotionState {
    pub position_degrees: f32,
    pub velocity_degrees_per_second: f32,
    #[serde(default)]
    pub target: Option<PhysicalMotionTarget>,
}

/// Ordered wheel movement. Slot centres are evenly spaced around one physical revolution and
/// follow the functions' raw-DMX order, so movement from slot 1 to slot 3 necessarily crosses 2.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WheelMotionState {
    pub motion: PhysicalMotionState,
    pub slot_count: u16,
}

impl WheelMotionState {
    pub fn set_target(
        &mut self,
        slot_index: usize,
        slot_count: usize,
        max_speed: f32,
        acceleration: f32,
        deceleration: f32,
    ) {
        self.slot_count = slot_count.min(u16::MAX as usize) as u16;
        if self.slot_count == 0 {
            self.motion.target = None;
            return;
        }
        let degrees_per_slot = 360.0 / f32::from(self.slot_count);
        self.motion.set_target(PhysicalMotionTarget::Position {
            degrees: slot_index as f32 * degrees_per_slot,
            max_speed,
            acceleration,
            deceleration,
        });
    }

    pub fn advance(&mut self, elapsed: f32) {
        self.motion.advance(elapsed);
    }

    pub fn visible_slot(&self) -> Option<usize> {
        if self.slot_count == 0 || self.motion.target.is_none() {
            return None;
        }
        let degrees_per_slot = 360.0 / f32::from(self.slot_count);
        Some(
            (self.motion.position_degrees / degrees_per_slot)
                .round()
                .clamp(0.0, f32::from(self.slot_count - 1)) as usize,
        )
    }
}

impl PhysicalMotionState {
    pub fn set_target(&mut self, target: PhysicalMotionTarget) {
        self.target = Some(target);
    }

    /// Whether advancing the display clock can still alter this state.
    pub fn is_moving(&self) -> bool {
        match self.target {
            Some(PhysicalMotionTarget::Position { degrees, .. }) => {
                (degrees - self.position_degrees).abs() > 1.0e-4
                    || self.velocity_degrees_per_second.abs() > 1.0e-3
            }
            Some(PhysicalMotionTarget::Velocity {
                degrees_per_second, ..
            }) => {
                degrees_per_second.abs() > f32::EPSILON
                    || self.velocity_degrees_per_second.abs() > 1.0e-3
            }
            None => false,
        }
    }

    pub fn advance(&mut self, elapsed: f32) {
        let elapsed = elapsed.clamp(0.0, 0.25);
        if elapsed <= 0.0 {
            return;
        }
        match self.target {
            Some(PhysicalMotionTarget::Position {
                degrees,
                max_speed,
                acceleration,
                deceleration,
            }) => self.advance_position(degrees, max_speed, acceleration, deceleration, elapsed),
            Some(PhysicalMotionTarget::Velocity {
                degrees_per_second,
                acceleration,
                deceleration,
            }) => {
                let reversing = self.velocity_degrees_per_second.abs() > f32::EPSILON
                    && self.velocity_degrees_per_second.signum() != degrees_per_second.signum();
                let rate = if reversing
                    || degrees_per_second.abs() < self.velocity_degrees_per_second.abs()
                {
                    deceleration
                } else {
                    acceleration
                };
                self.velocity_degrees_per_second = approach(
                    self.velocity_degrees_per_second,
                    degrees_per_second,
                    rate * elapsed,
                );
                self.position_degrees += self.velocity_degrees_per_second * elapsed;
            }
            None => {}
        }
    }

    fn advance_position(
        &mut self,
        target: f32,
        max_speed: f32,
        acceleration: f32,
        deceleration: f32,
        elapsed: f32,
    ) {
        let error = target - self.position_degrees;
        if error.abs() <= 1.0e-4 && self.velocity_degrees_per_second.abs() <= 1.0e-3 {
            self.position_degrees = target;
            self.velocity_degrees_per_second = 0.0;
            return;
        }
        let direction = error.signum();
        let moving_toward = self.velocity_degrees_per_second.signum() == direction;
        let stopping_distance =
            self.velocity_degrees_per_second.powi(2) / (2.0 * deceleration.max(f32::EPSILON));
        let desired_velocity = if !moving_toward || stopping_distance >= error.abs() {
            0.0
        } else {
            direction * max_speed
        };
        let rate = if desired_velocity.abs() < self.velocity_degrees_per_second.abs() {
            deceleration
        } else {
            acceleration
        };
        self.velocity_degrees_per_second = approach(
            self.velocity_degrees_per_second,
            desired_velocity,
            rate * elapsed,
        )
        .clamp(-max_speed, max_speed);
        let next = self.position_degrees + self.velocity_degrees_per_second * elapsed;
        if (target - next).signum() != direction {
            self.position_degrees = target;
            self.velocity_degrees_per_second = 0.0;
        } else {
            self.position_degrees = next;
        }
    }
}

fn approach(current: f32, target: f32, maximum_delta: f32) -> f32 {
    let delta = target - current;
    if delta.abs() <= maximum_delta {
        target
    } else {
        current + delta.signum() * maximum_delta
    }
}

impl EmitterValues {
    /// Which gobo slot is in the beam, counting the open slot as zero.
    ///
    /// A wheel channel is a continuous range in DMX and a set of discrete slots in the optics.
    /// Without slot tables in the profile the wheel is divided evenly, which puts a pattern in
    /// the beam at the right point in the operator's fade even if its artwork is not the
    /// manufacturer's.
    pub fn gobo_slot(&self, slots: u32) -> u32 {
        if self.gobo <= 0.004 || slots <= 1 {
            return 0;
        }
        let index = (self.gobo * slots as f32).floor() as u32;
        index.min(slots - 1)
    }

    /// How many beam copies the prism makes, `0` when it is out of the beam.
    pub fn prism_facets(&self) -> u32 {
        if self.prism <= 0.02 {
            return 0;
        }
        // Prisms are usually three, five, or eight facets; the wheel selects between them.
        const FACETS: [u32; 3] = [3, 5, 8];
        let index = ((self.prism * FACETS.len() as f32) as usize).min(FACETS.len() - 1);
        FACETS[index]
    }

    /// Effective visible intensity including the shutter gate.
    pub fn visible_intensity(&self) -> f32 {
        (self.intensity * self.shutter).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{
        BodyKind, EmitterInstance, EmitterKind, EmitterLayoutCells, EmitterOptics, FixtureBody,
        FixtureInstance,
    };
    use glam::Vec3;

    fn fixture(instance_id: Uuid, name: &str) -> FixtureInstance {
        FixtureInstance {
            instance_id,
            fixture_id: instance_id,
            name: name.into(),
            number: None,
            position: Vec3::ZERO,
            rotation_degrees: Vec3::ZERO,
            bracket_degrees: 0.0,
            shaper_degrees: None,
            installed_colour: [1.0; 3],
            installed_shaper_angles_degrees: [0.0; 4],
            body: FixtureBody {
                size: Vec3::splat(0.3),
                kind: BodyKind::Lantern,
            },
            patched: true,
            address: None,
            model: None,
            fallback: None,
        }
    }

    fn emitter(fixture_index: u32, head_index: u16) -> EmitterInstance {
        EmitterInstance {
            fixture_index,
            head_index,
            label: "head".into(),
            local_origin: Vec3::ZERO,
            tilt_pivot: Vec3::ZERO,
            local_orientation_degrees: Vec3::ZERO,
            pan: None,
            tilt: None,
            beam_angle_degrees: 10.0,
            field_angle_degrees: 20.0,
            optics: EmitterOptics::default(),
            kind: EmitterKind::Beam,
            cells: EmitterLayoutCells::single(),
            laser: None,
            effect: None,
            live_shaper_angle_roles: [false; 4],
            shaper_roles: [false; 4],
            live_shaper_rotation_role: false,
        }
    }

    /// A rig of exactly these fixtures and these heads, which is all these tests need of a scene.
    fn rig(fixtures: Vec<FixtureInstance>, emitters: Vec<EmitterInstance>) -> Scene {
        Scene {
            fixtures,
            emitters,
            ..Scene::default()
        }
    }

    /// Two fixtures, each with one head, and the first one is removed.
    #[test]
    fn a_head_keeps_its_level_when_the_fixture_before_it_is_removed() {
        let first = Uuid::from_u128(1);
        let second = Uuid::from_u128(2);
        let previous = rig(
            vec![fixture(first, "one"), fixture(second, "two")],
            vec![emitter(0, 0), emitter(1, 0)],
        );

        let mut values = SceneValues::default();
        values.resize(2);
        values.emitters[0].intensity = 0.25;
        values.emitters[1].intensity = 0.8;
        values.emitters[1].colour = [1.0, 0.0, 0.0];

        let next = rig(vec![fixture(second, "two")], vec![emitter(0, 0)]);

        values.carry_over(&previous, &next);
        assert_eq!(values.emitters.len(), 1);
        assert_eq!(values.emitters[0].intensity, 0.8);
        assert_eq!(values.emitters[0].colour, [1.0, 0.0, 0.0]);
    }

    #[test]
    fn a_newly_patched_head_starts_at_its_defaults() {
        let known = Uuid::from_u128(1);
        let added = Uuid::from_u128(9);
        let previous = rig(vec![fixture(known, "one")], vec![emitter(0, 0)]);
        let mut values = SceneValues::default();
        values.resize(1);
        values.emitters[0].intensity = 0.5;

        let next = rig(
            vec![fixture(added, "new"), fixture(known, "one")],
            vec![emitter(0, 0), emitter(1, 0)],
        );

        values.carry_over(&previous, &next);
        assert_eq!(values.emitters.len(), 2);
        assert_eq!(values.emitters[0].intensity, 0.0);
        assert_eq!(values.emitters[1].intensity, 0.5);
    }

    #[test]
    fn static_values_do_not_request_display_clock_frames() {
        let mut values = SceneValues::default();
        values.resize(1);
        assert!(!values.is_time_driven(&PersistencePreference::default()));
    }

    #[test]
    fn persistence_and_unsettled_motion_request_display_clock_frames() {
        let mut values = SceneValues::default();
        values.resize(1);
        values.emitters[0].held_intensity = 1.0;
        assert!(values.is_time_driven(&PersistencePreference::default()));
        values.emitters[0].held_intensity = 0.0;
        values.emitters[0]
            .pan_motion
            .set_target(PhysicalMotionTarget::Position {
                degrees: 90.0,
                max_speed: 180.0,
                acceleration: 360.0,
                deceleration: 360.0,
            });
        assert!(values.is_time_driven(&PersistencePreference::default()));
    }

    #[test]
    fn a_settled_position_target_is_static() {
        let mut values = SceneValues::default();
        values.resize(1);
        values.emitters[0].pan_motion = PhysicalMotionState {
            position_degrees: 90.0,
            velocity_degrees_per_second: 0.0,
            target: Some(PhysicalMotionTarget::Position {
                degrees: 90.0,
                max_speed: 180.0,
                acceleration: 360.0,
                deceleration: 360.0,
            }),
        };
        assert!(!values.is_time_driven(&PersistencePreference::default()));
    }

    /// Heads of one multi-head fixture are told apart by their head index, not by their order.
    #[test]
    fn each_head_of_a_fixture_keeps_its_own_value() {
        let bar = Uuid::from_u128(7);
        let previous = rig(
            vec![fixture(bar, "bar")],
            vec![emitter(0, 0), emitter(0, 1), emitter(0, 2)],
        );
        let mut values = SceneValues::default();
        values.resize(3);
        for (index, emitter) in values.emitters.iter_mut().enumerate() {
            emitter.intensity = index as f32 / 10.0;
        }

        // The middle head is gone: a mode change that drops a cell.
        let next = rig(
            vec![fixture(bar, "bar")],
            vec![emitter(0, 0), emitter(0, 2)],
        );

        values.carry_over(&previous, &next);
        assert_eq!(values.emitters[0].intensity, 0.0);
        assert_eq!(values.emitters[1].intensity, 0.2);
    }

    #[test]
    fn absolute_motion_preserves_multi_turn_targets_and_respects_limits() {
        let mut motion = PhysicalMotionState::default();
        motion.set_target(PhysicalMotionTarget::Position {
            degrees: 630.0,
            max_speed: 180.0,
            acceleration: 360.0,
            deceleration: 360.0,
        });
        motion.advance(0.25);
        assert!(motion.position_degrees > 0.0 && motion.position_degrees < 630.0);
        assert!(motion.velocity_degrees_per_second <= 180.0);
        for _ in 0..200 {
            motion.advance(0.1);
        }
        assert!((motion.position_degrees - 630.0).abs() < 0.001);
        assert_eq!(motion.velocity_degrees_per_second, 0.0);
    }

    #[test]
    fn a_replaced_target_is_followed_without_teleporting() {
        let mut motion = PhysicalMotionState::default();
        motion.set_target(PhysicalMotionTarget::Position {
            degrees: 90.0,
            max_speed: 90.0,
            acceleration: 180.0,
            deceleration: 180.0,
        });
        motion.advance(0.25);
        let before = motion.position_degrees;
        motion.set_target(PhysicalMotionTarget::Position {
            degrees: -90.0,
            max_speed: 90.0,
            acceleration: 180.0,
            deceleration: 180.0,
        });
        assert_eq!(motion.position_degrees, before);
        motion.advance(0.25);
        assert!(motion.position_degrees > -90.0);
    }

    #[test]
    fn endless_motion_accelerates_to_an_authored_signed_velocity() {
        let mut motion = PhysicalMotionState::default();
        motion.set_target(PhysicalMotionTarget::Velocity {
            degrees_per_second: -120.0,
            acceleration: 240.0,
            deceleration: 360.0,
        });
        motion.advance(0.25);
        assert_eq!(motion.velocity_degrees_per_second, -60.0);
        assert_eq!(motion.position_degrees, -15.0);
        motion.advance(0.25);
        assert_eq!(motion.velocity_degrees_per_second, -120.0);
    }

    #[test]
    fn an_ordered_wheel_crosses_intermediate_slots() {
        let mut wheel = WheelMotionState::default();
        wheel.set_target(3, 4, 180.0, 720.0, 720.0);
        let mut visited = Vec::new();
        for _ in 0..20 {
            wheel.advance(0.1);
            let slot = wheel.visible_slot().unwrap();
            if visited.last() != Some(&slot) {
                visited.push(slot);
            }
        }
        assert!(visited.windows(2).all(|pair| pair[0] <= pair[1]));
        assert!(visited.contains(&1));
        assert!(visited.contains(&2));
        assert_eq!(visited.last(), Some(&3));
    }
}

/// One pixel cell's value.
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct CellValue {
    pub intensity: f32,
    pub colour: [f32; 3],
    /// This cell's level after persistence of vision. A strobing blinder or a chased pixel strip
    /// is exactly the case that needs it, and those are per-cell rather than per-head.
    pub held_intensity: f32,
}

impl Default for CellValue {
    fn default() -> Self {
        Self {
            intensity: 0.0,
            colour: [1.0, 1.0, 1.0],
            held_intensity: 0.0,
        }
    }
}

/// One frame of a laser's scan engine: the path the beam takes, in order.
///
/// Kept beside the emitter values rather than inside them because it is the one value that is a
/// list rather than a number, and the decode path clones an emitter's values on every DMX frame.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct LaserScan {
    /// The path, in scan order. Fewer than two points draws nothing.
    pub points: Vec<ScanPoint>,
    /// Scanner speed for this frame, points per second. Taken from the fixture unless the script
    /// named its own — a script that blanks half its points is moving faster than its rated speed
    /// through the part that is actually drawn.
    pub points_per_second: f32,
    /// The fixture's own DMX slots this frame, in patch order from its start address.
    ///
    /// Captured by the decoder and consumed by whoever runs the scan engine. It lives here rather
    /// than being read straight from a universe frame because the engine runs on the render thread
    /// once per displayed frame, while decoding happens on whatever thread a packet arrives on and
    /// as often as the desk sends.
    pub slots: Vec<u8>,
    /// Why this laser is not projecting, when it is not. A script that fails to compile, throws,
    /// or overruns its time budget leaves the laser dark and says so here; nothing about a laser
    /// that has stopped working may be silent.
    pub fault: Option<String>,
}

impl LaserScan {
    /// How long one complete pass of this path takes, in seconds.
    pub fn scan_seconds(&self) -> f32 {
        if self.points.len() < 2 || self.points_per_second <= 0.0 {
            return 0.0;
        }
        self.points.len() as f32 / self.points_per_second
    }
}

/// One control point on a laser's scan path.
#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct ScanPoint {
    /// Deflection across the scanner's X axis, `-1..=1` of its half scan angle.
    pub x: f32,
    /// Deflection across the scanner's Y axis, `-1..=1`.
    pub y: f32,
    /// Linear RGB at this point, `0..=1`. All zero is a blanked move, which is how a scan engine
    /// jumps between figures without drawing the join.
    pub colour: [f32; 3],
    /// The share of one complete scan spent reaching this point, `0..=1`.
    ///
    /// This is the script's percentage divided by a hundred and normalised across the path. It is
    /// both a timing and a brightness: a point the scanner dwells on receives proportionally more
    /// of the frame's light, which is why the corners of a real laser figure are the bright parts.
    pub dwell: f32,
}
