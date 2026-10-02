//! Fixture-level destination fitting for independent Focus and Zoom requests.
//!
//! This is the inverse companion of [`CompiledOpticsForward`]. Compilation resolves, per logical
//! head and optical family, the one native control that owns the family (own control first, then
//! a master-shared head's control), its function boundaries and its compiled physical mapping.
//! Solving never allocates: the caller owns the workspace and output buffers.
//!
//! Focus is normalized lens travel (0..=1), never a focal distance. Zoom is the full opening in
//! degrees and keeps the profile's Beam/Field convention; a requested convention is checked, never
//! converted. Every proposed write is re-evaluated through [`CompiledOpticsForward`] so the
//! reported achieved value is what the native output actually produces.
//!
//! This helper does not activate runtime behavior; it only proposes native control writes.
use crate::forward::{CompiledOpticsForward, OpticsForwardResult, OpticsForwardStatus};
use crate::{
    ChannelFunctionBehavior, CompiledPhysicalMapping, FixtureChannel, FixtureMode,
    OpeningConvention, PhysicalDataQuality, PhysicalUnit, ProfileError,
};
use uuid::Uuid;

/// Absolute agreement required between the fitted and the forward-evaluated achieved value.
const FORWARD_TOLERANCE: f64 = 1e-9;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpticsFamily {
    Focus,
    Zoom,
}

impl OpticsFamily {
    const fn attribute(self) -> &'static str {
        match self {
            Self::Focus => "focus",
            Self::Zoom => "zoom",
        }
    }
}

/// Requested normalized Focus travel. `function_id` pins an exact native function; otherwise
/// the currently active fittable function is kept, or the only fittable function is used.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FocusFitRequest {
    pub normalized: f64,
    pub function_id: Option<Uuid>,
}

/// Requested full Zoom opening in degrees. A `Some` convention must match the profile's
/// calibrated convention; `None` accepts the profile's convention (reported, possibly unknown).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ZoomFitRequest {
    pub degrees: f64,
    pub convention: Option<OpeningConvention>,
    pub function_id: Option<Uuid>,
}

/// One logical head's request. Focus and Zoom are fitted independently; `None` leaves that
/// family's native control untouched.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OpticsFitRequest {
    pub focus: Option<FocusFitRequest>,
    pub zoom: Option<ZoomFitRequest>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum OpticsFitStatus {
    /// No request for this family; nothing is written.
    #[default]
    NotRequested,
    /// A write is proposed. `clipped` tells whether the request was outside the reachable range.
    Fitted,
    /// The head has no control for this family.
    Unsupported,
    /// The requested function identity is not a function of the owning control.
    UnknownFunction,
    /// No usable physical/travel mapping (e.g. Zoom without explicit degree units).
    UnknownPhysicalMapping,
    /// A convention was requested but the profile does not record one.
    UnknownConvention,
    /// A convention was requested and the profile records the other one. No conversion occurs.
    ConventionMismatch,
    /// Several controls, or several fittable functions without a current or pinned one.
    Ambiguous,
    /// The control also carries the other optical family, or heads requested different values
    /// for one master-shared control in the same solve.
    OwnershipConflict,
    /// Non-finite Focus, or Zoom outside the open (0°, 180°) opening domain.
    InvalidRequest,
    /// The forward model disagreed with the fitted value; the write was withheld.
    ForwardMismatch,
}

/// A proposed native write, addressed for [`crate::FixtureModeEncodingPlan::encode_split_by_index`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpticsControlWrite {
    pub channel_index: u32,
    pub channel_id: Uuid,
    pub split: u16,
    /// Final native value within the selected function's `dmx_from..=dmx_to`.
    pub raw: u32,
}

/// The one native control that owns a family on a head: the complete native footprint of that
/// family for a destination adapter. `shared` means it belongs to a master-shared head.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpticsFitControl {
    pub channel_index: u32,
    pub channel_id: Uuid,
    pub split: u16,
    pub shared: bool,
    pub raw_max: u32,
}

