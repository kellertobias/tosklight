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
    let channels = server_channels(mode, primary, addresses);
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
    let position_index = position
        .as_ref()
        .map(|model| register_position(fixture, mode, instance, model, scene, emitter_start));
    let native_channels = native_channel_refs(fixture, mode);
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

/// The server-output channel for each mode channel, or `None` when it is unpatched or out of
/// range.
fn server_channels(
    mode: &FixtureMode,
    primary: &HashMap<Uuid, u16>,
    addresses: &HashMap<u16, (u16, u16)>,
) -> Box<[Option<ChannelRef>]> {
    mode.channels
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
        .collect()
}

/// Registers the fixture's physical Position plan in the scene and returns its index.
fn register_position(
    fixture: &PatchedFixture,
    mode: &FixtureMode,
    instance: &PhysicalInstance,
    model: &CompiledPositionForward,
    scene: &mut viz_scene::Scene,
    emitter_start: usize,
) -> usize {
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
}

/// Native-protocol channel references addressing the mode's channels in packed order.
fn native_channel_refs(fixture: &PatchedFixture, mode: &FixtureMode) -> Box<[ChannelRef]> {
    mode.channels
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
        .collect()
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod position_tests;
