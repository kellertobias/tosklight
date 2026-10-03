//! TL-592 Color/UV physical adapter: the existing compiled destination fitter
//! (`CompiledColorFitting`, TL-568/TL-573) wired into the TL-590 captured-frame adapter seam.
//!
//! Per target and frame:
//! 1. The destination descriptor is compiled once per runtime generation by the lane. It holds
//!    one entry per Color head *per physical destination*: every Color head the target owns
//!    (a root fixture can own several) on the root instance and on every multipatch copy
//!    (TL-557). Each destination is fitted with its own compiled fitter, built from that
//!    instance's installed color calibration and installed appearance; fitters are shared
//!    through `Arc` and cached per `(fixture, instance)` against the snapshot's fixture list,
//!    so a patch, mode, calibration or appearance replacement (always a new list) recompiles.
//! 2. `current` is the root's pre-master native raw vector, read once per frame from the
//!    scalar-resolved baseline (`HybridFrameContext::native_raw_into`, same token). Every
//!    destination starts from it (copies duplicate the root's resolved channels), overlaid with
//!    this lane's last accepted writes *of that destination and head* only.
//! 3. The complete composed `ColorIntent` (recipe/XYZ, relativeOutput, White Blend/CCT/Duv, UV,
//!    allocation, wheel constraints) goes to `CompiledColorFitting::fit` unchanged for every
//!    head. UV is frozen by the fitter before visible fitting; known leakage is part of the
//!    fitted total.
//! 4. Every footprint control is written once: fitted controls from the fitter, and every
//!    control the fitter retains (unmodeled, unknown appearance, candidate limit) parked at its
//!    neutral raw *before* the final fit, so the published forward prediction describes the
//!    exact written output. Zero UV is an explicit parked write. A master-shared slot that
//!    several heads of one destination fit belongs to the first head; a later head that wanted
//!    another value is re-evaluated forward against the written value and reports it.
//! 5. The sidecar publishes the unchanged request, the forward-evaluated achieved output and
//!    compact quality/work counters of the primary head (root instance, first Color head) plus
//!    the complete per-destination/head breakdown. No consumer refits; no gamut diagnostics.
//!
//! Intensity, sequence/group/grand masters and blackout are not read or applied here: `current`
//! is pre-master and the writes are pre-master drives, so the final render attenuates once.
//!
//! Direct programs (TL-559, `direct.rs`) take the same per-head path: each destination head is
//! planned by TL-595 identity only (`plan_direct_replay`). Compatible heads replay the exact
//! native recipe; incompatible or unverified heads fit the source estimate, forward-evaluated
//! by the ORIGINAL model for the exact recipe of this frame, through the fitter above (visible
//! Fit/Hold and UV Apply/ParkOff are independent). Every footprint control is still written once.
//! Native descriptors, the reference head, page overflow and first-edit adoption live in
//! `native.rs`.
use super::*;
use light_core::NativeColorIdentity;
use light_core::Xyz;
use light_core::programming::{
    ColorIntent, ColorProgram, DirectCompatibility, NativeDriveLimit, PortableColorEstimate,
    UvFallback, VisibleFallback,
};
use light_engine::{CapturedNativeRaw, profile_head_destinations};
use light_fixture::PhysicalDataQuality;
use light_fixture::forward::{
    ColorConstraintFit, ColorFitControl, ColorFitLimitations, ColorFitResult, ColorFitWork,
    ColorFitWorkspace, ColorForwardResult, ColorMatch, ColorRetainReason, CompiledColorFitting,
    CompiledColorForward, UvFitStatus, VisibleFitStatus,
};
use light_fixture::{MultiPatchInstance, PatchedFixture};
use parking_lot::Mutex;
use std::cell::{Cell, RefCell};
use uuid::Uuid;

mod direct;
pub(in crate::runtime) mod native;
pub(in crate::runtime) mod native_seed;
mod resolve;
mod transition;

