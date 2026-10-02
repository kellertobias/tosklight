//! Configuration-time candidate tables for one head's optical path.
//!
//! Every discrete Color control contributes its known steady states (filter samples and
//! alternative emitter functions). The Cartesian product of those states is enumerated once and
//! each combination stores the per-unit emitter XYZ and fixed-source XYZ produced by the same
//! compiled [`Appearance`] and spectral integration the forward model evaluates.
use super::super::super::spectrum::{self, SAMPLES, Spectrum};
use super::super::{CompiledColorForward, Path, Source};
use super::solve::MAX_VISIBLE_VARIABLES;
use crate::{
    ChannelFunctionBehavior, FixtureMode, OpticalEmitterBand, OpticalSource, ProfileError,
};
use light_core::NativeColorIdentity;
use uuid::Uuid;

/// Largest number of enumerated discrete-state combinations per head.
pub const COLOR_FIT_MAX_COMBINATIONS: usize = 4096;
/// Largest number of combinations when continuous visible emitters are solved per combination.
pub const COLOR_FIT_MAX_CONTINUOUS_COMBINATIONS: usize = 256;
/// Largest `combinations × (emitters + 1)` table size per head.
pub const COLOR_FIT_MAX_TABLE_ENTRIES: usize = 65_536;
/// Largest number of emitters (visible, UV and other) per head path.
pub const COLOR_FIT_MAX_PATH_EMITTERS: usize = 32;
/// Largest number of simultaneously solved visible emitters per combination.
pub const COLOR_FIT_MAX_VISIBLE_EMITTERS: usize = MAX_VISIBLE_VARIABLES;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum EmitterRole {
    Colored,
    White,
    Ultraviolet,
    NonVisible,
}

#[derive(Clone, Debug)]
pub(super) struct EmitterFit {
    pub id: Uuid,
    pub control: usize,
    pub function_id: Uuid,
    pub from: u32,
    pub to: u32,
    pub reversed: bool,
    pub maximum: f64,
    pub exponent: f64,
    pub gain: f64,
    pub role: EmitterRole,
}

