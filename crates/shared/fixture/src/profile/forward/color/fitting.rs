//! Destination fitting of one semantic Color intent through a head's compiled optical path.
//!
//! This is the inverse companion of [`CompiledColorForward`]. Compilation enumerates each head's
//! known steady discrete states (filter samples, alternative emitter functions) once and stores
//! the per-unit XYZ every combination produces, computed by the same compiled appearances and
//! spectral integration the forward model uses. Solving is pure and allocation-free: the caller
//! owns the workspace and the output buffers.
//!
//! Order of work for one request:
//! 1. UV is resolved first as a common normalized native drive (clamped per emitter) and frozen.
//!    It is never a visible fitting variable, never scaled by `relativeOutput` and never borrowed
//!    to make violet. Known UV visible leakage stays a fixed offset: the visible solve fits the
//!    chromaticity of the total output (visible emitters plus that offset).
//! 2. Wheel constraints that belong to this head's native identity pin their raw values.
//! 3. Every allowed combination is solved (continuous visible emitters by bounded least squares,
//!    chromaticity first, then luminance, then allocation) and ranked; measured whole-path
//!    recipes compatible with the frozen UV and pins compete as exact candidates.
//! 4. The best few candidates are quantized to native raw values and re-evaluated through the
//!    forward model. Only forward-evaluated output is reported as achieved.
//!
//! The helper never changes Intensity or non-Color controls, never publishes, and never writes
//! the achieved approximation back into the request. Family lifecycle belongs to the caller.
mod run;
mod solve;
mod tables;
mod white;

pub use tables::{
    COLOR_FIT_MAX_COMBINATIONS, COLOR_FIT_MAX_CONTINUOUS_COMBINATIONS, COLOR_FIT_MAX_PATH_EMITTERS,
    COLOR_FIT_MAX_TABLE_ENTRIES, COLOR_FIT_MAX_VISIBLE_EMITTERS,
};
pub use white::{requested_visible_xyz, white_target_xyz};

use super::{ColorForwardFlags, ColorForwardResult, CompiledColorForward};
use crate::{
    FixtureProfile, InstalledColorCalibration, InstalledFixtureAppearance, PhysicalDataQuality,
    ProfileError,
};
use light_core::Xyz;
use light_core::programming::ColorIntent;
use tables::{HeadTables, TableStatus};
use uuid::Uuid;

/// Candidates quantized and re-evaluated through the forward model per solve.
pub const COLOR_FIT_REEVALUATED_CANDIDATES: usize = 4;
/// Matches the semantic intent's wheel-constraint limit.
pub const COLOR_FIT_MAX_CONSTRAINTS: usize = 32;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ColorFitStatus {
    #[default]
    NotEvaluated,
    /// Native writes are proposed; see the visible and UV parts for their own outcomes.
    Fitted,
    /// The intent failed validation or still carries unresolved spreads. Nothing is written.
    InvalidRequest,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum VisibleFitStatus {
    #[default]
    NotEvaluated,
    /// The forward model predicts the complete visible output of the proposed writes.
    Fitted,
    /// Visible controls are written, but the total prediction is incomplete (unknown UV
    /// leakage, an unmodeled control or an unknown filter state).
    PredictionIncomplete,
    /// No candidate has a known visible appearance; visible controls are retained.
    UnknownAppearance,
    /// The path exceeds the bounded candidate tables; visible controls are retained.
    CandidateLimit,
}

/// Chromaticity match of the forward-evaluated output, independent of data quality.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ColorMatch {
    /// Within [`light_core::color_intent::EXACT_DELTA_UV`] (or black for a black request).
    Exact,
    /// Within [`light_core::color_intent::APPROXIMATE_DELTA_UV`].
    Approximate,
    OutOfGamut,
    #[default]
    Unknown,
}

/// Luminance at or below which a complete forward output matches a black target. Shared by the
/// fitter and by measured comparisons of replayed native output, so both classify alike.
pub const COLOR_MATCH_BLACK_Y: f64 = 1e-9;

/// CIE 1976 u′v′ of an XYZ in f64 (denominator above 1e-12), `None` for black: the fitter's
/// chromaticity metric, which keeps dim colours chromatic far below `f32` thresholds.
pub fn measured_chromaticity(value: Xyz) -> Option<(f64, f64)> {
    solve::chromaticity(white::xyz_array(value))
}

/// Δu′v′ between two XYZ values by the fitter's metric, `None` when either is black.
pub fn measured_delta_uv(achieved: Xyz, target: Xyz) -> Option<f64> {
    solve::delta_uv(white::xyz_array(achieved), white::xyz_array(target))
}