#[cfg(test)]
pub(in crate::runtime) mod profiles;
#[cfg(test)]
pub(in crate::runtime) mod tests;

/// One Color head on one physical destination (root instance or multipatch copy).
pub(in crate::runtime) struct ColorHeadDescriptor {
    /// Fitter compiled with this destination's own installed calibration and appearance.
    pub fitting: Arc<CompiledColorFitting>,
    /// Root fixture id, or the copy's stable multipatch id.
    pub destination: FixtureId,
    /// Head position inside `fitting`.
    pub head: usize,
    pub head_id: Uuid,
    /// Complete Color-owned controls of the head, in fitting order.
    pub controls: Box<[ColorFitControl]>,
    /// Per control: this head writes it. False only for a master-shared slot an earlier head of
    /// the same destination already owns.
    pub writes_control: Box<[bool]>,
    /// TL-595 identity of this destination head, from the complete profile's runtime Color
    /// context (never re-derived from names, slots or a compacted runtime profile). None: the
    /// destination is unverified and Direct values always fall back.
    pub native: Option<NativeColorIdentity>,
    /// Authoritative native controls of the head's optical path, in path order (TL-554 pages).
    pub native_controls: Box<[native::NativeColorControl]>,
    scratch: Mutex<ColorHeadScratch>,
}

struct ColorHeadScratch {
    workspace: ColorFitWorkspace,
    output: ColorFitResult,
    /// Forward re-evaluation, used only after a shared-slot conflict.
    forward: Vec<ColorForwardResult>,
}

/// Compiled Color destinations of one programming target. Scratch is lane-local (Mutex: Send).
pub(in crate::runtime) struct ColorDescriptor {
    /// Root patched fixture whose mode channel space the native raw values index.
    pub root: FixtureId,
    /// Root-instance heads first (mode order), then each multipatch copy in patch order.
    pub heads: Box<[ColorHeadDescriptor]>,
    /// Every written slot of every head, exactly once, in head order.
    pub footprint: Box<[NativeControlSlot]>,
    scratch: Mutex<ColorScratch>,
}

struct ColorScratch {
    native: CapturedNativeRaw,
    current: Vec<u32>,
    /// Last forward evaluation of a Direct recipe by its original model, keyed by the exact
    /// immutable program and model objects (weak: never keeps a retired value alive).
    direct: Option<direct::DirectEstimateCache>,
}

impl ColorDescriptor {
    /// The primary head: root instance, first Color head in mode order.
    pub fn primary(&self) -> &ColorHeadDescriptor {
        &self.heads[0]
    }
}

/// This lane's last accepted Color writes of one destination head.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::runtime) struct ColorHeadContinuity {
    pub destination: FixtureId,
    pub head_id: Uuid,
    /// `(channel_index, channel_id, raw)` of every control this head wrote.
    pub controls: Vec<(u32, Uuid, u32)>,
}

/// Last accepted writes of every destination head, the discrete-state preference.
#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::runtime) struct ColorContinuity {
    pub heads: Vec<ColorHeadContinuity>,
}

impl ColorContinuity {
    pub fn head(&self, destination: FixtureId, head_id: Uuid) -> Option<&ColorHeadContinuity> {
        self.heads
            .iter()
            .find(|h| h.destination == destination && h.head_id == head_id)
    }
}

/// Forward-evaluated output of the written native values (never a requested value).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::runtime) struct AchievedColor {
    /// Complete visible XYZ, only when every active contribution is modeled.
    pub visible: Option<Xyz>,
    /// Modeled contributions only; incomplete does not mean black.
    pub known_xyz: Xyz,
    /// Common normalized UV drive; None for unequal banks or no UV.
    pub uv_drive: Option<f64>,
}

/// Work of one head's resolve. Counters only; no budget is implied.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::runtime) struct ColorSolveWork {
    /// Fitter calls (2 when a retained control had to be parked and the head was refitted).
    pub fits: u32,
    pub candidates_ranked: u32,
    pub fit: ColorFitWork,
}