/// Outcome for one family of one head.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OpticsFit {
    pub status: OpticsFitStatus,
    /// Requested value exactly as supplied (normalized Focus or Zoom degrees), kept when clipped.
    pub requested: Option<f64>,
    /// Forward-verified achieved value in the same domain as `requested`.
    pub achieved: Option<f64>,
    pub clipped: bool,
    pub function_id: Option<Uuid>,
    pub quality: Option<PhysicalDataQuality>,
    /// Focus only: native travel without a measured focus curve.
    pub nominal: bool,
    /// Zoom only: the profile's convention for `achieved`.
    pub convention: Option<OpeningConvention>,
    /// The owning control belongs to a master-shared head; writing it affects every inheriting head.
    pub shared: bool,
    pub write: Option<OpticsControlWrite>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OpticsFitResult {
    pub head_id: Uuid,
    pub focus: OpticsFit,
    pub zoom: OpticsFit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpticsFitInputError {
    ChannelCount,
    RawOutOfRange,
    RequestLayout,
    OutputLayout,
    WorkspaceLayout,
}

#[derive(Clone, Copy, Debug, Default)]
struct Claim {
    raw: Option<u32>,
    conflict: bool,
}

/// Caller-owned scratch reused across solves; created by [`CompiledOpticsFitting::create_workspace`].
#[derive(Clone, Debug)]
pub struct OpticsFitWorkspace {
    raw: Vec<u32>,
    forward: Vec<OpticsForwardResult>,
    claims: Vec<Claim>,
}

impl OpticsFitWorkspace {
    /// The current native values with the accepted writes of the last solve applied.
    /// Unrelated channels and unrequested families keep their current values.
    pub fn proposed_raw(&self) -> &[u32] {
        &self.raw
    }

    /// Forward evaluation of [`Self::proposed_raw`] from the last solve.
    pub fn forward(&self) -> &[OpticsForwardResult] {
        &self.forward
    }
}

#[derive(Clone, Copy, Debug)]
enum FitKind {
    /// Authored Percent/Normalized mapping; `percent_factor` converts physical to percent.
    FocusMapped {
        percent_factor: f64,
    },
    /// Nominal native travel across the function, as reported by the forward model.
    FocusNominal,
    ZoomDegrees,
    Unfittable,
}

#[derive(Clone, Debug)]
struct FitFunction {
    id: Uuid,
    from: u32,
    to: u32,
    kind: FitKind,
    mapping: Option<CompiledPhysicalMapping>,
}

impl FitFunction {
    fn fittable(&self) -> bool {
        !matches!(self.kind, FitKind::Unfittable)
    }
}

#[derive(Clone, Debug)]
struct FitControl {
    write: OpticsControlWrite,
    shared: bool,
    functions: Box<[FitFunction]>,
    /// Fallback when no function is pinned and the current value is not in a fittable function.
    fallback: Result<usize, OpticsFitStatus>,
}

#[derive(Clone, Debug)]
enum Binding {
    Control(FitControl),
    Unavailable(OpticsFitStatus),
}

#[derive(Clone, Debug)]
struct FitHead {
    id: Uuid,
    focus: Binding,
    zoom: Binding,
}

/// Precompiled Focus/Zoom destination fitting for one fixture mode.
#[derive(Clone, Debug)]
pub struct CompiledOpticsFitting {
    forward: CompiledOpticsForward,
    maxima: Box<[u32]>,
    heads: Box<[FitHead]>,
}

fn carries(channel: &FixtureChannel, family: OpticsFamily) -> bool {
    channel
        .functions
        .iter()
        .any(|f| f.attribute.0.as_ref() == family.attribute())
}

fn fit_kind(
    family: OpticsFamily,
    continuous: bool,
    mapping: Option<&CompiledPhysicalMapping>,
) -> FitKind {
    match (family, continuous, mapping.map(|m| &m.unit)) {
        (_, false, _) => FitKind::Unfittable,
        (OpticsFamily::Focus, true, Some(PhysicalUnit::Percent)) => {
            FitKind::FocusMapped { percent_factor: 1. }
        }
        (OpticsFamily::Focus, true, Some(PhysicalUnit::Normalized)) => FitKind::FocusMapped {
            percent_factor: 100.,
        },
        (OpticsFamily::Focus, true, _) => FitKind::FocusNominal,
        (OpticsFamily::Zoom, true, Some(PhysicalUnit::Degrees)) => FitKind::ZoomDegrees,
        (OpticsFamily::Zoom, true, _) => FitKind::Unfittable,
    }
}

fn compile_functions(
    channel: &FixtureChannel,
    family: OpticsFamily,
) -> Result<Box<[FitFunction]>, ProfileError> {
    channel
        .functions
        .iter()
        .filter(|f| f.attribute.0.as_ref() == family.attribute())
        .map(|f| {
            // Same policy as the forward model: authored calibration failures are errors, legacy
            // equal/unknown endpoints may still describe nominal Focus travel.
            let mapping = match CompiledPhysicalMapping::compile(channel, f) {
                Ok(value) => value,
                Err(error) if f.physical_mapping.is_some() => return Err(error),
                Err(_) => None,
            };
            let continuous = matches!(f.behavior, ChannelFunctionBehavior::Continuous { .. })
                && f.dmx_from < f.dmx_to;
            Ok(FitFunction {
                id: f.id,
                from: f.dmx_from,
                to: f.dmx_to,
                kind: fit_kind(family, continuous, mapping.as_ref()),
                mapping,
            })
        })
        .collect()
}

fn compile_binding(
    mode: &FixtureMode,
    head: Uuid,
    family: OpticsFamily,
) -> Result<Binding, ProfileError> {
    let own = mode
        .channels
        .iter()
        .any(|c| c.head_id == head && carries(c, family));
    let shared_head = |id: Uuid| mode.heads.iter().any(|h| h.id == id && h.master_shared);
    let mut owners = mode.channels.iter().enumerate().filter(|(_, c)| {
        (c.head_id == head || (!own && shared_head(c.head_id))) && carries(c, family)
    });
    let Some((index, channel)) = owners.next() else {
        return Ok(Binding::Unavailable(OpticsFitStatus::Unsupported));
    };
    if owners.next().is_some() {
        return Ok(Binding::Unavailable(OpticsFitStatus::Ambiguous));
    }
    let other = match family {
        OpticsFamily::Focus => OpticsFamily::Zoom,
        OpticsFamily::Zoom => OpticsFamily::Focus,
    };
    if carries(channel, other) {
        // Writing either family would move the other: they cannot stay independent.
        return Ok(Binding::Unavailable(OpticsFitStatus::OwnershipConflict));
    }
    let functions = compile_functions(channel, family)?;
    let mut fittable = functions.iter().enumerate().filter(|(_, f)| f.fittable());
    let fallback = match (fittable.next(), fittable.next()) {
        (None, _) => Err(OpticsFitStatus::UnknownPhysicalMapping),
        (Some((sole, _)), None) => Ok(sole),
        (Some(_), Some(_)) => Err(OpticsFitStatus::Ambiguous),
    };
    Ok(Binding::Control(FitControl {
        write: OpticsControlWrite {
            channel_index: u32::try_from(index)
                .map_err(|_| ProfileError::Invalid("optics fitting: channel index".into()))?,
            channel_id: channel.id,
            split: channel.split,
            raw: 0,
        },
        shared: channel.head_id != head,
        functions,
        fallback,
    }))
}

impl FitControl {
    fn select(&self, current: u32, pinned: Option<Uuid>) -> Result<&FitFunction, OpticsFitStatus> {
        if let Some(id) = pinned {
            let function = self
                .functions
                .iter()
                .find(|f| f.id == id)
                .ok_or(OpticsFitStatus::UnknownFunction)?;
            return if function.fittable() {
                Ok(function)
            } else {
                Err(OpticsFitStatus::UnknownPhysicalMapping)
            };
        }
        if let Some(active) = self
            .functions
            .iter()
            .find(|f| f.fittable() && (f.from..=f.to).contains(&current))
        {
            return Ok(active);
        }
        self.fallback.map(|index| &self.functions[index])
    }
}

struct Solved {
    raw: u32,
    achieved: f64,
    clipped: bool,
    quality: PhysicalDataQuality,
    nominal: bool,
    convention: Option<OpeningConvention>,
}

fn solve_focus(function: &FitFunction, requested: f64) -> Result<Solved, OpticsFitStatus> {
    if !requested.is_finite() {
        return Err(OpticsFitStatus::InvalidRequest);
    }
    let target = requested.clamp(0., 1.);
    let domain_clipped = target != requested;
    match (function.kind, function.mapping.as_ref()) {
        (FitKind::FocusMapped { percent_factor }, Some(mapping)) => {
            let result = mapping
                .raw_for_physical(100. * target / percent_factor)
                .map_err(|_| OpticsFitStatus::InvalidRequest)?;
            let percent = result.physical * percent_factor;
            if !(0. ..=100.).contains(&percent) {
                return Err(OpticsFitStatus::UnknownPhysicalMapping);
            }
            Ok(Solved {
                raw: result.raw,
                achieved: percent / 100.,
                clipped: domain_clipped || result.clipped,
                quality: mapping.quality,
                nominal: false,
                convention: None,
            })
        }
        (FitKind::FocusNominal, _) => {
            let span = f64::from(function.to - function.from);
            let offset = (target * span).round() as u32;
            Ok(Solved {
                raw: function.from + offset,
                achieved: f64::from(offset) / span,
                clipped: domain_clipped,
                quality: PhysicalDataQuality::Estimated,
                nominal: true,
                convention: None,
            })
        }
        _ => Err(OpticsFitStatus::UnknownPhysicalMapping),
    }
}

fn solve_zoom(function: &FitFunction, request: &ZoomFitRequest) -> Result<Solved, OpticsFitStatus> {
    if !request.degrees.is_finite() || request.degrees <= 0. || request.degrees >= 180. {
        return Err(OpticsFitStatus::InvalidRequest);
    }
    let (FitKind::ZoomDegrees, Some(mapping)) = (function.kind, function.mapping.as_ref()) else {
        return Err(OpticsFitStatus::UnknownPhysicalMapping);
    };
    match (request.convention, mapping.opening_convention) {
        (Some(_), None) => return Err(OpticsFitStatus::UnknownConvention),
        (Some(wanted), Some(actual)) if wanted != actual => {
            return Err(OpticsFitStatus::ConventionMismatch);
        }
        _ => {}
    }
    let result = mapping
        .raw_for_physical(request.degrees)
        .map_err(|_| OpticsFitStatus::InvalidRequest)?;
    if result.physical <= 0. || result.physical >= 180. {
        return Err(OpticsFitStatus::UnknownPhysicalMapping);
    }
    Ok(Solved {
        raw: result.raw,
        achieved: result.physical,
        clipped: result.clipped,
        quality: mapping.quality,
        nominal: false,
        convention: mapping.opening_convention,
    })
}

fn fit_family(
    binding: &Binding,
    current: &[u32],
    requested: f64,
    pinned: Option<Uuid>,
    solve: impl FnOnce(&FitFunction) -> Result<Solved, OpticsFitStatus>,
) -> OpticsFit {
    let mut fit = OpticsFit {
        requested: Some(requested),
        ..OpticsFit::default()
    };
    let control = match binding {
        Binding::Unavailable(status) => {
            fit.status = *status;
            return fit;
        }
        Binding::Control(control) => control,
    };
    fit.shared = control.shared;
    let current = current[control.write.channel_index as usize];
    let solved = control.select(current, pinned).and_then(|function| {
        fit.function_id = Some(function.id);
        solve(function)
    });
    match solved {
        Err(status) => fit.status = status,
        Ok(solved) => {
            fit.status = OpticsFitStatus::Fitted;
            fit.achieved = Some(solved.achieved);
            fit.clipped = solved.clipped;
            fit.quality = Some(solved.quality);
            fit.nominal = solved.nominal;
            fit.convention = solved.convention;
            fit.write = Some(OpticsControlWrite {
                raw: solved.raw,
                ..control.write
            });
        }
    }
    fit
}

fn claim(claims: &mut [Claim], fit: &OpticsFit) {
    if let Some(write) = fit.write {
        let claim = &mut claims[write.channel_index as usize];
        match claim.raw {
            None => claim.raw = Some(write.raw),
            Some(raw) if raw != write.raw => claim.conflict = true,
            Some(_) => {}
        }
    }
}

fn withhold(fit: &mut OpticsFit, status: OpticsFitStatus) {
    fit.status = status;
    fit.achieved = None;
    fit.clipped = false;
    fit.write = None;
}

fn resolve_conflict(claims: &[Claim], raw: &mut [u32], fit: &mut OpticsFit) {
    if let Some(write) = fit.write {
        if claims[write.channel_index as usize].conflict {
            withhold(fit, OpticsFitStatus::OwnershipConflict);
        } else {
            raw[write.channel_index as usize] = write.raw;
        }
    }
}

fn agrees(
    fit: &OpticsFit,
    status: OpticsForwardStatus,
    forward: Option<(f64, Uuid, PhysicalDataQuality)>,
) -> bool {
    match (status, forward, fit.achieved) {
        (OpticsForwardStatus::Resolved, Some((value, id, quality)), Some(achieved)) => {
            Some(id) == fit.function_id
                && Some(quality) == fit.quality
                && (value - achieved).abs() <= FORWARD_TOLERANCE * achieved.abs().max(1.)
        }
        _ => false,
    }
}

impl CompiledOpticsFitting {
    pub fn compile(mode: &FixtureMode) -> Result<Self, ProfileError> {
        // Validates capacity, native function domains and every authored calibration.
        let forward = CompiledOpticsForward::compile(mode)?;
        let heads = mode
            .heads
            .iter()
            .map(|h| {
                Ok(FitHead {
                    id: h.id,
                    focus: compile_binding(mode, h.id, OpticsFamily::Focus)?,
                    zoom: compile_binding(mode, h.id, OpticsFamily::Zoom)?,
                })
            })
            .collect::<Result<_, ProfileError>>()?;
        Ok(Self {
            forward,
            maxima: mode
                .channels
                .iter()
                .map(|c| c.resolution.max_raw())
                .collect(),
            heads,
        })
    }

    /// The forward model used to verify every proposed write.
    pub fn forward(&self) -> &CompiledOpticsForward {
        &self.forward
    }

    pub fn head_count(&self) -> usize {
        self.heads.len()
    }

    /// Position of a mode head in this fitting (mode order).
    pub fn head_index(&self, head_id: Uuid) -> Option<usize> {
        self.heads.iter().position(|h| h.id == head_id)
    }

    /// The control owning `family` on `head`, or the compile-time reason it has none
    /// (`Unsupported`, `Ambiguous`, `OwnershipConflict`). `None` for an unknown head.
    /// A bound control can still report a function-level status when fitted.
    pub fn control(
        &self,
        head: usize,
        family: OpticsFamily,
    ) -> Option<Result<OpticsFitControl, OpticsFitStatus>> {
        let head = self.heads.get(head)?;
        let binding = match family {
            OpticsFamily::Focus => &head.focus,
            OpticsFamily::Zoom => &head.zoom,
        };
        Some(match binding {
            Binding::Unavailable(status) => Err(*status),
            Binding::Control(control) => Ok(OpticsFitControl {
                channel_index: control.write.channel_index,
                channel_id: control.write.channel_id,
                split: control.write.split,
                shared: control.shared,
                raw_max: self.maxima[control.write.channel_index as usize],
            }),
        })
    }

    pub fn create_workspace(&self) -> OpticsFitWorkspace {
        OpticsFitWorkspace {
            raw: vec![0; self.maxima.len()],
            forward: self.forward.create_output(),
            claims: vec![Claim::default(); self.maxima.len()],
        }
    }

    pub fn create_output(&self) -> Vec<OpticsFitResult> {
        self.heads
            .iter()
            .map(|h| OpticsFitResult {
                head_id: h.id,
                ..OpticsFitResult::default()
            })
            .collect()
    }

    fn validate(
        &self,
        current: &[u32],
        requests: &[OpticsFitRequest],
        workspace: &OpticsFitWorkspace,
        output: &[OpticsFitResult],
    ) -> Result<(), OpticsFitInputError> {
        if current.len() != self.maxima.len() {
            return Err(OpticsFitInputError::ChannelCount);
        }
        if current.iter().zip(&self.maxima).any(|(v, max)| v > max) {
            return Err(OpticsFitInputError::RawOutOfRange);
        }
        if requests.len() != self.heads.len() {
            return Err(OpticsFitInputError::RequestLayout);
        }
        if output.len() != self.heads.len()
            || output
                .iter()
                .zip(&self.heads)
                .any(|(o, h)| o.head_id != h.id)
        {
            return Err(OpticsFitInputError::OutputLayout);
        }
        if workspace.raw.len() != self.maxima.len()
            || workspace.claims.len() != self.maxima.len()
            || workspace.forward.len() != self.heads.len()
        {
            return Err(OpticsFitInputError::WorkspaceLayout);
        }
        Ok(())
    }

    /// Fits every head's independent Focus/Zoom request against the current native values.
    ///
    /// Only the owning control of a requested family receives a proposed write. Heads that ask
    /// different values of one master-shared control in the same solve get `OwnershipConflict`.
    /// Accepted writes are applied to the workspace and re-evaluated through the forward model.
    pub fn fit(
        &self,
        current: &[u32],
        requests: &[OpticsFitRequest],
        workspace: &mut OpticsFitWorkspace,
        output: &mut [OpticsFitResult],
    ) -> Result<(), OpticsFitInputError> {
        self.validate(current, requests, workspace, output)?;
        workspace.raw.copy_from_slice(current);
        workspace.claims.fill(Claim::default());
        for ((head, request), out) in self.heads.iter().zip(requests).zip(output.iter_mut()) {
            out.focus = request.focus.map_or_else(OpticsFit::default, |r| {
                fit_family(&head.focus, current, r.normalized, r.function_id, |f| {
                    solve_focus(f, r.normalized)
                })
            });
            out.zoom = request.zoom.map_or_else(OpticsFit::default, |r| {
                fit_family(&head.zoom, current, r.degrees, r.function_id, |f| {
                    solve_zoom(f, &r)
                })
            });
            claim(&mut workspace.claims, &out.focus);
            claim(&mut workspace.claims, &out.zoom);
        }
        for out in output.iter_mut() {
            resolve_conflict(&workspace.claims, &mut workspace.raw, &mut out.focus);
            resolve_conflict(&workspace.claims, &mut workspace.raw, &mut out.zoom);
        }
        self.forward
            .evaluate(&workspace.raw, &mut workspace.forward)
            .map_err(|_| OpticsFitInputError::RawOutOfRange)?;
        for (out, forward) in output.iter_mut().zip(&workspace.forward) {
            let focus = forward
                .focus
                .map(|f| (f.percent / 100., f.function_id, f.quality));
            let zoom = forward.zoom.map(|z| (z.degrees, z.function_id, z.quality));
            for (fit, status, value) in [
                (&mut out.focus, forward.focus_status, focus),
                (&mut out.zoom, forward.zoom_status, zoom),
            ] {
                if fit.write.is_some() && !agrees(fit, status, value) {
                    // Never report an achieved value the native output would not produce.
                    let index = fit.write.map_or(0, |w| w.channel_index as usize);
                    workspace.raw[index] = current[index];
                    withhold(fit, OpticsFitStatus::ForwardMismatch);
                }
            }
        }
        Ok(())
    }
}