/// The fitter's [`ColorMatch`] of a complete forward-evaluated output against a target.
pub fn measured_color_match(achieved: Xyz, target: Xyz) -> ColorMatch {
    solve::color_match(white::xyz_array(achieved), white::xyz_array(target))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VisibleFit {
    pub status: VisibleFitStatus,
    /// Forward-evaluated visible XYZ, only when the prediction is complete.
    pub achieved: Option<Xyz>,
    /// Modeled contributions only; incomplete does not mean black.
    pub known_xyz: Xyz,
    pub delta_uv: Option<f64>,
    /// Achieved Y divided by requested Y, when the request is not black.
    pub luminance_ratio: Option<f64>,
    pub color_match: ColorMatch,
    /// Luminance differs from the request by more than 1% (or a black request is not black).
    pub luminance_limited: bool,
    /// Forward data quality of the proposed native output.
    pub data_quality: PhysicalDataQuality,
    /// Nominal or uncalibrated data (Estimated/Unknown), never presented as a measured match.
    pub nominal: bool,
}

impl Default for VisibleFit {
    fn default() -> Self {
        Self {
            status: VisibleFitStatus::NotEvaluated,
            achieved: None,
            known_xyz: Xyz {
                x: 0.,
                y: 0.,
                z: 0.,
            },
            delta_uv: None,
            luminance_ratio: None,
            color_match: ColorMatch::Unknown,
            luminance_limited: false,
            data_quality: PhysicalDataQuality::Unknown,
            nominal: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum UvFitStatus {
    /// Zero request on a head without UV emitters.
    #[default]
    NotRequested,
    /// Every UV emitter is driven at the requested amount (zero actively closes them).
    Applied,
    /// Nonzero request on a head without UV emitters. The request stays stored; no violet
    /// substitute is produced.
    Unsupported,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UvFit {
    pub status: UvFitStatus,
    pub requested: f32,
    /// Forward-evaluated common normalized drive; None for unequal banks or no UV.
    pub achieved_drive: Option<f64>,
    /// Some emitter's declared maximum is below the request.
    pub clipped: bool,
    /// Visible leakage of every driven UV emitter is known (a zero drive is always known).
    pub appearance_known: bool,
}

/// Why a Color-owned function receives this write.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColorWriteRole {
    VisibleEmitter {
        emitter_id: Uuid,
    },
    /// Unused, non-visible or appearance-unknown emitter actively closed.
    ParkedEmitter {
        emitter_id: Uuid,
    },
    Ultraviolet {
        emitter_id: Uuid,
    },
    Filter {
        filter_id: Uuid,
    },
    /// Raw value pinned by a matching wheel constraint.
    Constrained,
    /// Raw value from a measured whole-path recipe.
    MeasuredRecipe,
}

/// A proposed native write, addressed for [`crate::FixtureModeEncodingPlan::encode_split_by_index`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ColorControlWrite {
    pub channel_index: u32,
    pub channel_id: Uuid,
    /// Native function containing `raw`; nil only if the profile has no such function.
    pub function_id: Uuid,
    pub split: u16,
    pub raw: u32,
    pub role: ColorWriteRole,
    /// The channel belongs to a master-shared head; writing it affects every inheriting head.
    pub shared: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColorRetainReason {
    /// The control has no authored optical binding; its value is left to the caller.
    Unmodeled,
    UnknownAppearance,
    CandidateLimit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ColorRetainedControl {
    pub channel_index: u32,
    pub channel_id: Uuid,
    pub reason: ColorRetainReason,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColorConstraintStatus {
    /// Pinned to a known steady filter state.
    Applied,
    /// Pinned, but the raw value lies in an unmodeled or appearance-unknown range.
    AppliedUnknownState,
    /// The constraint belongs to another fixture, mode, head or native layout. Not applied here.
    SourceMismatch,
    /// The channel is not a Color control of this head.
    UnknownControl,
    /// The function does not exist or does not contain the raw value.
    OutOfFunction,
    /// The channel is an emitter or UV control, not a filter/wheel.
    NotAFilter,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ColorConstraintFit {
    /// Position in `ColorIntent::wheel_constraints`.
    pub index: u16,
    pub channel_id: Uuid,
    pub status: ColorConstraintStatus,
}

/// Passive capability notes, not operator errors.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ColorFitLimitations(pub u32);
impl ColorFitLimitations {
    pub const UNKNOWN_EMITTER_PARKED: Self = Self(1);
    pub const UNKNOWN_UV_APPEARANCE: Self = Self(2);
    pub const UNMODELED_CONTROL: Self = Self(4);
    pub const UNKNOWN_FILTER: Self = Self(8);
    pub const UV_CLIPPED: Self = Self(16);
    pub const UV_UNSUPPORTED: Self = Self(32);
    pub const CONSTRAINT_REJECTED: Self = Self(64);
    pub const LUMINANCE_LIMITED: Self = Self(128);
    pub const OUT_OF_GAMUT: Self = Self(256);
    pub const BLACK_UNATTAINABLE: Self = Self(512);
    pub const SHARED_CONTROL: Self = Self(1024);
    pub const MEASURED_RECIPE: Self = Self(2048);
    pub const NO_VISIBLE_EMITTERS: Self = Self(4096);
    pub const CANDIDATE_LIMIT: Self = Self(8192);
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
    fn add(&mut self, other: Self) {
        self.0 |= other.0;
    }
}

/// Measured work of the last solve, published so callers can report per-frame cost. These are
/// counters only: they describe what one solve did and imply no time budget.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ColorFitWork {
    /// Enumerated combinations whose continuous visible emitters were solved.
    pub visible_solves: u32,
    /// Visible solves with a fixed known offset (fixed source or frozen UV leakage), which run
    /// the bounded luminance-level search (16-sample grid plus 24 golden-section refinements).
    pub fixed_offset_solves: u32,
    /// Chromaticity-first level solves (two bounded QPs each) across every visible solve.
    pub level_solves: u32,
    /// Forward-model evaluations: candidate re-evaluation plus the reported proposal.
    pub forward_evaluations: u32,
}

/// One Color-owned control of a head's optical path, in fitting order. The set is the complete
/// native footprint the fitter decides for the head: every entry is either written or retained.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ColorFitControl {
    pub channel_index: u32,
    pub channel_id: Uuid,
    pub split: u16,
    /// Master-shared channel of another head.
    pub shared: bool,
    /// The channel has an authored optical binding.
    pub modeled: bool,
    /// Largest raw value of the channel.
    pub raw_max: u32,
    /// Neutral raw for a control the fitter retains: the closed drive of the control's first
    /// emitter (direction-aware), otherwise the profile default.
    pub park_raw: u32,
}

/// One head's outcome. Created by [`CompiledColorFitting::create_output`] and reused.
#[derive(Clone, Debug, PartialEq)]
pub struct ColorFitResult {
    pub head_id: Uuid,
    pub status: ColorFitStatus,
    /// Derived lamp target `relativeOutput × (colored × base + white × white target)`.
    pub requested_visible: Option<Xyz>,
    /// White target at Y = 1 from Kelvin/Duv.
    pub requested_white: Option<Xyz>,
    pub requested_relative_output: f32,
    pub visible: VisibleFit,
    pub uv: UvFit,
    /// Quality of the total prediction: Unknown whenever any active contribution is unknown.
    pub total_quality: PhysicalDataQuality,
    pub flags: ColorForwardFlags,
    pub limitations: ColorFitLimitations,
    /// Combinations and measured recipes ranked in the last solve.
    pub candidates_ranked: u32,
    pub work: ColorFitWork,
    pub writes: Vec<ColorControlWrite>,
    pub retained: Vec<ColorRetainedControl>,
    pub constraints: Vec<ColorConstraintFit>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColorFitInputError {
    HeadIndex,
    ChannelCount,
    RawOutOfRange,
    OutputLayout,
    WorkspaceLayout,
}

/// Caller-owned scratch reused across solves; created by [`CompiledColorFitting::create_workspace`].
#[derive(Clone, Debug)]
pub struct ColorFitWorkspace {
    raw: Vec<u32>,
    forward: Vec<ColorForwardResult>,
    scratch: run::Scratch,
}

impl ColorFitWorkspace {
    /// Current native values with the last solve's writes applied. Unrelated channels,
    /// including Intensity, keep their current values.
    pub fn proposed_raw(&self) -> &[u32] {
        &self.raw
    }

    /// Forward evaluation of [`Self::proposed_raw`] for the head of the last solve.
    pub fn forward(&self, head: usize) -> Option<&ColorForwardResult> {
        self.forward.get(head)
    }
}

/// Precompiled semantic Color destination fitting for one fixture mode.
#[derive(Clone, Debug)]
pub struct CompiledColorFitting {
    forward: CompiledColorForward,
    maxima: Box<[u32]>,
    heads: Box<[HeadTables]>,
    max_controls: usize,
}

impl CompiledColorFitting {
    /// Compiles the forward model and the candidate tables. `Ok(None)` without a physical model.
    pub fn compile(
        profile: &FixtureProfile,
        mode_id: Uuid,
        installed: Option<&InstalledColorCalibration>,
    ) -> Result<Option<Self>, ProfileError> {
        let Some(forward) = CompiledColorForward::compile(profile, mode_id, installed)? else {
            return Ok(None);
        };
        Self::from_forward(profile, mode_id, forward).map(Some)
    }

    /// Builds tables from an already compiled forward model of the same profile and mode, for
    /// callers that compile with a calibration context.
    pub fn from_forward(
        profile: &FixtureProfile,
        mode_id: Uuid,
        forward: CompiledColorForward,
    ) -> Result<Self, ProfileError> {
        let invalid = |m: &str| ProfileError::Invalid(format!("Color fitting: {m}"));
        let mode = profile
            .mode(mode_id)
            .ok_or_else(|| invalid("missing mode"))?;
        let model = mode
            .color_physical
            .as_ref()
            .ok_or_else(|| invalid("missing physical model"))?;
        if forward.raw_maxima.len() != mode.channels.len()
            || forward.paths.len() != model.paths.len()
        {
            return Err(invalid("forward model was compiled for another mode"));
        }
        // Identity derivation validates the complete profile; without it no constraint applies.
        let identities = profile.native_color_identities(mode_id).unwrap_or_default();
        let heads = forward
            .paths
            .iter()
            .zip(&model.paths)
            .map(|(path, authored)| {
                let head_forward = forward
                    .for_head(path.head)
                    .ok_or_else(|| invalid("missing head path"))?;
                let identity = identities.iter().find(|i| i.head_id == path.head).cloned();
                let filter_ids = authored.filters.iter().map(|f| f.id).collect();
                HeadTables::compile(mode, head_forward, identity, authored.id, filter_ids)
            })
            .collect::<Result<Box<[_]>, _>>()?;
        Ok(Self {
            maxima: forward.raw_maxima.clone(),
            max_controls: heads.iter().map(|h| h.controls.len()).max().unwrap_or(0),
            heads,
            forward,
        })
    }

    /// Applies the installed source/gel override exactly as the forward model does.
    pub fn with_installed_appearance(mut self, appearance: &InstalledFixtureAppearance) -> Self {
        self.forward = self.forward.with_installed_appearance(appearance);
        for head in self.heads.iter_mut() {
            head.forward = head.forward.clone().with_installed_appearance(appearance);
        }
        self
    }

    /// The forward model used to verify every proposed write.
    pub fn forward(&self) -> &CompiledColorForward {
        &self.forward
    }

    pub fn head_count(&self) -> usize {
        self.heads.len()
    }

    /// Authored optical path identity of one head, for native-identity bookkeeping.
    pub fn path_id(&self, head: usize) -> Option<Uuid> {
        self.heads.get(head).map(|h| h.path_id)
    }

    pub fn head_index(&self, head_id: Uuid) -> Option<usize> {
        self.heads.iter().position(|h| h.head_id == head_id)
    }

    /// Every raw channel a fit of one head reads besides the whole-vector validation of
    /// [`Self::accepts_raw`]: its fitting controls and every input of its forward model. Two
    /// fits of the same head and intent whose raw values agree on these channels, and that both
    /// pass `accepts_raw`, produce identical results.
    pub fn head_input_channels(&self, head: usize) -> Option<Box<[usize]>> {
        let tables = self.heads.get(head)?;
        let mut channels = tables
            .forward
            .head_input_channels(tables.head_id)?
            .into_vec();
        channels.extend(tables.controls.iter().map(|control| control.channel));
        channels.sort_unstable();
        channels.dedup();
        Some(channels.into_boxed_slice())
    }

    /// The whole-vector layout and range validation every fit applies to `current`.
    pub fn accepts_raw(&self, current: &[u32]) -> bool {
        current.len() == self.maxima.len()
            && current
                .iter()
                .zip(&self.maxima)
                .all(|(value, max)| value <= max)
    }

    /// The complete Color-owned native footprint of one head, in fitting order.
    pub fn controls(
        &self,
        head: usize,
    ) -> Option<impl ExactSizeIterator<Item = ColorFitControl> + '_> {
        let tables = self.heads.get(head)?;
        Some(tables.controls.iter().map(move |control| ColorFitControl {
            channel_index: control.channel as u32,
            channel_id: control.channel_id,
            split: control.split,
            shared: control.shared,
            modeled: control.modeled,
            raw_max: self.maxima[control.channel],
            park_raw: control.park_raw,
        }))
    }

    /// Enumerated discrete-state combinations for one head (0 when over the table limits or
    /// the source is unknown).
    pub fn candidate_combinations(&self, head: usize) -> usize {
        self.heads.get(head).map_or(0, |h| h.combinations)
    }

    /// Whether the head has continuously driven visible emitters. Without them it can only
    /// choose among discrete filter states and observed recipes (a colour wheel).
    pub fn has_visible_emitters(&self, head: usize) -> bool {
        self.heads
            .get(head)
            .is_some_and(|h| h.has_visible_emitters())
    }

    pub fn create_workspace(&self) -> ColorFitWorkspace {
        ColorFitWorkspace {
            raw: vec![0; self.maxima.len()],
            forward: self
                .heads
                .iter()
                .map(|h| h.forward.create_output().remove(0))
                .collect(),
            scratch: run::Scratch::new(self.max_controls),
        }
    }

    pub fn create_output(&self, head: usize) -> Option<ColorFitResult> {
        let tables = self.heads.get(head)?;
        let controls = tables.controls.len();
        Some(ColorFitResult {
            head_id: tables.head_id,
            status: ColorFitStatus::NotEvaluated,
            requested_visible: None,
            requested_white: None,
            requested_relative_output: 0.,
            visible: VisibleFit::default(),
            uv: UvFit::default(),
            total_quality: PhysicalDataQuality::Unknown,
            flags: ColorForwardFlags::default(),
            limitations: ColorFitLimitations::default(),
            candidates_ranked: 0,
            work: ColorFitWork::default(),
            writes: Vec::with_capacity(controls),
            retained: Vec::with_capacity(controls),
            constraints: Vec::with_capacity(COLOR_FIT_MAX_CONSTRAINTS),
        })
    }

    fn validate(
        &self,
        head: usize,
        current: &[u32],
        workspace: &ColorFitWorkspace,
        output: &ColorFitResult,
    ) -> Result<&HeadTables, ColorFitInputError> {
        let tables = self.heads.get(head).ok_or(ColorFitInputError::HeadIndex)?;
        if current.len() != self.maxima.len() {
            return Err(ColorFitInputError::ChannelCount);
        }
        if current.iter().zip(&self.maxima).any(|(v, max)| v > max) {
            return Err(ColorFitInputError::RawOutOfRange);
        }
        let controls = tables.controls.len();
        if output.head_id != tables.head_id
            || output.writes.capacity() < controls
            || output.retained.capacity() < controls
            || output.constraints.capacity() < COLOR_FIT_MAX_CONSTRAINTS
        {
            return Err(ColorFitInputError::OutputLayout);
        }
        if workspace.raw.len() != self.maxima.len()
            || workspace.forward.len() != self.heads.len()
            || workspace.forward[head].head_id != tables.head_id
            || !workspace.scratch.fits(self.max_controls)
        {
            return Err(ColorFitInputError::WorkspaceLayout);
        }
        Ok(tables)
    }

    /// Fits one head to one local semantic intent against the current native values.
    ///
    /// Only the head's Color-owned controls receive writes; Intensity and unrelated channels
    /// keep their current values in [`ColorFitWorkspace::proposed_raw`]. The request is read,
    /// never modified; requested and achieved values are reported separately.
    pub fn fit(
        &self,
        head: usize,
        current: &[u32],
        intent: &ColorIntent,
        workspace: &mut ColorFitWorkspace,
        output: &mut ColorFitResult,
    ) -> Result<(), ColorFitInputError> {
        let tables = self.validate(head, current, workspace, output)?;
        run::reset(output, intent);
        workspace.raw.copy_from_slice(current);
        if intent.validate().is_err() || !intent.spreads.is_empty() {
            output.status = ColorFitStatus::InvalidRequest;
            return Ok(());
        }
        let Ok(request) = white::derive(intent) else {
            output.status = ColorFitStatus::InvalidRequest;
            return Ok(());
        };
        output.status = ColorFitStatus::Fitted;
        output.requested_visible = Some(request.visible);
        output.requested_white = Some(request.white);
        let visible_state = match tables.status {
            TableStatus::Ready => None,
            TableStatus::CandidateLimit => Some(VisibleFitStatus::CandidateLimit),
        };
        run::Solve {
            tables,
            current,
            intent,
            request: &request,
            raw: &mut workspace.raw,
            forward: &mut workspace.forward[head],
            scratch: &mut workspace.scratch,
            output,
        }
        .run(visible_state);
        Ok(())
    }
}