/// The composed Color request, unchanged. Semantic values keep the TL-557 `ColorIntent`
/// comparison; Direct values keep the exact tagged program (recipe plus recorded estimate).
#[derive(Clone, Debug, PartialEq)]
pub(in crate::runtime) enum ColorRequest {
    Semantic(ColorIntent),
    Direct(Arc<ColorProgram>),
}

impl ColorRequest {
    pub fn semantic(&self) -> Option<&ColorIntent> {
        match self {
            Self::Semantic(intent) => Some(intent),
            Self::Direct(_) => None,
        }
    }
}

impl PartialEq<ColorIntent> for ColorRequest {
    fn eq(&self, other: &ColorIntent) -> bool {
        self.semantic() == Some(other)
    }
}

/// How one destination head resolved a Direct value.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::runtime) enum DirectReplayOutcome {
    /// Verified compatible native layout: the recorded controls were written unchanged.
    Exact,
    /// The source estimate was fitted through the destination resolver.
    Fallback {
        compatibility: DirectCompatibility,
        visible: VisibleFallback,
        uv: UvFallback,
    },
}

/// Where the source estimate used by a Direct head came from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum DirectEstimateOrigin {
    /// Forward-evaluated this frame by the pinned ORIGINAL model for this exact recipe.
    Forward,
    /// The original model is unavailable: the recorded estimate is used as valid fallback data.
    Recorded,
}

/// Passive Direct status of one destination head.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::runtime) struct DirectColorStatus {
    pub replay: DirectReplayOutcome,
    pub origin: DirectEstimateOrigin,
    /// The source estimate this frame used (fallback input; exact heads report it passively).
    pub estimate: PortableColorEstimate,
    pub drive_limit: NativeDriveLimit,
    /// Recorded plus replay limitations (unknown components, unavailable originals).
    pub limitations: Vec<String>,
}

/// Compact passive status of one encoded result.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::runtime) struct ColorQuality {
    pub visible: VisibleFitStatus,
    pub color_match: ColorMatch,
    pub delta_uv: Option<f64>,
    pub luminance_ratio: Option<f64>,
    pub luminance_limited: bool,
    pub data_quality: PhysicalDataQuality,
    /// Nominal/estimated data, never presented as a measured match.
    pub nominal: bool,
    /// The head has no continuously driven visible emitter: it shows colour only through
    /// discrete wheel slots, so an inexact match is wheel-limited.
    pub discrete: bool,
    /// Unknown whenever any active contribution (for example unknown UV leakage) is unknown.
    pub total_quality: PhysicalDataQuality,
    pub uv: UvFitStatus,
    pub uv_clipped: bool,
    pub uv_appearance_known: bool,
    pub limitations: ColorFitLimitations,
    pub constraints: Vec<ColorConstraintFit>,
    /// Controls the fitter retained, written at their neutral raw, with the fitter's reason.
    pub parked: Vec<(Uuid, ColorRetainReason)>,
    /// A master-shared slot kept an earlier head's value; achieved was re-evaluated forward and
    /// the fitter's match figures describe the unshared proposal only.
    pub shared_conflict: bool,
    pub work: ColorSolveWork,
    /// Direct values only (TL-559): exact replay or fallback, and the estimate it used.
    pub direct: Option<DirectColorStatus>,
    /// Top-level entry only: every destination head in descriptor order (the first equals the
    /// top-level fields). Nested entries leave this empty.
    pub heads: Vec<ColorHeadOutcome>,
}

/// Achieved output and quality of one destination head.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::runtime) struct ColorHeadOutcome {
    pub destination: FixtureId,
    pub head_id: Uuid,
    pub achieved: AchievedColor,
    pub quality: ColorQuality,
}

