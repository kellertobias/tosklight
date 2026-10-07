//! One solve: constraints, frozen UV, candidate ranking, forward re-evaluation and reporting.
use super::super::{ColorForwardFlags, ColorForwardResult, Source};
use super::solve::{
    MAX_VISIBLE_VARIABLES, VisibleProblem, chromaticity, color_match, delta_uv, solve_visible,
};
use super::tables::{COLOR_FIT_MAX_PATH_EMITTERS, EmitterRole, HeadTables, StateKind};
use super::white::{DerivedRequest, xyz_array};
use super::{
    COLOR_FIT_REEVALUATED_CANDIDATES as K, ColorConstraintFit, ColorConstraintStatus,
    ColorControlWrite, ColorFitLimitations as Limit, ColorFitResult, ColorFitStatus, ColorMatch,
    ColorRetainReason, ColorRetainedControl, ColorWriteRole, UvFit, UvFitStatus, VisibleFit,
    VisibleFitStatus,
};
use crate::PhysicalDataQuality;
use light_core::programming::{ColorIntent, ColorWheelConstraint};
use uuid::Uuid;

/// Chromaticity differences below this are ties decided by luminance.
const CHROMA_TIE: f64 = 5e-4;
/// Relative luminance differences below this are ties decided by filter density and stability.
const LUMINANCE_TIE: f64 = 5e-3;
const LUMINANCE_MATCH: f64 = 0.01;
const BLACK: f64 = super::COLOR_MATCH_BLACK_Y;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Pin {
    Free,
    Known { raw: u32, state: usize },
    Opaque { raw: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum PickSource {
    Combination(usize),
    Measured(usize),
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Score {
    known: bool,
    chroma: f64,
    luminance: f64,
    density: f64,
    moved: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Pick {
    source: PickSource,
    score: Score,
    /// Continuous native drive per path emitter, before quantization.
    drives: [f64; COLOR_FIT_MAX_PATH_EMITTERS],
    parked_unknown: bool,
}

const EMPTY_PICK: Pick = Pick {
    source: PickSource::Combination(0),
    score: Score {
        known: false,
        chroma: 0.,
        luminance: 0.,
        density: 0.,
        moved: 0,
    },
    drives: [0.; COLOR_FIT_MAX_PATH_EMITTERS],
    parked_unknown: false,
};

/// Bounded per-control scratch sized once for the largest head.
#[derive(Clone, Debug)]
pub(super) struct Scratch {
    pins: Vec<Pin>,
    uv_raw: Vec<Option<u32>>,
    uv_drive: Vec<f64>,
    current_state: Vec<Option<usize>>,
    picks: [Pick; K],
    count: usize,
}

impl Scratch {
    pub fn new(controls: usize) -> Self {
        Self {
            pins: vec![Pin::Free; controls],
            uv_raw: vec![None; controls],
            uv_drive: vec![0.; controls],
            current_state: vec![None; controls],
            picks: [EMPTY_PICK; K],
            count: 0,
        }
    }
    pub fn fits(&self, controls: usize) -> bool {
        self.pins.len() >= controls
            && self.uv_raw.len() >= controls
            && self.uv_drive.len() >= controls
            && self.current_state.len() >= controls
    }
}

pub(super) fn reset(output: &mut ColorFitResult, intent: &ColorIntent) {
    output.status = ColorFitStatus::NotEvaluated;
    output.requested_visible = None;
    output.requested_white = None;
    output.requested_relative_output = intent.relative_output;
    output.visible = VisibleFit::default();
    output.uv = UvFit {
        requested: intent.uv.amount,
        ..UvFit::default()
    };
    output.total_quality = PhysicalDataQuality::Unknown;
    output.flags = ColorForwardFlags::default();
    output.limitations = Limit::default();
    output.candidates_ranked = 0;
    output.work = Default::default();
    output.writes.clear();
    output.retained.clear();
    output.constraints.clear();
}

/// Maps a continuous drive into the exact native function, respecting direction, declared
/// maximum and integer resolution. Returns the raw value and its actual normalized drive.
pub(super) fn quantize(from: u32, to: u32, reversed: bool, drive: f64, maximum: f64) -> (u32, f64) {
    let span = u64::from(to - from);
    let wanted = if drive.is_finite() {
        drive.clamp(0., maximum)
    } else {
        0.
    };
    let mut offset = ((wanted * span as f64).round() as u64).min(span);
    if offset > 0 && offset as f64 > maximum * span as f64 {
        offset -= 1;
    }
    let offset32 = offset as u32;
    let raw = if reversed {
        to - offset32
    } else {
        from + offset32
    };
    (raw, offset as f64 / span as f64)
}

fn score(achieved: [f64; 3], target: [f64; 3], known: bool, density: f64, moved: u32) -> Score {
    let (chroma, luminance) = match chromaticity(target) {
        None => (0., achieved[1].max(0.)),
        Some(_) => (
            delta_uv(achieved, target).unwrap_or(1.),
            (achieved[1] - target[1]).abs() / target[1].max(1e-12),
        ),
    };
    Score {
        known,
        chroma,
        luminance,
        density,
        moved,
    }
}

/// Known first, then chromaticity, luminance, clearer filters and fewer moved discrete controls.
fn better(a: &Score, b: &Score) -> bool {
    if a.known != b.known {
        return a.known;
    }
    if (a.chroma - b.chroma).abs() > CHROMA_TIE {
        return a.chroma < b.chroma;
    }
    if (a.luminance - b.luminance).abs() > LUMINANCE_TIE {
        return a.luminance < b.luminance;
    }
    if (a.density - b.density).abs() > 1e-6 {
        return a.density < b.density;
    }
    a.moved < b.moved
}

pub(super) struct Solve<'a> {
    pub tables: &'a HeadTables,
    pub current: &'a [u32],
    pub intent: &'a ColorIntent,
    pub request: &'a DerivedRequest,
    pub raw: &'a mut Vec<u32>,
    pub forward: &'a mut ColorForwardResult,
    pub scratch: &'a mut Scratch,
    pub output: &'a mut ColorFitResult,
}

impl Solve<'_> {
    pub fn run(mut self, mut visible_state: Option<VisibleFitStatus>) {
        self.constraints();
        self.uv();
        for (position, control) in self.tables.controls.iter().enumerate() {
            self.scratch.current_state[position] = control.state_of(self.current[control.channel]);
        }
        let controls = self.tables.controls.len();
        if self.scratch.pins[..controls]
            .iter()
            .any(|p| matches!(p, Pin::Opaque { .. }))
        {
            visible_state = visible_state.or(Some(VisibleFitStatus::UnknownAppearance));
        }
        let mut winner = None;
        if visible_state.is_none() {
            self.enumerate();
            winner = self.reevaluate().filter(|p| p.score.known);
            if winner.is_none() {
                visible_state = Some(VisibleFitStatus::UnknownAppearance);
            }
        }
        let reason = match visible_state {
            Some(VisibleFitStatus::CandidateLimit) => ColorRetainReason::CandidateLimit,
            _ => ColorRetainReason::UnknownAppearance,
        };
        self.apply(winner.as_ref(), true, reason);
        self.evaluate();
        self.report(winner.as_ref(), visible_state);
    }

    fn constraints(&mut self) {
        let controls = self.tables.controls.len();
        self.scratch.pins[..controls].fill(Pin::Free);
        for (index, constraint) in self.intent.wheel_constraints.iter().enumerate() {
            let status = self.constraint(constraint);
            if !matches!(
                status,
                ColorConstraintStatus::Applied | ColorConstraintStatus::AppliedUnknownState
            ) {
                self.output.limitations.add(Limit::CONSTRAINT_REJECTED);
            }
            self.output.constraints.push(ColorConstraintFit {
                index: index as u16,
                channel_id: constraint.value.channel_id,
                status,
            });
        }
    }

    /// A constraint stays pinned to its own native source; it is never reinterpreted as a
    /// slot index of a different wheel.
    fn constraint(&mut self, constraint: &ColorWheelConstraint) -> ColorConstraintStatus {
        let source = &constraint.source;
        let same = self.tables.identity.as_ref().is_some_and(|id| {
            id.profile_id == source.profile_id
                && id.mode_id == source.mode_id
                && id.head_id == source.head_id
                && id.path_id == source.path_id
                && id.native_layout_signature == source.native_layout_signature
        });
        if !same {
            return ColorConstraintStatus::SourceMismatch;
        }
        let value = &constraint.value;
        let Some(position) = self
            .tables
            .controls
            .iter()
            .position(|c| c.channel_id == value.channel_id)
        else {
            return ColorConstraintStatus::UnknownControl;
        };
        let control = &self.tables.controls[position];
        if control.function_of(value.raw) != Some(value.function_id) {
            return ColorConstraintStatus::OutOfFunction;
        }
        if control.uv.is_some() {
            return ColorConstraintStatus::NotAFilter;
        }
        match control.state_of(value.raw) {
            Some(state) if matches!(control.states[state].kind, StateKind::Emitter(_)) => {
                ColorConstraintStatus::NotAFilter
            }
            Some(state) => {
                self.scratch.pins[position] = Pin::Known {
                    raw: value.raw,
                    state,
                };
                ColorConstraintStatus::Applied
            }
            None => {
                self.scratch.pins[position] = Pin::Opaque { raw: value.raw };
                ColorConstraintStatus::AppliedUnknownState
            }
        }
    }

    fn uv_appearance_known(&self, emitter: usize) -> bool {
        match &self.tables.forward.paths[0].source {
            Source::Additive(list) => list
                .get(emitter)
                .is_some_and(|e| e.appearance.xyz.is_some() && !e.appearance.inconsistent),
            _ => false,
        }
    }

    /// UV is resolved before visible fitting and never changes afterwards.
    fn uv(&mut self) {
        let amount = f64::from(self.intent.uv.amount);
        let (mut any, mut clipped, mut known) = (false, false, true);
        for (position, control) in self.tables.controls.iter().enumerate() {
            self.scratch.uv_raw[position] = None;
            self.scratch.uv_drive[position] = 0.;
            let Some(index) = control.uv else { continue };
            let emitter = &self.tables.emitters[index];
            any = true;
            clipped |= amount > emitter.maximum;
            let (raw, drive) = quantize(
                emitter.from,
                emitter.to,
                emitter.reversed,
                amount,
                emitter.maximum,
            );
            self.scratch.uv_raw[position] = Some(raw);
            self.scratch.uv_drive[position] = drive;
            if drive > 0. && !self.uv_appearance_known(index) {
                known = false;
            }
        }
        let uv = &mut self.output.uv;
        uv.clipped = clipped;
        uv.appearance_known = known;
        uv.status = match (any, amount > 0.) {
            (true, _) => UvFitStatus::Applied,
            (false, true) => UvFitStatus::Unsupported,
            (false, false) => UvFitStatus::NotRequested,
        };
        if uv.status == UvFitStatus::Unsupported {
            self.output.limitations.add(Limit::UV_UNSUPPORTED);
        }
        if clipped {
            self.output.limitations.add(Limit::UV_CLIPPED);
        }
        if !known {
            self.output.limitations.add(Limit::UNKNOWN_UV_APPEARANCE);
        }
    }

    fn allowed(&self, combination: usize) -> bool {
        self.tables
            .dimensions
            .iter()
            .enumerate()
            .all(|(d, &position)| match self.scratch.pins[position] {
                Pin::Known { state, .. } => self.tables.digit(combination, d) == state,
                _ => true,
            })
    }

    fn moved(&self, combination: usize) -> u32 {
        self.tables
            .dimensions
            .iter()
            .enumerate()
            .filter(|&(d, &position)| {
                self.scratch.current_state[position] != Some(self.tables.digit(combination, d))
            })
            .count() as u32
    }

    fn combination(&mut self, combination: usize) -> Pick {
        let tables = self.tables;
        let target = xyz_array(self.request.visible);
        let (mut offset, mut known) = match tables.fixed[combination] {
            Some(value) => (value, true),
            None => ([0.; 3], false),
        };
        let mut columns = [[0.; 3]; MAX_VISIBLE_VARIABLES];
        let mut white = [false; MAX_VISIBLE_VARIABLES];
        let mut owners = [0usize; MAX_VISIBLE_VARIABLES];
        let (mut count, mut parked_unknown) = (0, false);
        for (index, emitter) in tables.emitters.iter().enumerate() {
            if !tables.emitter_active(combination, index) {
                continue;
            }
            let basis = tables.basis_at(combination, index);
            match (emitter.role, basis) {
                (EmitterRole::Ultraviolet, Some(b)) => {
                    let drive = self.scratch.uv_drive[emitter.control];
                    let amount = drive.powf(emitter.exponent) * emitter.gain;
                    for c in 0..3 {
                        offset[c] += b[c] * amount;
                    }
                }
                (EmitterRole::Colored | EmitterRole::White, Some(b)) => {
                    let upper = emitter.upper_amount();
                    if upper > 0. && b != [0.; 3] {
                        columns[count] = b.map(|v| v * upper);
                        white[count] = emitter.role == EmitterRole::White;
                        owners[count] = index;
                        count += 1;
                    }
                }
                (EmitterRole::Colored | EmitterRole::White, None) => parked_unknown = true,
                _ => {}
            }
        }
        if (parked_unknown && count == 0) || tables.opaque {
            known = false;
        }
        let mut drives = [0.; COLOR_FIT_MAX_PATH_EMITTERS];
        let mut predicted = offset;
        if count > 0 {
            let mut x = [0.; MAX_VISIBLE_VARIABLES];
            let problem = VisibleProblem {
                columns: &columns[..count],
                white: &white[..count],
                // The objective is the final chromaticity of visible plus fixed contributions.
                target,
                offset,
                colored_part: self.request.colored_part,
                white_part: self.request.white_part,
                allocation: self.intent.allocation,
            };
            let work = solve_visible(&problem, &mut x);
            self.output.work.visible_solves += 1;
            self.output.work.level_solves += work.levels;
            self.output.work.fixed_offset_solves += u32::from(work.fixed_offset);
            for k in 0..count {
                let amount = x[k].clamp(0., 1.);
                for c in 0..3 {
                    predicted[c] += columns[k][c] * amount;
                }
                let emitter = &tables.emitters[owners[k]];
                drives[owners[k]] = emitter.maximum * amount.powf(1. / emitter.exponent);
            }
        }
        Pick {
            source: PickSource::Combination(combination),
            score: score(
                predicted,
                target,
                known,
                tables.density[combination],
                self.moved(combination),
            ),
            drives,
            parked_unknown,
        }
    }

    fn insert(&mut self, pick: Pick) {
        let count = self.scratch.count;
        let at = (0..count)
            .find(|&i| better(&pick.score, &self.scratch.picks[i].score))
            .unwrap_or(count);
        if at >= K {
            return;
        }
        let end = count.min(K - 1);
        self.scratch.picks.copy_within(at..end, at + 1);
        self.scratch.picks[at] = pick;
        self.scratch.count = (count + 1).min(K);
    }

    fn measured_eligible(&self, recipe: &[u32]) -> bool {
        self.tables
            .controls
            .iter()
            .enumerate()
            .all(|(position, _)| {
                match (self.scratch.uv_raw[position], self.scratch.pins[position]) {
                    (Some(raw), _) | (None, Pin::Known { raw, .. } | Pin::Opaque { raw }) => {
                        recipe[position] == raw
                    }
                    (None, Pin::Free) => true,
                }
            })
    }

    fn enumerate(&mut self) {
        self.scratch.count = 0;
        let mut ranked = 0u32;
        for combination in 0..self.tables.combinations {
            if self.allowed(combination) {
                ranked += 1;
                let pick = self.combination(combination);
                self.insert(pick);
            }
        }
        let target = xyz_array(self.request.visible);
        for (index, recipe) in self.tables.measured.iter().enumerate() {
            if !self.measured_eligible(&recipe.raws) {
                continue;
            }
            ranked += 1;
            let moved = self
                .tables
                .controls
                .iter()
                .enumerate()
                .filter(|&(position, control)| {
                    control.dimension.is_some()
                        && self.scratch.current_state[position]
                            != control.state_of(recipe.raws[position])
                })
                .count() as u32;
            let pick = Pick {
                source: PickSource::Measured(index),
                score: score(recipe.xyz, target, true, 0., moved),
                ..EMPTY_PICK
            };
            self.insert(pick);
        }
        self.output.candidates_ranked = ranked;
    }

    /// Quantizes the best table candidates and ranks them again by forward-evaluated output.
    fn reevaluate(&mut self) -> Option<Pick> {
        let target = xyz_array(self.request.visible);
        let mut best: Option<(Pick, Score)> = None;
        for i in 0..self.scratch.count {
            let pick = self.scratch.picks[i];
            self.apply(Some(&pick), false, ColorRetainReason::UnknownAppearance);
            self.evaluate();
            let achieved = xyz_array(self.forward.known_xyz);
            let forward = score(
                achieved,
                target,
                pick.score.known,
                pick.score.density,
                pick.score.moved,
            );
            if best.as_ref().is_none_or(|(_, s)| better(&forward, s)) {
                best = Some((pick, forward));
            }
        }
        best.map(|(pick, _)| pick)
    }

    fn decide(&self, pick: &Pick, position: usize) -> Option<(u32, Uuid, ColorWriteRole)> {
        let tables = self.tables;
        let control = &tables.controls[position];
        match pick.source {
            PickSource::Measured(index) => {
                let raw = tables.measured[index].raws[position];
                let function = control.function_of(raw).unwrap_or_else(Uuid::nil);
                Some((raw, function, ColorWriteRole::MeasuredRecipe))
            }
            PickSource::Combination(combination) => {
                let state = control.states[tables.digit(combination, control.dimension?)];
                match state.kind {
                    StateKind::Filter { filter, .. } => {
                        // Keep the current raw inside the chosen steady range; otherwise park at
                        // its center, away from neighbouring slot boundaries.
                        let current = self.current[control.channel];
                        let raw = if (state.from..=state.to).contains(&current) {
                            current
                        } else {
                            state.from + (state.to - state.from) / 2
                        };
                        let filter_id = tables.filter_ids.get(filter).copied().unwrap_or_default();
                        Some((raw, state.function_id, ColorWriteRole::Filter { filter_id }))
                    }
                    StateKind::Emitter(index) => {
                        let emitter = &tables.emitters[index];
                        let (raw, _) = quantize(
                            emitter.from,
                            emitter.to,
                            emitter.reversed,
                            pick.drives[index],
                            emitter.maximum,
                        );
                        let known = tables.basis_at(combination, index).is_some();
                        let role = if emitter.visible() && known {
                            ColorWriteRole::VisibleEmitter {
                                emitter_id: emitter.id,
                            }
                        } else {
                            ColorWriteRole::ParkedEmitter {
                                emitter_id: emitter.id,
                            }
                        };
                        Some((raw, emitter.function_id, role))
                    }
                }
            }
        }
    }

    fn apply(&mut self, pick: Option<&Pick>, record: bool, reason: ColorRetainReason) {
        self.raw.copy_from_slice(self.current);
        if record {
            self.output.writes.clear();
            self.output.retained.clear();
        }
        let tables = self.tables;
        for (position, control) in tables.controls.iter().enumerate() {
            let decision = if let Some(raw) = self.scratch.uv_raw[position] {
                let emitter = &tables.emitters[control.uv.unwrap_or_default()];
                Some((
                    raw,
                    emitter.function_id,
                    ColorWriteRole::Ultraviolet {
                        emitter_id: emitter.id,
                    },
                ))
            } else if let Pin::Known { raw, .. } | Pin::Opaque { raw } = self.scratch.pins[position]
            {
                let function = control.function_of(raw).unwrap_or_else(Uuid::nil);
                Some((raw, function, ColorWriteRole::Constrained))
            } else {
                pick.and_then(|p| self.decide(p, position))
            };
            match decision {
                Some((raw, function_id, role)) => {
                    self.raw[control.channel] = raw;
                    if record {
                        self.output.writes.push(ColorControlWrite {
                            channel_index: control.channel as u32,
                            channel_id: control.channel_id,
                            function_id,
                            split: control.split,
                            raw,
                            role,
                            shared: control.shared,
                        });
                    }
                }
                None if record => self.output.retained.push(ColorRetainedControl {
                    channel_index: control.channel as u32,
                    channel_id: control.channel_id,
                    reason: if !control.modeled {
                        ColorRetainReason::Unmodeled
                    } else if pick.is_some() {
                        ColorRetainReason::UnknownAppearance
                    } else {
                        reason
                    },
                }),
                None => {}
            }
        }
    }

    fn evaluate(&mut self) {
        self.output.work.forward_evaluations += 1;
        // Layout and range were validated before the solve; a failure leaves the result unknown.
        if self
            .tables
            .forward
            .evaluate(&self.raw[..], std::slice::from_mut(&mut *self.forward))
            .is_err()
        {
            self.forward.visible_complete = false;
            self.forward.data_quality = PhysicalDataQuality::Unknown;
        }
    }

    fn report(&mut self, winner: Option<&Pick>, visible_state: Option<VisibleFitStatus>) {
        let forward = &*self.forward;
        let target = xyz_array(self.request.visible);
        let known = xyz_array(forward.known_xyz);
        let complete = forward.visible_complete;
        let black_target = chromaticity(target).is_none();
        let visible = &mut self.output.visible;
        visible.status = visible_state.unwrap_or(if complete {
            VisibleFitStatus::Fitted
        } else {
            VisibleFitStatus::PredictionIncomplete
        });
        visible.known_xyz = forward.known_xyz;
        visible.achieved = complete.then_some(forward.known_xyz);
        visible.delta_uv = if black_target {
            None
        } else {
            delta_uv(known, target)
        };
        visible.luminance_ratio = (!black_target && target[1] > 0.).then(|| known[1] / target[1]);
        visible.color_match = if complete {
            color_match(known, target)
        } else {
            ColorMatch::Unknown
        };
        visible.luminance_limited = complete
            && if black_target {
                known[1].abs() > BLACK
            } else {
                visible
                    .luminance_ratio
                    .is_none_or(|r| (r - 1.).abs() > LUMINANCE_MATCH)
            };
        visible.data_quality = forward.data_quality;
        visible.nominal = matches!(
            forward.data_quality,
            PhysicalDataQuality::Unknown | PhysicalDataQuality::Estimated
        );
        self.output.total_quality = if complete {
            forward.data_quality
        } else {
            PhysicalDataQuality::Unknown
        };
        self.output.flags = forward.flags;
        if self.output.uv.status == UvFitStatus::Applied {
            self.output.uv.achieved_drive = forward.portable_uv.map(|uv| uv.amount);
        }
        self.limitations(winner, visible_state, black_target);
    }

    fn limitations(
        &mut self,
        winner: Option<&Pick>,
        visible_state: Option<VisibleFitStatus>,
        black_target: bool,
    ) {
        let output = &*self.output;
        let flags = output.flags;
        let unknown_filter = [
            ColorForwardFlags::UNKNOWN_FILTER,
            ColorForwardFlags::FILTER_SAMPLE_GAP,
            ColorForwardFlags::SPECTRAL_COVERAGE,
        ]
        .into_iter()
        .any(|f| flags.contains(f))
            || self
                .tables
                .controls
                .iter()
                .any(|c| c.modeled && c.uv.is_none() && c.states.is_empty());
        let conditions = [
            (
                winner.is_some_and(|w| w.parked_unknown),
                Limit::UNKNOWN_EMITTER_PARKED,
            ),
            (
                winner.is_some_and(|w| matches!(w.source, PickSource::Measured(_))),
                Limit::MEASURED_RECIPE,
            ),
            (
                flags.contains(ColorForwardFlags::UNMODELED_CONTROL)
                    || output
                        .retained
                        .iter()
                        .any(|r| r.reason == ColorRetainReason::Unmodeled),
                Limit::UNMODELED_CONTROL,
            ),
            (unknown_filter, Limit::UNKNOWN_FILTER),
            (output.visible.luminance_limited, Limit::LUMINANCE_LIMITED),
            (
                black_target && output.visible.luminance_limited,
                Limit::BLACK_UNATTAINABLE,
            ),
            (
                output.visible.color_match == ColorMatch::OutOfGamut && !black_target,
                Limit::OUT_OF_GAMUT,
            ),
            (
                output.writes.iter().any(|w| w.shared),
                Limit::SHARED_CONTROL,
            ),
            (
                matches!(self.tables.forward.paths[0].source, Source::Additive(_))
                    && !self.tables.has_visible_emitters(),
                Limit::NO_VISIBLE_EMITTERS,
            ),
            (
                visible_state == Some(VisibleFitStatus::CandidateLimit),
                Limit::CANDIDATE_LIMIT,
            ),
        ];
        for (condition, limit) in conditions {
            if condition {
                self.output.limitations.add(limit);
            }
        }
    }
}