impl EmitterFit {
    pub fn visible(&self) -> bool {
        matches!(self.role, EmitterRole::Colored | EmitterRole::White)
    }
    /// Normalized amount at the declared maximum drive, in the forward model's units.
    pub fn upper_amount(&self) -> f64 {
        self.maximum.powf(self.exponent) * self.gain
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StateKind {
    Emitter(usize),
    Filter { filter: usize, sample: usize },
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ControlState {
    pub kind: StateKind,
    pub from: u32,
    pub to: u32,
    pub function_id: Uuid,
}

#[derive(Clone, Debug)]
pub(super) struct ControlFit {
    pub channel: usize,
    pub channel_id: Uuid,
    pub split: u16,
    pub shared: bool,
    /// Known steady states; empty for an unmodeled or appearance-unknown control.
    pub states: Box<[ControlState]>,
    /// Every native function of the channel, for constraint validation.
    pub functions: Box<[(Uuid, u32, u32)]>,
    /// UV emitter driven on this channel (first function of its bank); frozen before fitting.
    pub uv: Option<usize>,
    /// Position in the discrete dimensions, when this control takes part in enumeration.
    pub dimension: Option<usize>,
    /// The channel has an authored optical binding (a known state set may still be empty).
    pub modeled: bool,
    /// Neutral raw when the control is retained: its first emitter closed, else the default.
    pub park_raw: u32,
}

impl ControlFit {
    pub fn state_of(&self, raw: u32) -> Option<usize> {
        self.states
            .iter()
            .position(|s| (s.from..=s.to).contains(&raw))
    }
    pub fn function_of(&self, raw: u32) -> Option<Uuid> {
        self.functions
            .iter()
            .find(|(_, from, to)| (*from..=*to).contains(&raw))
            .map(|(id, _, _)| *id)
    }
}

#[derive(Clone, Debug)]
pub(super) struct MeasuredRecipe {
    /// One raw value per control, in control order.
    pub raws: Box<[u32]>,
    pub xyz: [f64; 3],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TableStatus {
    Ready,
    CandidateLimit,
}

#[derive(Clone, Debug)]
pub(super) struct HeadTables {
    pub head_id: Uuid,
    pub path_id: Uuid,
    /// Authored filter identities in forward-model order.
    pub filter_ids: Box<[Uuid]>,
    pub identity: Option<NativeColorIdentity>,
    /// Single-path forward model with the original native indices.
    pub forward: CompiledColorForward,
    pub controls: Box<[ControlFit]>,
    pub emitters: Box<[EmitterFit]>,
    /// Control positions of the discrete dimensions and their state counts.
    pub dimensions: Box<[usize]>,
    pub combinations: usize,
    /// `combinations × emitters`: XYZ per unit amount through the combination, None if unknown.
    pub basis: Box<[Option<[f64; 3]>]>,
    /// Fixed-source XYZ per combination; zero for additive sources, None if unknown.
    pub fixed: Box<[Option<[f64; 3]>]>,
    /// Luminous absorption of the active filters (0 = clear, 1 = opaque or unknown).
    pub density: Box<[f64]>,
    pub measured: Box<[MeasuredRecipe]>,
    /// Some non-UV control has no known steady state (unmodeled or unknown transmission), so
    /// the forward model cannot know any combination; only measured recipes stay known.
    pub opaque: bool,
    pub status: TableStatus,
}

impl HeadTables {
    /// Digit of `dimension` inside `combination` (mixed radix, first dimension least significant).
    pub fn digit(&self, combination: usize, dimension: usize) -> usize {
        let mut value = combination;
        for &control in &self.dimensions[..dimension] {
            value /= self.controls[control].states.len();
        }
        value % self.controls[self.dimensions[dimension]].states.len()
    }

    /// Whether emitter `index` is driven in `combination` (its channel selects its function).
    pub fn emitter_active(&self, combination: usize, index: usize) -> bool {
        let emitter = &self.emitters[index];
        let control = &self.controls[emitter.control];
        if control.uv.is_some() {
            return control.uv == Some(index);
        }
        control.dimension.is_some_and(|d| {
            control.states[self.digit(combination, d)].kind == StateKind::Emitter(index)
        })
    }

    pub fn has_visible_emitters(&self) -> bool {
        self.emitters.iter().any(EmitterFit::visible)
    }
}

fn is_white(attribute: &str) -> bool {
    attribute.starts_with("color.") && attribute.ends_with("white")
}

fn function_span(mode: &FixtureMode, channel: usize, from: u32, to: u32) -> Uuid {
    mode.channels[channel]
        .functions
        .iter()
        .find(|f| f.dmx_from == from && f.dmx_to == to)
        .map_or_else(Uuid::nil, |f| f.id)
}

/// Rotating or velocity functions are never steady filter states.
fn steady(mode: &FixtureMode, channel: usize, from: u32, to: u32) -> bool {
    mode.channels[channel]
        .functions
        .iter()
        .find(|f| f.dmx_from == from && f.dmx_to == to)
        .is_some_and(|f| {
            !matches!(f.behavior, ChannelFunctionBehavior::Control { .. })
                && f.angular_motion
                    .is_none_or(|m| m.kind != crate::AngularMotionKind::AngularVelocity)
        })
}

fn emitters(mode: &FixtureMode, path: &Path, head: Uuid) -> Vec<EmitterFit> {
    let Source::Additive(emitters) = &path.source else {
        return Vec::new();
    };
    let authored = mode
        .color_physical
        .as_ref()
        .and_then(|m| m.paths.iter().find(|p| p.head_id == head))
        .and_then(|p| match &p.source {
            OpticalSource::Additive { emitters } => Some(emitters),
            _ => None,
        });
    emitters
        .iter()
        .map(|e| {
            let band = authored
                .and_then(|list| list.iter().find(|a| a.id == e.id))
                .map_or(OpticalEmitterBand::Visible, |a| a.band);
            let channel = &mode.channels[e.binding.channel];
            let function_id = function_span(mode, e.binding.channel, e.binding.from, e.binding.to);
            let function_attribute = channel
                .functions
                .iter()
                .find(|f| f.id == function_id)
                .map_or("", |f| f.attribute.0.as_ref());
            let white = [
                function_attribute,
                channel.attribute.0.as_ref(),
                channel.fixture_attribute.0.as_ref(),
            ]
            .into_iter()
            .any(is_white);
            let role = match (e.uv_index.is_some(), band, white) {
                (true, _, _) => EmitterRole::Ultraviolet,
                (false, OpticalEmitterBand::Visible, true) => EmitterRole::White,
                (false, OpticalEmitterBand::Visible, false) => EmitterRole::Colored,
                _ => EmitterRole::NonVisible,
            };
            EmitterFit {
                id: e.id,
                control: path
                    .controls
                    .iter()
                    .position(|c| *c == e.binding.channel)
                    .unwrap_or(0),
                function_id,
                from: e.binding.from,
                to: e.binding.to,
                reversed: e.reversed,
                maximum: e.maximum,
                exponent: e.exponent,
                gain: e.gain,
                role,
            }
        })
        .collect()
}

fn controls(
    mode: &FixtureMode,
    path: &Path,
    head: Uuid,
    emitters: &[EmitterFit],
) -> Vec<ControlFit> {
    path.controls
        .iter()
        .enumerate()
        .map(|(position, &channel)| {
            let native = &mode.channels[channel];
            let mut states = Vec::new();
            let mut uv = None;
            for (index, e) in emitters
                .iter()
                .enumerate()
                .filter(|(_, e)| e.control == position)
            {
                if e.role == EmitterRole::Ultraviolet {
                    uv = uv.or(Some(index));
                } else {
                    states.push(ControlState {
                        kind: StateKind::Emitter(index),
                        from: e.from,
                        to: e.to,
                        function_id: e.function_id,
                    });
                }
            }
            for (filter, f) in path.filters.iter().enumerate() {
                if f.binding.channel != channel
                    || !steady(mode, channel, f.binding.from, f.binding.to)
                {
                    continue;
                }
                let function_id = function_span(mode, channel, f.binding.from, f.binding.to);
                for (sample, s) in f.samples.iter().flat_map(|s| s.iter()).enumerate() {
                    if s.transmission.is_some() {
                        states.push(ControlState {
                            kind: StateKind::Filter { filter, sample },
                            from: s.from,
                            to: s.to,
                            function_id,
                        });
                    }
                }
            }
            let modeled = !path.modeled[position].is_empty();
            if uv.is_some() {
                states.clear();
            }
            let park_raw =
                emitters
                    .iter()
                    .find(|e| e.control == position)
                    .map_or(
                        native.default_raw,
                        |e| if e.reversed { e.to } else { e.from },
                    );
            ControlFit {
                channel,
                channel_id: native.id,
                split: native.split,
                shared: native.head_id != head,
                states: states.into_boxed_slice(),
                functions: native
                    .functions
                    .iter()
                    .map(|f| (f.id, f.dmx_from, f.dmx_to))
                    .collect(),
                uv,
                dimension: None,
                modeled,
                park_raw,
            }
        })
        .collect()
}

fn measured(path: &Path) -> Vec<MeasuredRecipe> {
    let mut recipes: Vec<MeasuredRecipe> = path
        .measurements
        .values()
        .flatten()
        .filter_map(|m| {
            let raws = path
                .controls
                .iter()
                .map(|c| m.recipe.iter().find(|(i, _)| i == c).map(|(_, raw)| *raw))
                .collect::<Option<Box<[u32]>>>()?;
            Some(MeasuredRecipe {
                raws,
                xyz: [m.xyz.x, m.xyz.y, m.xyz.z].map(f64::from),
            })
        })
        .collect();
    // The forward model stores observations by hash; fix a deterministic order here.
    recipes.sort_by(|a, b| a.raws.cmp(&b.raws));
    recipes
}

fn combination_count(controls: &[ControlFit], dimensions: &[usize]) -> Option<usize> {
    dimensions.iter().try_fold(1usize, |product, &c| {
        product.checked_mul(controls[c].states.len())
    })
}

impl HeadTables {
    pub fn compile(
        mode: &FixtureMode,
        forward: CompiledColorForward,
        identity: Option<NativeColorIdentity>,
        path_id: Uuid,
        filter_ids: Box<[Uuid]>,
    ) -> Result<Self, ProfileError> {
        let path = &forward.paths[0];
        let head_id = path.head;
        let emitters = emitters(mode, path, head_id);
        let mut controls = controls(mode, path, head_id, &emitters);
        let mut dimensions = Vec::new();
        for (position, control) in controls.iter_mut().enumerate() {
            if control.uv.is_none() && !control.states.is_empty() {
                control.dimension = Some(dimensions.len());
                dimensions.push(position);
            }
        }
        let visible = emitters.iter().filter(|e| e.visible()).count();
        let combinations = combination_count(&controls, &dimensions);
        let over_limit = combinations.is_none_or(|count| {
            count > COLOR_FIT_MAX_COMBINATIONS
                || count.saturating_mul(emitters.len() + 1) > COLOR_FIT_MAX_TABLE_ENTRIES
                || (visible > 0 && count > COLOR_FIT_MAX_CONTINUOUS_COMBINATIONS)
        }) || emitters.len() > COLOR_FIT_MAX_PATH_EMITTERS
            || visible > COLOR_FIT_MAX_VISIBLE_EMITTERS;
        // An unknown source stays Ready: its combinations rank as unknown while measured
        // whole-path recipes can still be known exactly.
        let status = if over_limit {
            TableStatus::CandidateLimit
        } else {
            TableStatus::Ready
        };
        let measured = measured(path).into_boxed_slice();
        let opaque = controls
            .iter()
            .any(|c| c.uv.is_none() && c.states.is_empty());
        let mut tables = Self {
            head_id,
            path_id,
            filter_ids,
            identity,
            controls: controls.into_boxed_slice(),
            emitters: emitters.into_boxed_slice(),
            dimensions: dimensions.into_boxed_slice(),
            combinations: 0,
            basis: Box::new([]),
            fixed: Box::new([]),
            density: Box::new([]),
            measured,
            opaque,
            status,
            forward,
        };
        if status == TableStatus::Ready {
            tables.combinations = combinations.unwrap_or(0);
            tables.fill();
        }
        Ok(tables)
    }

    /// Evaluates every combination through the compiled appearances. Configuration time only.
    fn fill(&mut self) {
        let path = &self.forward.paths[0];
        let count = self.combinations;
        let emitters = self.emitters.len();
        let mut basis = vec![None; count * emitters];
        let mut fixed = vec![None; count];
        let mut density = vec![1.0; count];
        let open = spectrum::integrate(&[1.0; SAMPLES], None)[1];
        let mut transmission: Box<Spectrum> = Box::new([1.0; SAMPLES]);
        for combination in 0..count {
            transmission.fill(1.0);
            let mut active = false;
            let mut known = true;
            for d in 0..self.dimensions.len() {
                let control = &self.controls[self.dimensions[d]];
                if let StateKind::Filter { filter, sample } =
                    control.states[self.digit(combination, d)].kind
                {
                    let sample = path.filters[filter].samples.as_ref().map(|s| &s[sample]);
                    // A unit transmission (a control parked open) filters nothing.
                    if sample.is_some_and(|s| s.identity()) {
                        continue;
                    }
                    active = true;
                    match sample.and_then(|s| s.transmission.as_deref()) {
                        Some(values) => {
                            for (t, v) in transmission.iter_mut().zip(values.iter()) {
                                *t *= v;
                            }
                        }
                        None => known = false,
                    }
                }
            }
            if known {
                density[combination] = if active {
                    1.0 - spectrum::integrate(&[1.0; SAMPLES], Some(&*transmission))[1] / open
                } else {
                    0.0
                };
            }
            let mut flags = Default::default();
            fixed[combination] = match &path.source {
                Source::Unknown => None,
                Source::Fixed(appearance) => {
                    appearance.contribution(1.0, active, known, &transmission, &mut flags)
                }
                Source::Additive(_) => Some([0.0; 3]),
            };
            if let Source::Additive(list) = &path.source {
                for (index, emitter) in list.iter().enumerate() {
                    if self.emitter_active(combination, index) {
                        basis[combination * emitters + index] = emitter.appearance.contribution(
                            1.0,
                            active,
                            known,
                            &transmission,
                            &mut flags,
                        );
                    }
                }
            }
        }
        self.basis = basis.into_boxed_slice();
        self.fixed = fixed.into_boxed_slice();
        self.density = density.into_boxed_slice();
    }

    pub fn basis_at(&self, combination: usize, emitter: usize) -> Option<[f64; 3]> {
        self.basis[combination * self.emitters.len() + emitter]
    }
}