/// Cumulative adapter work since creation. Counters only; no budget is implied.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::runtime) struct ColorAdapterCounters {
    pub descriptor_compiles: u64,
    pub fitting_compiles: u64,
    pub fitting_cache_hits: u64,
    /// Profiles whose Color model failed to compile (passive: the head keeps the scalar path).
    pub fitting_failures: u64,
    /// Descriptors spanning more than one destination head (several heads and/or copies).
    pub multi_head_targets: u64,
    /// Multipatch copy destinations compiled with their own calibration.
    pub copy_destinations: u64,
    pub resolves: u64,
    pub fits: u64,
    pub refits: u64,
    /// Heads whose master-shared slot kept an earlier head's value.
    pub shared_conflicts: u64,
    /// Direct heads written with their exact recorded native controls.
    pub direct_exact: u64,
    /// Direct heads fitted from the source estimate.
    pub direct_fallbacks: u64,
    /// Direct heads whose visible solution was held (unknown appearance).
    pub direct_visible_holds: u64,
    /// Original-model forward evaluations of Direct recipes (one per changed recipe object).
    pub direct_forward_evaluations: u64,
    /// Interior Semantic↔Direct transition samples blended through portable appearance.
    pub representation_transitions: u64,
    /// Interior transition samples held because one endpoint's appearance is unknown.
    pub representation_holds: u64,
    /// Current adoptions into another Color representation for one composition.
    pub representation_adoptions: u64,
    pub candidates_ranked: u64,
    pub visible_solves: u64,
    pub fixed_offset_solves: u64,
    pub level_solves: u64,
    pub forward_evaluations: u64,
}

type FittingCache = (
    Arc<Vec<PatchedFixture>>,
    FxHashMap<(FixtureId, Uuid), Option<Arc<CompiledColorFitting>>>,
);

#[derive(Default)]
pub(in crate::runtime) struct ColorAdapter {
    fittings: RefCell<Option<FittingCache>>,
    counters: Cell<ColorAdapterCounters>,
}

pub(super) fn invalid(message: impl Into<String>) -> TransitionError {
    IntentError(message.into()).into()
}

pub(super) fn semantic_intent(value: &AttributeValue) -> Result<&ColorIntent, TransitionError> {
    match value {
        AttributeValue::ColorProgram(program) => match program.as_ref() {
            ColorProgram::Semantic { intent } => Ok(intent),
            // The Color adapter resolves Direct through `direct.rs`; semantic-only consumers
            // (Media White Blend) keep Direct a passive requirement.
            _ => Err(TransitionError::Requires(
                TransitionRequirement::ColorAppearance,
            )),
        },
        _ => Err(TransitionError::Requires(
            TransitionRequirement::ColorAppearance,
        )),
    }
}

/// Same forward model as the engine's physical projection of this instance: the root uses the
/// fixture's calibration/appearance, a copy its own (`physical_projection.rs`).
fn compile_fitting(
    fixture: &PatchedFixture,
    copy: Option<&MultiPatchInstance>,
) -> Result<Option<CompiledColorFitting>, light_fixture::ProfileError> {
    let (Some(profile), Some(mode)) = (
        fixture.definition.profile_snapshot.as_deref(),
        fixture.definition.mode_id,
    ) else {
        return Ok(None);
    };
    let (calibration, appearance) = match copy {
        Some(copy) => (copy.color_calibration.as_ref(), &copy.installed_appearance),
        None => (
            fixture.color_calibration.as_ref(),
            &fixture.installed_appearance,
        ),
    };
    let Some(forward) = CompiledColorForward::compile_with_context(
        profile,
        mode,
        calibration,
        fixture.definition.runtime_color_context.as_deref(),
    )?
    else {
        return Ok(None);
    };
    let forward = forward.with_installed_appearance(appearance);
    CompiledColorFitting::from_forward(profile, mode, forward).map(Some)
}

impl ColorAdapter {
    pub fn counters(&self) -> ColorAdapterCounters {
        self.counters.get()
    }

