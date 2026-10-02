//! One compiled optical evaluator and reused workspace per physical instance.
use crate::{ChannelRef, EmitterBinding, PatchedFixture, PhysicalInstance};
use light_fixture::forward::{
    AxisForwardCommand, ColorForwardResult, CompiledColorForward, CompiledOpticsForward,
    CompiledPositionForward, OpticsForwardResult, OpticsForwardStatus, PositionInstallation,
};
use light_fixture::{FixtureMode, OpeningConvention, PhysicalDataQuality};
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;
use viz_dmx::DMX_SLOTS;
use viz_scene::{EmitterValues, PhysicalColorState, PhysicalOpticsState};

#[derive(Clone, Debug)]
pub struct PhysicalEmitterBinding {
    pub(crate) plan: Arc<PhysicalPlan>,
    native_binding: Box<EmitterBinding>,
    color: Option<usize>,
    optics: Option<usize>,
}
#[derive(Clone, Debug)]
pub(crate) struct PhysicalPlan {
    pub instance_id: Uuid,
    pub fixture_id: Uuid,
    pub native_identity: String,
    native_channels: Box<[ChannelRef]>,
    channels: Box<[Option<ChannelRef>]>,
    universes: Box<[u16]>,
    color: Option<CompiledColorForward>,
    color_declared: bool,
    optics: Option<CompiledOpticsForward>,
    position: Option<CompiledPositionForward>,
    position_index: Option<usize>,
    position_declared: bool,
}
pub(crate) struct PhysicalRuntime {
    pub plan: Arc<PhysicalPlan>,
    raw: Box<[u32]>,
    colors: Vec<ColorForwardResult>,
    optics: Vec<OpticsForwardResult>,
    axes: Vec<AxisForwardCommand>,
    available: bool,
    channel_available: Box<[bool]>,
    native_frames: HashMap<u16, [u8; DMX_SLOTS]>,
    native_stale: HashMap<u16, bool>,
}
impl PhysicalRuntime {
    pub fn new(plan: Arc<PhysicalPlan>) -> Self {
        let mut output = Self {
            raw: vec![0; plan.channels.len()].into_boxed_slice(),
            colors: plan
                .color
                .as_ref()
                .map_or_else(Vec::new, |m| m.create_output()),
            optics: plan
                .optics
                .as_ref()
                .map_or_else(Vec::new, |m| m.create_output()),
            axes: plan
                .position
                .as_ref()
                .map_or_else(Vec::new, |m| m.create_commands()),
            available: false,
            channel_available: vec![true; plan.channels.len()].into_boxed_slice(),
            native_frames: plan
                .native_channels
                .iter()
                .map(|c| (c.logical_universe, [0; DMX_SLOTS]))
                .collect(),
            native_stale: plan
                .native_channels
                .iter()
                .map(|c| (c.logical_universe, false))
                .collect(),
            plan,
        };
        for (raw, channel) in output.raw.iter_mut().zip(&output.plan.native_channels) {
            *raw = channel.default_raw;
        }
        output.available = true;
        output.evaluate();
        output
    }
    pub fn affected(&self, frames: &[viz_dmx::UniverseFrame]) -> bool {
        frames.iter().any(|f| {
            self.plan
                .universes
                .binary_search(&f.logical_universe)
                .is_ok()
        })
    }
    pub fn update(&mut self, frames: &HashMap<u16, [u8; DMX_SLOTS]>) {
        self.available = true;
        for (i, (out, reader)) in self.raw.iter_mut().zip(&self.plan.channels).enumerate() {
            self.channel_available[i] = true;
            match reader
                .as_ref()
                .and_then(|r| frames.get(&r.logical_universe).map(|f| (r, f)))
            {
                Some((reader, frame)) => *out = reader.raw(frame),
                None => {
                    self.channel_available[i] = false;
                    *out = 0;
                }
            }
        }
        self.evaluate();
    }
    fn evaluate(&mut self) {
        if let Some(model) = &self.plan.color {
            self.available &= model.evaluate(&self.raw, &mut self.colors).is_ok();
        }
        if let Some(model) = &self.plan.optics {
            self.available &= model.evaluate(&self.raw, &mut self.optics).is_ok();
            for (i, result) in self.optics.iter_mut().enumerate() {
                if !model.inputs_available(i, true, &self.channel_available) {
                    result.focus = None;
                    result.focus_status = OpticsForwardStatus::UnknownFunction;
                }
                if !model.inputs_available(i, false, &self.channel_available) {
                    result.zoom = None;
                    result.zoom_status = OpticsForwardStatus::UnknownFunction;
                }
            }
        }
        if let Some(model) = &self.plan.position {
            self.available &= model.decode_commands(&self.raw, &mut self.axes).is_ok();
        }
    }
    /// Live fills only channels with no actual output destination. Preload replaces explicitly
    /// owned values. Real patched slots remain authoritative, including manual DMX overrides.
    pub fn update_native(
        &mut self,
        raw: &[u32],
        owned: Option<&[bool]>,
        preload: bool,
        live: &HashMap<u16, [u8; DMX_SLOTS]>,
    ) -> bool {
        if raw.len() != self.raw.len()
            || owned.is_some_and(|mask| mask.len() != raw.len())
            || raw
                .iter()
                .zip(&self.plan.native_channels)
                .any(|(&v, c)| v > c.max_raw)
        {
            return false;
        }
        self.available = true;
        for (i, &input) in raw.iter().enumerate() {
            if preload && owned.is_some_and(|mask| !mask[i]) {
                continue;
            }
            self.channel_available[i] = true;
            self.raw[i] = if !preload {
                match &self.plan.channels[i] {
                    Some(reader) => match live.get(&reader.logical_universe) {
                        Some(frame) => reader.raw(frame),
                        None => {
                            self.channel_available[i] = false;
                            0
                        }
                    },
                    None => input,
                }
            } else {
                input
            };
            let reader = &self.plan.native_channels[i];
            let frame = self
                .native_frames
                .get_mut(&reader.logical_universe)
                .unwrap();
            for (byte, slot) in reader.slots.iter().rev().enumerate() {
                frame[usize::from(*slot - 1)] = (self.raw[i] >> (byte * 8)) as u8;
            }
        }
        self.evaluate();
        true
    }
    pub fn decode_native(
        &self,
        binding: &PhysicalEmitterBinding,
        emitter: &viz_scene::EmitterInstance,
        value: &mut EmitterValues,
        previous: f32,
        now: f32,
    ) {
        let native = &binding.native_binding;
        crate::decode::SlotReader {
            frames: &self.native_frames,
            stale: &self.native_stale,
        }
        .decode_emitter(native, emitter, value, previous, now);
        let dimmer = native
            .intensity
            .as_ref()
            .map(|c| c.normalised(&self.native_frames[&c.logical_universe]));
        self.apply(binding, value, dimmer, emitter);
    }
    pub fn clear_retired_ownership(&self, value: &mut EmitterValues) {
        if !self.plan.position_declared {
            value.physical_pose = None;
        }
        if !self.plan.color_declared {
            value.physical_color = None;
        }
    }
    pub fn apply_home(&self, values: &mut viz_scene::SceneValues) {
        self.apply_position_inner(values, true);
    }
    pub fn apply_position(&self, values: &mut viz_scene::SceneValues) {
        self.apply_position_inner(values, false);
    }
    fn apply_position_inner(&self, values: &mut viz_scene::SceneValues, only_new: bool) {
        let Some(index) = self.plan.position_index else {
            return;
        };
        let Some(state) = values.physical_positions.get_mut(index) else {
            return;
        };
        for (i, (state, command)) in state.axes.iter_mut().zip(&self.axes).enumerate() {
            if only_new && state.has_position {
                continue;
            }
            let known = self.available
                && !command.conflict()
                && self
                    .plan
                    .position
                    .as_ref()
                    .is_some_and(|m| m.inputs_available(i, &self.channel_available));
            let physical = if known {
                command.absolute.or(command.velocity)
            } else {
                None
            };
            state.known = physical.is_some();
            let Some(value) = physical else {
                state.motion.target = None;
                state.motion.velocity_degrees_per_second = 0.;
                continue;
            };
            state.has_position = true;
            state.nominal_limits = value.limits.acceleration.is_none()
                || value.limits.deceleration.is_none()
                || (command.absolute.is_some() && value.limits.speed.is_none());
            let acceleration = value
                .limits
                .acceleration
                .unwrap_or(f64::from(crate::binding::FALLBACK_ANGULAR_ACCELERATION))
                as f32;
            let deceleration = value
                .limits
                .deceleration
                .unwrap_or(f64::from(crate::binding::FALLBACK_ANGULAR_ACCELERATION))
                as f32;
            state.motion.target = Some(if command.absolute.is_some() {
                viz_scene::PhysicalMotionTarget::Position {
                    degrees: value.value as f32,
                    max_speed: value
                        .limits
                        .speed
                        .unwrap_or(f64::from(crate::binding::FALLBACK_ANGULAR_SPEED))
                        as f32,
                    acceleration,
                    deceleration,
                }
            } else {
                viz_scene::PhysicalMotionTarget::Velocity {
                    degrees_per_second: value.value as f32,
                    acceleration,
                    deceleration,
                }
            });
        }
    }
    pub fn apply(
        &self,
        binding: &PhysicalEmitterBinding,
        value: &mut EmitterValues,
        dimmer: Option<f32>,
        emitter: &viz_scene::EmitterInstance,
    ) {
        if !self.plan.position_declared {
            value.physical_pose = None;
        }
        if !self.plan.color_declared {
            value.physical_color = None;
        }
        if self.plan.position_declared && self.plan.position.is_none() {
            value.physical_pose = Some(viz_scene::PhysicalPoseState {
                flags: 4,
                ..Default::default()
            });
        }
        if self.plan.color_declared {
            // Missing/unknown data never reveals an old guessed wheel color or a requested color.
            let color = self
                .available
                .then(|| {
                    binding
                        .color
                        .filter(|&i| {
                            self.plan
                                .color
                                .as_ref()
                                .is_some_and(|m| m.inputs_available(i, &self.channel_available))
                        })
                        .and_then(|i| self.colors.get(i))
                })
                .flatten();
            let state = color.map_or(
                PhysicalColorState {
                    flags: 1,
                    ..Default::default()
                },
                |c| PhysicalColorState {
                    known_xyz: [c.known_xyz.x, c.known_xyz.y, c.known_xyz.z],
                    visible_complete: c.visible_complete,
                    quality: quality(c.data_quality),
                    flags: u32::from(c.flags.0),
                    uv_drive: c.uv_drive_max as f32,
                },
            );
            value.colour = display_linear_rgb(state.known_xyz);
            value.uv_drive = state.uv_drive;
            value.intensity = dimmer.unwrap_or(1.);
            value.physical_color = Some(state);
            // Current multi-cell legacy mixes have no calibrated per-cell optical model.
            // A head-level result belongs uniformly to that head, never mixed with guessed cells.
            value.cells.clear();
        }
        if let Some(optics) = self
            .available
            .then(|| binding.optics.and_then(|i| self.optics.get(i)))
            .flatten()
        {
            if let Some(focus) = optics.focus {
                value.focus = focus.percent as f32 / 100.;
            }
            let shape = optics.zoom.and_then(|v| {
                v.convention.and_then(|c| {
                    viz_scene::physical_zoom_outer_half_angle(
                        v.degrees as f32,
                        c == OpeningConvention::Field,
                        emitter.optics.sharpness,
                        emitter.optics.uniformity,
                        value.focus,
                    )
                })
            });
            value.physical_optics = Some(PhysicalOpticsState {
                zoom_shape_half_angle: shape,
                shape_nominal: true,
                zoom_full_degrees: optics.zoom.map(|v| v.degrees as f32),
                zoom_is_field: optics
                    .zoom
                    .and_then(|v| v.convention.map(|c| c == OpeningConvention::Field)),
                zoom_uncertain: !matches!(
                    optics.zoom_status,
                    OpticsForwardStatus::Resolved | OpticsForwardStatus::Unsupported
                ) || optics
                    .zoom
                    .is_some_and(|v| v.convention.is_none() || shape.is_none()),
                focus_uncertain: !matches!(
                    optics.focus_status,
                    OpticsForwardStatus::Resolved | OpticsForwardStatus::Unsupported
                ),
                focus_nominal: optics.focus.is_some_and(|v| v.nominal),
            });
            if let Some(focus) = optics.focus {
                value.focus = focus.percent as f32 / 100.;
            }
        } else {
            value.physical_optics = Some(PhysicalOpticsState {
                zoom_uncertain: true,
                focus_uncertain: true,
                ..Default::default()
            });
        }
    }
}
fn quality(q: PhysicalDataQuality) -> u8 {
    match q {
        PhysicalDataQuality::Unknown => 0,
        PhysicalDataQuality::Estimated => 1,
        PhysicalDataQuality::Manufacturer => 2,
        PhysicalDataQuality::Measured => 3,
    }
}
/// Display conversion only. Keep relative linear output; do not normalize every recipe to white.
/// Out-of-display-gamut negative components clip here, never in the physical XYZ prediction.
fn display_linear_rgb([x, y, z]: [f32; 3]) -> [f32; 3] {
    [
        3.2404542 * x - 1.5371385 * y - 0.4985314 * z,
        -0.969266 * x + 1.8760108 * y + 0.041556 * z,
        0.0556434 * x - 0.2040259 * y + 1.0572252 * z,
    ]
    .map(|v| v.max(0.))
}