    fn count(&self, update: impl FnOnce(&mut ColorAdapterCounters)) {
        let mut counters = self.counters.get();
        update(&mut counters);
        self.counters.set(counters);
    }

    /// One shared fitter per patched instance of this exact fixture list.
    fn fitting(
        &self,
        snapshot: &EngineSnapshot,
        fixture: &PatchedFixture,
        copy: Option<&MultiPatchInstance>,
    ) -> Option<Arc<CompiledColorFitting>> {
        let mut cache = self.fittings.borrow_mut();
        if !cache
            .as_ref()
            .is_some_and(|(fixtures, _)| Arc::ptr_eq(fixtures, &snapshot.fixtures))
        {
            *cache = Some((Arc::clone(&snapshot.fixtures), FxHashMap::default()));
        }
        let entries = &mut cache.as_mut().expect("cache installed").1;
        let key = (
            fixture.fixture_id,
            copy.map_or(fixture.fixture_id.0, |c| c.id),
        );
        if let Some(hit) = entries.get(&key) {
            self.count(|c| c.fitting_cache_hits += 1);
            return hit.clone();
        }
        let compiled = match compile_fitting(fixture, copy) {
            Ok(compiled) => compiled.map(Arc::new),
            Err(_) => {
                self.count(|c| c.fitting_failures += 1);
                None
            }
        };
        self.count(|c| c.fitting_compiles += 1);
        entries.insert(key, compiled.clone());
        compiled
    }

    /// One head descriptor; `claimed` holds the slots earlier heads of this destination write.
    fn head_descriptor(
        fixture: &PatchedFixture,
        fitting: Arc<CompiledColorFitting>,
        index: usize,
        head_id: Uuid,
        destination: FixtureId,
        claimed: &mut Vec<NativeControlSlot>,
    ) -> Result<ColorHeadDescriptor, TransitionError> {
        let disappeared = || invalid("Color fitter head disappeared");
        let controls: Box<[ColorFitControl]> =
            fitting.controls(index).ok_or_else(disappeared)?.collect();
        let writes_control = controls
            .iter()
            .map(|control| {
                let slot = NativeControlSlot {
                    destination,
                    channel_index: control.channel_index,
                    split: control.split,
                };
                let owned = !claimed.contains(&slot);
                if owned {
                    claimed.push(slot);
                }
                owned
            })
            .collect();
        let scratch = ColorHeadScratch {
            workspace: fitting.create_workspace(),
            output: fitting.create_output(index).ok_or_else(disappeared)?,
            forward: Vec::new(),
        };
        let (native, native_controls) = native::head_native(fixture, head_id, &controls);
        Ok(ColorHeadDescriptor {
            destination,
            head: index,
            head_id,
            controls,
            writes_control,
            native,
            native_controls,
            scratch: Mutex::new(scratch),
            fitting,
        })
    }
}

impl PhysicalFamilyAdapter for ColorAdapter {
    type Descriptor = ColorDescriptor;
    type Continuity = ColorContinuity;
    type Requested = ColorRequest;
    type Achieved = AchievedColor;
    type Quality = ColorQuality;

    fn owns(&self, owner: ProgrammingOwner) -> bool {
        owner == ProgrammingOwner::Color
    }