pub(crate) fn attach(
    fixture: &PatchedFixture,
    mode: &FixtureMode,
    instance: &PhysicalInstance,
    primary: &HashMap<Uuid, u16>,
    addresses: &HashMap<u16, (u16, u16)>,
    scene: &mut viz_scene::Scene,
    emitter_start: usize,
    bindings: &mut [EmitterBinding],
) {
    if bindings.is_empty() {
        return;
    }
    let channels = mode
        .channels
        .iter()
        .map(|c| {
            let (universe, base) = *addresses.get(&c.split)?;
            let coarse = *primary.get(&c.id)?;
            let slots: Vec<u16> = std::iter::once(coarse)
                .chain(c.secondary_slots.iter().copied())
                .take(c.resolution.bytes())
                .map(|s| base.saturating_add(s).saturating_sub(1))
                .collect();
            if slots.len() != c.resolution.bytes() || slots.iter().any(|s| !(1..=512).contains(s)) {
                return None;
            }
            Some(ChannelRef {
                logical_universe: universe,
                slots,
                max_raw: c.resolution.max_raw(),
                invert: false,
                physical_min: 0.,
                physical_max: 1.,
                physical_unit: None,
                snap: c.snap,
                default_raw: c.default_raw,
                functions: vec![],
            })
        })
        .collect();
    let color = CompiledColorForward::compile(
        &fixture.profile,
        mode.id,
        instance.color_calibration.as_ref(),
    )
    .ok()
    .flatten()
    .map(|m| m.with_installed_appearance(&instance.installed_appearance));
    let optics = CompiledOpticsForward::compile(mode).ok();
    let colors = color.as_ref().map_or_else(Vec::new, |m| m.create_output());
    let optical = optics.as_ref().map_or_else(Vec::new, |m| m.create_output());
    let channels: Box<[Option<ChannelRef>]> = channels;
    let mut universes: Vec<u16> = channels
        .iter()
        .flatten()
        .map(|c| c.logical_universe)
        .collect();
    universes.sort_unstable();
    universes.dedup();
    // A derived (nominal) Position model drives output through the server; the Stage keeps its
    // proxy, which reads the same derived travel from the graph's motion nodes.
    let derived_position =
        light_fixture::is_derived_position_geometry(&fixture.profile.mode_geometry(mode));
    let position = CompiledPositionForward::compile(
        &fixture.profile,
        mode.id,
        PositionInstallation {
            calibration: instance.position_calibration.as_ref(),
            invert_pan: instance.invert_pan,
            invert_tilt: instance.invert_tilt,
            bracket_degrees: f64::from(instance.bracket_angle),
        },
    )
    .ok()
    .flatten()
    .filter(|_| !derived_position);
    let position_index = position.as_ref().map(|model| {
        let index = scene.physical_positions.len();
        let lenses = model.create_output();
        let graph = model.pose_graph();
        let geometry = fixture.profile.mode_geometry(mode);
        let emitter_map = scene.emitters[emitter_start..]
            .iter()
            .enumerate()
            .filter_map(|(offset, e)| {
                let head = mode.heads.get(e.head_index as usize)?.id;
                // Geometry build order is the authored emitter order for this head.
                let previous = scene.emitters[emitter_start..emitter_start + offset]
                    .iter()
                    .filter(|v| v.head_index == e.head_index)
                    .count();
                let geometry_emitter = geometry
                    .emitters
                    .iter()
                    .filter(|g| g.head_id == Some(head))
                    .nth(previous)?;
                lenses
                    .iter()
                    .position(|l| l.emitter_id == geometry_emitter.id)
                    .map(|i| (emitter_start + offset, i))
            })
            .collect();
        let fixture_index = scene.emitters[emitter_start].fixture_index as usize;
        scene
            .physical_position_indices
            .resize(scene.fixtures.len(), None);
        scene.physical_position_indices[fixture_index] = Some(index);
        let part_nodes = scene.fixtures[fixture_index]
            .model
            .and_then(|i| scene.models.get(i as usize))
            .map_or_else(Vec::new, |m| {
                m.parts
                    .iter()
                    .map(|p| {
                        let mut ancestor = p.geometry_node_id;
                        while let Some(id) = ancestor {
                            if let Some(i) =
                                (0..graph.node_count()).find(|&i| graph.node_id(i) == Some(id))
                            {
                                return Some(i);
                            }
                            ancestor = geometry
                                .nodes
                                .iter()
                                .find(|n| n.id == id)
                                .and_then(|n| n.parent_id);
                        }
                        None
                    })
                    .collect()
            });
        scene
            .physical_positions
            .push(viz_scene::PhysicalPositionPlan {
                instance_id: instance.instance_id,
                fixture_index,
                lens_axes: (0..lenses.len())
                    .map(|i| graph.lens_axis_indices(i))
                    .collect(),
                graph,
                axis_nodes: model.create_commands().iter().map(|a| a.node_id).collect(),
                emitters: emitter_map,
                model_part_nodes: part_nodes,
                model_scale: instance.model_scale,
            });
        index
    });
    let native_channels: Box<[ChannelRef]> = mode
        .channels
        .iter()
        .enumerate()
        .map(|(i, c)| ChannelRef {
            logical_universe: (i / 128) as u16,
            slots: (0..c.resolution.bytes())
                .map(|b| (i % 128 * 4 + b + 1) as u16)
                .collect(),
            max_raw: c.resolution.max_raw(),
            invert: c.invert,
            physical_min: c.physical_min.unwrap_or(0.),
            physical_max: c.physical_max.unwrap_or(1.),
            physical_unit: c.unit.clone(),
            snap: c.snap,
            default_raw: c.default_raw,
            functions: crate::plan::stage_channel_functions(&fixture.profile, c),
        })
        .collect();
    let native_by_id: HashMap<_, _> = mode
        .channels
        .iter()
        .zip(native_channels.iter())
        .filter(|(c, _)| c.behavior != light_fixture::ChannelBehavior::Static)
        .map(|(c, r)| (c.id, r.clone()))
        .collect();
    let native_heads = crate::plan::bindings::group_by_head(mode, &native_by_id);
    let plan = Arc::new(PhysicalPlan {
        position,
        position_index,
        position_declared: !derived_position
            && fixture
                .profile
                .mode_geometry(mode)
                .physical_contract
                .is_some(),
        universes: universes.into_boxed_slice(),
        instance_id: instance.instance_id,
        fixture_id: fixture.fixture_id,
        native_identity: light_fixture::forward::native_instance_identity(
            &fixture.profile,
            mode,
            instance.color_calibration.as_ref(),
            PositionInstallation {
                calibration: instance.position_calibration.as_ref(),
                invert_pan: instance.invert_pan,
                invert_tilt: instance.invert_tilt,
                bracket_degrees: f64::from(instance.bracket_angle),
            },
            &instance.installed_appearance,
            &instance
                .split_patches
                .iter()
                .filter_map(|(split, a)| a.map(|(u, a)| (*split, u, a)))
                .collect::<Vec<_>>(),
        ),
        native_channels,
        channels,
        color,
        color_declared: mode.color_physical.is_some(),
        optics,
    });
    for (emitter, binding) in scene.emitters[emitter_start..].iter().zip(bindings) {
        let head = mode.heads.get(emitter.head_index as usize).map(|h| h.id);
        let owned = head
            .and_then(|h| native_heads.get(&h))
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let mut native_binding = crate::plan::bindings::build_binding(
            owned,
            instance,
            mode,
            head.unwrap_or_default(),
            &native_by_id,
        );
        // These are final server bytes; installation inversion has already been encoded.
        native_binding.invert_pan = false;
        native_binding.invert_tilt = false;
        if emitter.cells.len() > 1 {
            native_binding.cells = crate::plan::bindings::cell_bindings(owned, emitter.cells.len());
        }
        binding.physical = Some(PhysicalEmitterBinding {
            native_binding: Box::new(native_binding),
            plan: plan.clone(),
            color: colors.iter().position(|c| Some(c.head_id) == head),
            optics: optical.iter().position(|c| Some(c.head_id) == head),
        });
        // Shared native controls can live on a different split/universe from this head.
        binding
            .universes
            .extend(plan.channels.iter().flatten().map(|c| c.logical_universe));
        binding.universes.sort_unstable();
        binding.universes.dedup();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;
    use light_fixture::{OpticalEmitterBand, OpticalSource};
    fn rig() -> crate::ScenePlan {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../assets/fixture-library/cameo--root-par-6.toskfixture");
        let mut profile =
            light_fixture::read_fixture_package(&std::fs::read(path).unwrap()).unwrap();
        let mode = profile
            .modes
            .iter_mut()
            .find(|m| m.channels.len() == 7)
            .unwrap();
        let OpticalSource::Additive { emitters } =
            &mut mode.color_physical.as_mut().unwrap().paths[0].source
        else {
            unreachable!()
        };
        for emitter in emitters {
            emitter.xyz = match emitter.name.to_lowercase().as_str() {
                "red" => Some(light_core::Xyz {
                    x: 0.4124564,
                    y: 0.2126729,
                    z: 0.0193339,
                }),
                "green" => Some(light_core::Xyz {
                    x: 0.3575761,
                    y: 0.7151522,
                    z: 0.119192,
                }),
                "blue" => Some(light_core::Xyz {
                    x: 0.1804375,
                    y: 0.072175,
                    z: 0.9503041,
                }),
                _ if emitter.band != OpticalEmitterBand::Ultraviolet => Some(light_core::Xyz {
                    x: 0.,
                    y: 0.,
                    z: 0.,
                }),
                _ => None,
            };
        }
        let mode_id = mode.id;
        crate::compile(&[crate::PatchedFixture {
            fixture_id: Uuid::new_v4(),
            name: "UV reference".into(),
            number: Some(1),
            profile: Arc::new(profile),
            mode_id,
            instances: vec![PhysicalInstance {
                instance_id: Uuid::new_v4(),
                name: "UV reference".into(),
                split_patches: vec![(1, Some((1, 1)))],
                position: Vec3::ZERO,
                rotation_degrees: Vec3::ZERO,
                invert_pan: false,
                invert_tilt: false,
                bracket_angle: 0.,
                shaper_angle: None,
                installed_appearance: Default::default(),
                scenery_size_metres: None,
                scenery_options: Default::default(),
                model_scale: 1.,
                color_calibration: None,
                position_calibration: None,
            }],
        }])
    }
    fn frame(rgbuv: [u8; 6]) -> viz_dmx::UniverseFrame {
        let mut slots = [0; DMX_SLOTS];
        slots[..6].copy_from_slice(&rgbuv);
        viz_dmx::UniverseFrame {
            logical_universe: 1,
            slots,
            received_micros: 0,
            stale: false,
        }
    }
    #[test]
    fn native_uv_activity_survives_unknown_visible_spill_without_fabricated_violet() {
        let plan = rig();
        let mut decoder = crate::Decoder::new(plan.bindings);
        let mut values = viz_scene::SceneValues::default();
        decoder.apply(&plan.scene, &[frame([0, 0, 0, 0, 0, 255])], &mut values, 0.);
        let v = &values.emitters[0];
        assert_eq!(v.colour, [0.; 3]);
        assert_eq!(v.uv_drive, 1.);
        assert!(!v.physical_color.unwrap().visible_complete);
        decoder.apply(
            &plan.scene,
            &[frame([128, 0, 0, 0, 0, 255])],
            &mut values,
            0.1,
        );
        let v = &mut values.emitters[0];
        assert!((v.colour[0] - 128. / 255.).abs() < 0.001);
        assert!(v.colour[1] < 0.0001 && v.colour[2] < 0.0001);
        assert!(!v.physical_color.unwrap().visible_complete);
        // A legacy color-wheel palette must not overwrite calibrated visible output on a tick.
        v.colour_wheel_palette = vec![[0., 0., 1.]];
        v.colour_wheel_motion.set_target(0, 1, 10., 20., 20.);
        let expected = v.colour;
        values.apply_physical_motion(0.1);
        assert_eq!(values.emitters[0].colour, expected);
        decoder.apply(
            &plan.scene,
            &[frame([128, 0, 0, 0, 0, 0])],
            &mut values,
            0.2,
        );
        assert!(values.emitters[0].physical_color.unwrap().visible_complete);
    }
    #[test]
    fn absent_native_input_is_not_authoritative_zero() {
        let plan = rig();
        let binding = plan.bindings[0].physical.as_ref().unwrap();
        let mut runtime = PhysicalRuntime::new(binding.plan.clone());
        runtime.update(&HashMap::new());
        let mut value = EmitterValues::default();
        runtime.apply(binding, &mut value, Some(1.), &plan.scene.emitters[0]);
        assert!(!value.physical_color.unwrap().visible_complete);
        assert_eq!(value.colour, [0.; 3]);
    }
    #[test]
    fn unpatched_native_values_drive_color_and_keep_uv_distinct() {
        let mut plan = rig();
        for binding in &mut plan.bindings {
            binding.universes.clear();
            let physical = binding.physical.as_mut().unwrap();
            let shared = Arc::make_mut(&mut physical.plan);
            shared.channels.fill(None);
            shared.universes = Box::default();
        }
        let p = plan.bindings[0].physical.as_ref().unwrap().plan.clone();
        let mut decoder = crate::Decoder::new(plan.bindings.clone());
        assert!(decoder.required_universes().is_empty());
        let mut values = viz_scene::SceneValues::default();
        let raw = [128, 0, 0, 0, 0, 255, 0];
        let record = || crate::NativeInstanceValues {
            fixture_id: p.fixture_id,
            instance_id: p.instance_id,
            native_identity: &p.native_identity,
            raw: &raw,
            owned_channels: None,
        };
        assert_eq!(
            decoder.apply_native(
                &plan.scene,
                [crate::NativeInstanceValues {
                    native_identity: "stale",
                    ..record()
                }],
                false,
                &mut values,
                0.
            ),
            0
        );
        assert_eq!(
            decoder.apply_native(&plan.scene, [record()], false, &mut values, 0.1),
            1
        );
        assert!((values.emitters[0].colour[0] - 128. / 255.).abs() < 0.001);
        assert_eq!(values.emitters[0].intensity, 1.);
        assert_eq!(values.emitters[0].uv_drive, 1.);
        assert!(!values.emitters[0].physical_color.unwrap().visible_complete);
    }

    #[test]
    fn patched_live_slots_win_native_while_preload_and_clear_redecode_without_new_dmx() {
        let plan = rig();
        let p = plan.bindings[0].physical.as_ref().unwrap().plan.clone();
        let mut decoder = crate::Decoder::new(plan.bindings.clone());
        let mut values = viz_scene::SceneValues::default();
        decoder.apply(&plan.scene, &[frame([255, 0, 0, 0, 0, 0])], &mut values, 0.);
        let raw = [0, 255, 0, 0, 0, 0, 0];
        let record = || crate::NativeInstanceValues {
            fixture_id: p.fixture_id,
            instance_id: p.instance_id,
            native_identity: &p.native_identity,
            raw: &raw,
            owned_channels: None,
        };
        decoder.apply_native(&plan.scene, [record()], false, &mut values, 0.1);
        assert!(values.emitters[0].colour[0] > 0.99 && values.emitters[0].colour[1] < 0.001);
        decoder.apply_native(&plan.scene, [record()], true, &mut values, 0.2);
        assert!(values.emitters[0].colour[1] > 0.99 && values.emitters[0].colour[0] < 0.001);
        decoder.apply_native(&plan.scene, [record()], false, &mut values, 0.3);
        assert!(values.emitters[0].colour[0] > 0.99 && values.emitters[0].colour[1] < 0.001);
        let bad = [0, 256, 0, 0, 0, 0, 0];
        assert_eq!(
            decoder.apply_native(
                &plan.scene,
                [crate::NativeInstanceValues {
                    raw: &bad,
                    ..record()
                }],
                true,
                &mut values,
                0.4
            ),
            0
        );
    }

    #[test]
    fn partial_native_preload_preserves_same_fixture_live_overrides_and_uv() {
        let plan = rig();
        let p = plan.bindings[0].physical.as_ref().unwrap().plan.clone();
        let mut decoder = crate::Decoder::new(plan.bindings.clone());
        let mut values = viz_scene::SceneValues::default();
        decoder.apply(
            &plan.scene,
            &[frame([201, 173, 0, 0, 0, 0])],
            &mut values,
            0.,
        );
        let raw = [64, 0, 0, 0, 0, 197, 0];
        let mask = [true, false, false, false, false, true, false];
        let record = || crate::NativeInstanceValues {
            fixture_id: p.fixture_id,
            instance_id: p.instance_id,
            native_identity: &p.native_identity,
            raw: &raw,
            owned_channels: Some(&mask),
        };
        decoder.apply_native(&plan.scene, [record()], false, &mut values, 0.1);
        decoder.apply_native(&plan.scene, [record()], true, &mut values, 0.1);
        assert!((values.emitters[0].colour[0] - 64. / 255.).abs() < 0.001);
        assert!((values.emitters[0].colour[1] - 173. / 255.).abs() < 0.001);
        assert!((values.emitters[0].uv_drive - 197. / 255.).abs() < 0.001);
        let before = values.emitters[0].colour;
        assert_eq!(
            decoder.apply_native(
                &plan.scene,
                [crate::NativeInstanceValues {
                    owned_channels: Some(&[true]),
                    ..record()
                }],
                true,
                &mut values,
                0.2
            ),
            0
        );
        assert_eq!(values.emitters[0].colour, before);
        decoder.apply_native(&plan.scene, [record()], false, &mut values, 0.3);
        assert!((values.emitters[0].colour[0] - 201. / 255.).abs() < 0.001);
        assert!((values.emitters[0].colour[1] - 173. / 255.).abs() < 0.001);
        assert_eq!(values.emitters[0].uv_drive, 0.);
    }

    #[test]
    fn native_fills_missing_split_but_never_replaces_a_patched_universe_that_has_not_arrived() {
        let mut plan = rig();
        let b = plan.bindings[0].physical.as_mut().unwrap();
        let p = Arc::make_mut(&mut b.plan);
        for channel in &mut p.channels[1..] {
            *channel = None;
        }
        let p = b.plan.clone();
        let mut decoder = crate::Decoder::new(plan.bindings.clone());
        let mut values = viz_scene::SceneValues::default();
        let raw = [255, 0, 255, 0, 0, 0, 0];
        let record = || crate::NativeInstanceValues {
            fixture_id: p.fixture_id,
            instance_id: p.instance_id,
            native_identity: &p.native_identity,
            raw: &raw,
            owned_channels: None,
        };
        decoder.apply_native(&plan.scene, [record()], false, &mut values, 0.);
        assert!(!values.emitters[0].physical_color.unwrap().visible_complete);
        decoder.apply(&plan.scene, &[frame([64, 0, 0, 0, 0, 0])], &mut values, 0.1);
        decoder.apply_native(&plan.scene, [record()], false, &mut values, 0.1);
        assert!(values.emitters[0].physical_color.unwrap().visible_complete);
        assert!((values.emitters[0].colour[0] - 64. / 255.).abs() < 0.001);
        assert!(values.emitters[0].colour[2] > 0.99);
    }

    #[test]
    fn missing_unrelated_dimmer_input_does_not_hide_known_color() {
        let mut plan = rig();
        let binding = plan.bindings[0].physical.as_mut().unwrap();
        let physical = Arc::make_mut(&mut binding.plan);
        // RGBWAU occupy the first six controls; dimmer is not an optical path input.
        physical.channels[6] = None;
        let mut decoder = crate::Decoder::new(plan.bindings.clone());
        let mut values = viz_scene::SceneValues::default();
        decoder.apply(
            &plan.scene,
            &[frame([255, 0, 255, 0, 0, 0])],
            &mut values,
            0.,
        );
        let achieved = values.emitters[0].physical_color.unwrap();
        assert!(achieved.visible_complete);
        assert!(values.emitters[0].colour[0] > 0.99 && values.emitters[0].colour[2] > 0.99);
    }

    #[test]
    fn replacing_with_legacy_mode_releases_old_prediction_ownership() {
        let plan = rig();
        let old = plan.bindings[0].physical.as_ref().unwrap();
        let mut replacement = (*old.plan).clone();
        replacement.color_declared = false;
        replacement.color = None;
        replacement.position_declared = false;
        replacement.position = None;
        let runtime = PhysicalRuntime::new(Arc::new(replacement));
        let mut value = EmitterValues::default();
        value.physical_color = Some(PhysicalColorState::default());
        value.physical_pose = Some(viz_scene::PhysicalPoseState {
            local: Some(glam::Mat4::IDENTITY),
            ..Default::default()
        });
        runtime.apply(old, &mut value, Some(1.), &plan.scene.emitters[0]);
        assert!(value.physical_color.is_none());
        assert!(value.physical_pose.is_none());
    }
}

#[cfg(test)]
mod position_tests {
    use super::*;
    use glam::Vec3;
    use light_fixture::*;
    fn fixture() -> crate::PatchedFixture {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../assets/fixture-library/cameo--root-par-6.toskfixture");
        let original = read_fixture_package(&std::fs::read(path).unwrap()).unwrap();
        let mut profile = FixtureProfile::blank();
        profile.manufacturer = "Reference".into();
        profile.name = "Physical motion".into();
        profile.short_name = "Motion".into();
        profile.fixture_type = "moving-head".into();
        let mode = &mut profile.modes[0];
        let head = mode.heads[0].id;
        profile.geometry = GeometryGraph::template(GeometryTemplate::MovingHead, &[head]);
        profile.geometry.physical_contract = Some(GeometryPhysicalContract {
            version: 1,
            provenance: Default::default(),
            bracket: GeometryBracket::Hinge {
                node_id: profile.geometry.nodes[0].id,
                pivot: Default::default(),
                axis: Vector3 {
                    x: 1.,
                    y: 0.,
                    z: 0.,
                },
            },
        });
        let mut bindings = Vec::new();
        for (i, (name, role, min, max)) in [
            ("pan", PositionAxisRole::Pan, -720., 720.),
            ("tilt", PositionAxisRole::Tilt, 0., 255.),
        ]
        .into_iter()
        .enumerate()
        {
            let mut c = original.modes[0].channels[i].clone();
            c.head_id = head;
            c.attribute = light_core::AttributeKey(name.into());
            c.fixture_attribute = c.attribute.clone();
            c.functions[0].attribute = c.attribute.clone();
            c.functions[0].behavior = ChannelFunctionBehavior::Continuous {
                physical_min: min,
                physical_max: max,
                unit: Some("deg".into()),
            };
            c.functions[0].angular_motion = Some(AngularMotion {
                kind: AngularMotionKind::AbsolutePosition,
                max_speed_degrees_per_second: Some(180.),
                acceleration_degrees_per_second_squared: Some(720.),
                deceleration_degrees_per_second_squared: Some(720.),
            });
            bindings.push(MotionFunctionBinding {
                node_id: profile.geometry.nodes[i + 1].id,
                channel_id: c.id,
                function_id: c.functions[0].id,
                role,
            });
            mode.channels.push(c);
        }
        mode.splits[0].footprint = 2;
        mode.position_physical = Some(PositionPhysicalModel {
            version: 1,
            revision: 0,
            bindings,
        });
        let lens = GeometryEmitter {
            id: Uuid::new_v4(),
            name: "Lens".into(),
            node_id: profile.geometry.nodes[2].id,
            head_id: None,
            origin: Vector3 {
                x: 0.,
                y: -600.,
                z: 100.,
            },
            orientation_degrees: Default::default(),
            beam_angle_degrees: 10.,
            field_angle_degrees: 20.,
            feather: 0.,
            focus: 1.,
            directional: true,
            layout: EmitterLayout::Point,
        };
        mode.emitter_heads = vec![EmitterHeadBinding {
            emitter_id: lens.id,
            head_id: head,
        }];
        profile.geometry.emitters = vec![lens];
        let mode_id = mode.id;
        profile.validate().unwrap();
        crate::PatchedFixture {
            fixture_id: Uuid::new_v4(),
            name: "Motion".into(),
            number: Some(1),
            profile: Arc::new(profile),
            mode_id,
            instances: vec![PhysicalInstance {
                instance_id: Uuid::new_v4(),
                name: "Motion".into(),
                split_patches: vec![(1, Some((1, 1)))],
                position: Vec3::ZERO,
                rotation_degrees: Vec3::ZERO,
                invert_pan: false,
                invert_tilt: false,
                bracket_angle: 45.,
                shaper_angle: None,
                installed_appearance: Default::default(),
                scenery_size_metres: None,
                scenery_options: Default::default(),
                model_scale: 1.,
                color_calibration: None,
                position_calibration: None,
            }],
        }
    }
    fn rig() -> crate::ScenePlan {
        crate::compile(&[fixture()])
    }
    /// Core timing only: no GPU or output scheduler claim. Run explicitly and retain stdout.
    #[test]
    #[ignore = "manual bounded physical decode and mechanical evaluation timing"]
    fn bounded_physical_motion_timing() {
        for count in [300usize, 1000] {
            let mut fixture = fixture();
            let original = fixture.instances[0].clone();
            fixture.instances = (0..count)
                .map(|i| {
                    let mut instance = original.clone();
                    instance.instance_id = Uuid::from_u128(i as u128 + 1);
                    instance.position = Vec3::new((i % 20) as f32, 5., (i / 20) as f32);
                    instance.split_patches =
                        vec![(1, Some(((i / 256 + 1) as u16, (i % 256 * 2 + 1) as u16)))];
                    instance
                })
                .collect();
            let plan = crate::compile(&[fixture]);
            let mut decoder = crate::Decoder::new(plan.bindings);
            let mut values = viz_scene::SceneValues::default();
            decoder.initialize_motion(&plan.scene, &mut values);
            let mut frames: Vec<_> = (1..=count.div_ceil(256))
                .map(|u| viz_dmx::UniverseFrame {
                    logical_universe: u as u16,
                    slots: [0; 512],
                    received_micros: 0,
                    stale: false,
                })
                .collect();
            let mut samples = Vec::with_capacity(600);
            for tick in 0..660 {
                let start = std::time::Instant::now();
                if tick % 3 == 0 {
                    for frame in &mut frames {
                        for pair in frame.slots.chunks_mut(2) {
                            pair[0] = (tick % 256) as u8;
                            pair[1] = (255 - tick % 256) as u8;
                        }
                    }
                    decoder.apply(&plan.scene, &frames, &mut values, tick as f32 / 60.);
                }
                values.apply_calibrated_motion(&plan.scene, 1. / 60.);
                std::hint::black_box(&values);
                if tick >= 60 {
                    samples.push(start.elapsed().as_secs_f64() * 1000.);
                }
            }
            samples.sort_by(f64::total_cmp);
            println!(
                "{}",
                serde_json::json!({"physical_instances": count, "samples":samples.len(), "mean_ms":samples.iter().sum::<f64>()/samples.len() as f64, "p95_ms":samples[samples.len()*95/100], "max_ms":samples[samples.len()-1], "includes_gpu":false})
            );
            assert_eq!(values.physical_positions.len(), count);
            assert!(
                values
                    .emitters
                    .iter()
                    .all(|v| v.physical_pose.unwrap().local.is_some())
            );
        }
    }

    #[test]
    fn physical_home_uses_defaults_and_keeps_buffers_and_held_pose() {
        let plan = rig();
        let decoder = crate::Decoder::new(plan.bindings);
        let mut values = viz_scene::SceneValues::default();
        decoder.initialize_motion(&plan.scene, &mut values);
        assert!(
            values.physical_positions[0]
                .axes
                .iter()
                .all(|a| a.has_position)
        );
        let targets: Vec<_> = values.physical_positions[0]
            .axes
            .iter()
            .map(|a| a.motion.target)
            .collect();
        let ptr = values.physical_positions[0].node_deltas.as_ptr();
        for _ in 0..1000 {
            values.apply_calibrated_motion(&plan.scene, 0.01);
        }
        assert_eq!(values.physical_positions[0].node_deltas.as_ptr(), ptr);
        assert!(values.emitters[0].physical_pose.unwrap().local.is_some());
        for (axis, target) in values.physical_positions[0].axes.iter().zip(&targets) {
            let Some(viz_scene::PhysicalMotionTarget::Position { degrees, .. }) = target else {
                panic!("authored absolute home");
            };
            assert_eq!(axis.motion.position_degrees, *degrees);
        }
        let held = values.physical_positions[0].axes[0].motion.position_degrees;
        let mut incoming = values.clone();
        incoming.retain_calibrated_motion_from(&values);
        incoming.take_calibrated_runtime_from(&mut values);
        incoming.apply_calibrated_motion(&plan.scene, 0.);
        assert_eq!(incoming.physical_positions[0].node_deltas.as_ptr(), ptr);
        assert_eq!(
            incoming.physical_positions[0].axes[0]
                .motion
                .position_degrees,
            held
        );
    }

    #[test]
    fn repeated_lenses_of_one_head_keep_values_when_scene_order_changes() {
        let plan = rig();
        let mut before = plan.scene;
        before.emitters.push(before.emitters[0].clone());
        before.emitter_ids.push(Uuid::new_v4());
        let mut values = viz_scene::SceneValues::default();
        values.resize(2);
        values.emitters[0].intensity = 0.2;
        values.emitters[1].intensity = 0.8;
        let mut after = before.clone();
        after.emitters.swap(0, 1);
        after.emitter_ids.swap(0, 1);
        values.carry_over(&before, &after);
        assert_eq!(values.emitters[0].intensity, 0.8);
        assert_eq!(values.emitters[1].intensity, 0.2);
    }

    #[test]
    fn native_commands_keep_unwrapped_motion_and_bracketed_lens_on_display_clock() {
        let plan = rig();
        assert_eq!(plan.scene.physical_positions.len(), 1);
        let mut decoder = crate::Decoder::new(plan.bindings);
        let mut values = viz_scene::SceneValues::default();
        let mut slots = [0; 512];
        slots[0] = 255;
        slots[1] = 90;
        decoder.apply(
            &plan.scene,
            &[viz_dmx::UniverseFrame {
                logical_universe: 1,
                slots,
                received_micros: 0,
                stale: false,
            }],
            &mut values,
            0.,
        );
        let commanded = values.physical_positions[0].axes[0].motion.target.unwrap();
        assert!(matches!(
            commanded,
            viz_scene::PhysicalMotionTarget::Position { degrees: 720., .. }
        ));
        values.apply_calibrated_motion(&plan.scene, 0.1);
        assert!(values.physical_positions[0].axes[0].motion.position_degrees < 20.);
        for _ in 0..100 {
            values.apply_calibrated_motion(&plan.scene, 0.1);
        }
        assert_eq!(
            values.physical_positions[0].axes[0].motion.position_degrees,
            720.
        );
        let pose = values.emitters[0].physical_pose.unwrap();
        assert_eq!(pose.flags, 0);
        assert!(!pose.nominal_motion);
        let local = pose.local.unwrap();
        let reference = glam::Quat::from_rotation_x(135_f32.to_radians());
        assert!((local.transform_vector3(Vec3::NEG_Y) - reference * Vec3::NEG_Y).length() < 1e-5);
        assert!(
            (local.transform_point3(Vec3::ZERO) - reference * Vec3::new(0., -0.6, 0.1)).length()
                < 1e-5
        );
        // A provider snapshot carries targets; it cannot reset renderer-owned settling.
        let mut next = viz_scene::SceneValues::default();
        decoder.apply(
            &plan.scene,
            &[viz_dmx::UniverseFrame {
                logical_universe: 1,
                slots,
                received_micros: 1,
                stale: false,
            }],
            &mut next,
            0.1,
        );
        next.retain_visual_motion_runtime_from(&values);
        next.take_calibrated_runtime_from(&mut values);
        next.apply_calibrated_motion(&plan.scene, 0.1);
        assert_eq!(
            next.physical_positions[0].axes[0].motion.position_degrees,
            720.
        );
    }
}