    fn compile(
        &self,
        snapshot: &EngineSnapshot,
        target: FixtureId,
    ) -> Result<Option<ColorDescriptor>, TransitionError> {
        self.count(|c| c.descriptor_compiles += 1);
        let destinations = profile_head_destinations(snapshot, target);
        let Some(first) = destinations.first() else {
            return Ok(None);
        };
        if destinations
            .iter()
            .any(|d| d.fixture_index != first.fixture_index)
        {
            return Err(invalid("one Color target spans several root fixtures"));
        }
        let fixture = &snapshot.fixtures[first.fixture_index];
        let instances = std::iter::once(None).chain(fixture.multipatch.iter().map(Some));
        let mut heads = Vec::new();
        let mut footprint = Vec::new();
        for copy in instances {
            let destination = copy.map_or(fixture.fixture_id, |c| FixtureId(c.id));
            let mut claimed = Vec::new();
            for head in &destinations {
                // Media White Blend is source desaturation with the tint kept (TL-593): a Media
                // head belongs to `MediaColorAdapter`, never to lamp fitting.
                if super::media_color::has_media_color_identity(fixture, head.head_id) {
                    continue;
                }
                let Some(fitting) = self.fitting(snapshot, fixture, copy) else {
                    // A physical copy may exceed compiled-model capacity independently of the
                    // root. Hold the complete owner; never silently omit a requested instance.
                    return Ok(None);
                };
                let Some(index) = fitting.head_index(head.head_id) else {
                    continue;
                };
                let controls: Vec<_> = fitting.controls(index).into_iter().flatten().collect();
                let owned_heads: Vec<_> = destinations
                    .iter()
                    .filter_map(|d| fitting.head_index(d.head_id))
                    .collect();
                let crosses_target = (0..fitting.head_count()).any(|other| {
                    !owned_heads.contains(&other)
                        && fitting.controls(other).into_iter().flatten().any(|c| {
                            controls
                                .iter()
                                .any(|owned| owned.channel_index == c.channel_index)
                        })
                });
                if crosses_target {
                    // Separate logical-head targets need one captured destination cohort to
                    // arbitrate shared controls. Until TL548 binds that cohort, hold passively
                    // instead of letting duplicate sidecars reject the whole output frame.
                    return Ok(None);
                }
                let start = claimed.len();
                heads.push(Self::head_descriptor(
                    fixture,
                    fitting,
                    index,
                    head.head_id,
                    destination,
                    &mut claimed,
                )?);
                footprint.extend_from_slice(&claimed[start..]);
            }
            if copy.is_some() && !claimed.is_empty() {
                self.count(|c| c.copy_destinations += 1);
            }
        }
        if heads.is_empty() {
            return Ok(None);
        }
        if heads.len() > 1 {
            self.count(|c| c.multi_head_targets += 1);
        }
        Ok(Some(ColorDescriptor {
            root: fixture.fixture_id,
            heads: heads.into_boxed_slice(),
            footprint: footprint.into_boxed_slice(),
            scratch: Mutex::new(ColorScratch {
                native: CapturedNativeRaw::default(),
                current: Vec::new(),
                direct: None,
            }),
        }))
    }

    fn footprint<'d>(&self, descriptor: &'d ColorDescriptor) -> &'d [NativeControlSlot] {
        &descriptor.footprint
    }

    fn resolve(
        &self,
        request: PhysicalRequest<'_, Self>,
    ) -> Result<PhysicalResolution<Self>, TransitionError> {
        self.resolve_heads(request)
    }

    /// Current adoption into another Color representation for one composition (never stored).
    fn adopt_with_continuity(
        &self,
        frame: HybridFrameContext<'_>,
        descriptor: &ColorDescriptor,
        target: FixtureId,
        original: &AttributeValue,
        address: &DynamicValueAddress,
        previous: Option<&ColorContinuity>,
    ) -> Result<AttributeValue, TransitionError> {
        self.adopt_representation(frame, descriptor, target, original, address, previous)
    }

    /// Semantic↔Direct (or Direct of another source) fades within the one Color owner.
    fn transition(
        &self,
        frame: HybridFrameContext<'_>,
        _descriptor: &ColorDescriptor,
        _target: FixtureId,
        requirement: TransitionRequirement,
        from: &AttributeValue,
        to: &AttributeValue,
        operation: FamilyExpressionOperation,
    ) -> Result<(AttributeValue, Option<ProgrammingTransitionTrace>), TransitionError> {
        self.transition_representations(frame, requirement, from, to, operation)
    }
}
