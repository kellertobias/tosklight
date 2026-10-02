//! TL-558 Focus/Zoom physical adapter: the existing compiled destination fitter
//! (`CompiledOpticsFitting`, TL-566) wired into the TL-590 captured-frame adapter seam.
//!
//! Focus and Zoom are independent owners. One [`OpticsAdapter`] serves exactly one family, so
//! its descriptor, footprint (the single owning native control) and continuity never cover the
//! other family. [`OpticsLanes`] pairs a Focus and a Zoom lane behind one `HybridFrameResolver`
//! and observer; both lanes share one fitter cache and counters.
//!
//! Per head and frame:
//! 1. `compile` finds the target's single head whose family has an owning control, shares the
//!    fixture's `CompiledOpticsFitting` through `Arc` (cached against the snapshot fixture
//!    list, so patch/mode replacement recompiles) and records the control as the footprint. A
//!    head without the family is a passive requirement; a control that is ambiguous or also
//!    carries the other family is published with an empty footprint and its status.
//! 2. `current` is the destination's pre-master native raw vector from the frame's scalar
//!    baseline (same token) with this lane's last accepted write overlaid, so the active
//!    function stays stable. The overlay is guarded (TL-601): it applies only while the
//!    captured baseline raw of the control (the authored native command, never telemetry) and
//!    the control's physical-response digest equal the witnesses recorded with that write. A
//!    fresh native edit or a changed response with the same native UUIDs uses the captured
//!    baseline instead of replaying the old raw.
//! 3. Only this family is requested (`None` for the other family = untouched, never zero).
//!    Zoom passes its stored convention: it is checked, never converted.
//! 4. The footprint is always written: a fitted write, or the current raw held and marked
//!    `parked` when the request cannot be fitted (unknown mapping/convention, invalid request,
//!    ambiguous function). The stored request is never rewritten.
//!    It covers the root and every multipatch copy with the same raw (`instances.rs`).
//! 5. `achieved` is the forward evaluation of the exact written raw (None = unknown, not zero).
//! 6. A passive requirement of a successful frame (TL-602, the TL-598 lane contract) keeps that
//!    head/family's last accepted continuity, token and provenance without a sidecar or a
//!    Release; a produced result wins over an incidental requirement, and only a genuine
//!    removal releases and retires continuity.
//!
//! Intensity, masters and blackout are not read or applied: `current` is pre-master.
use super::lane::PhysicalLaneKind;
use super::*;
use light_core::OpeningConvention;
use light_core::programming::{ScalarIntent, ZoomIntent};
use light_dynamics::DynamicFamilyRepresentation;
use light_engine::{CapturedNativeRaw, PreloadBranch, profile_head_destinations};
use light_fixture::{
    CompiledOpticsFitting, FocusFitRequest, OpticsFamily, OpticsFit, OpticsFitControl,
    OpticsFitRequest, OpticsFitResult, OpticsFitStatus, OpticsFitWorkspace, PatchedFixture,
    PhysicalDataQuality, ZoomFitRequest,
    forward::{OpticsForwardResult, OpticsForwardStatus},
};
use std::sync::Mutex;
use uuid::Uuid;

mod instances;
#[cfg(test)]
pub(in crate::runtime) mod profiles;
#[cfg(test)]
pub(in crate::runtime) mod tests;

/// Compiled destination of one family on one head. Scratch is lane-local, behind a Mutex (Send).
pub(in crate::runtime) struct OpticsDescriptor {
    pub fitting: Arc<CompiledOpticsFitting>,
    pub family: OpticsFamily,
    /// Root patched fixture whose mode channels the footprint addresses.
    pub destination: FixtureId,
    /// Head position inside `fitting`.
    pub head: usize,
    pub head_id: Uuid,
    /// The owning control, or why the head cannot be driven (`Ambiguous`, `OwnershipConflict`).
    pub control: Result<OpticsFitControl, OpticsFitStatus>,
    /// Root then every multipatch copy (`instances.rs`); each receives the same write.
    pub instances: Box<[FixtureId]>,
    /// The owning control on every instance (empty without one). Never the other family's.
    pub footprint: Box<[NativeControlSlot]>,
    /// Digest of the owning control's physical response (see [`response_digest`]), computed
    /// once per descriptor compile, never per frame.
    pub response: u64,
    scratch: parking_lot::Mutex<OpticsScratch>,
}

struct OpticsScratch {
    workspace: OpticsFitWorkspace,
    output: Vec<OpticsFitResult>,
    requests: Vec<OpticsFitRequest>,
    native: CapturedNativeRaw,
    current: Vec<u32>,
}

/// This lane's last accepted write of one head/family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) struct OpticsContinuity {
    pub destination: FixtureId,
    /// `(channel_index, channel_id, raw)`; None when the family has no drivable control.
    pub control: Option<(u32, Uuid, u32)>,
    /// Witness: the captured pre-master baseline raw of the control in the frame that produced
    /// `control`. A different captured baseline is a fresh authored native command.
    pub baseline: Option<u32>,
    /// Witness: the physical-response digest `control` was fitted against.
    pub response: u64,
}

/// The composed request exactly as stored: normalized Focus (0..1) or Zoom full-opening degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::runtime) struct OpticsRequested {
    pub family: OpticsFamily,
    pub value: f64,
    /// Zoom only: the stored Beam/Field convention.
    pub convention: Option<OpeningConvention>,
}

/// Passive status of one encoded result. Never a notification.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::runtime) struct OpticsQuality {
    pub status: OpticsFitStatus,
    /// The request lies outside the reachable range; `achieved` is the nearest limit.
    pub clipped: bool,
    /// The footprint holds the current raw because the request could not be fitted.
    pub held: bool,
    /// Function of the written raw (fitted or held), when it is fittable/known.
    pub function_id: Option<Uuid>,
    /// Data quality of `achieved`; None when unknown.
    pub data_quality: Option<PhysicalDataQuality>,
    /// Focus only: native travel without a measured curve. Never a focal distance.
    pub nominal: bool,
    /// Zoom only: the profile's convention of `achieved` (None = unknown).
    pub convention: Option<OpeningConvention>,
    /// The control belongs to a master-shared head and moves every inheriting head.
    pub shared: bool,
}

/// Cumulative work of an adapter pair. Counters only; no budget is implied.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::runtime) struct OpticsAdapterCounters {
    pub descriptor_compiles: u64,
    pub fitting_compiles: u64,
    pub fitting_cache_hits: u64,
    /// Profiles whose optics model failed to compile (passive: the scalar path stays).
    pub fitting_failures: u64,
    /// Targets owning more than one head with the family (passive requirement).
    pub multi_head_targets: u64,
    pub resolves: u64,
    pub fitted: u64,
    pub clipped: u64,
    pub held: u64,
    /// Accepted continuity not overlaid because the captured baseline or the response changed.
    pub stale_continuity: u64,
}

type FittingCache = (
    Arc<Vec<PatchedFixture>>,
    FxHashMap<FixtureId, Option<Arc<CompiledOpticsFitting>>>,
);

/// Shared by the Focus and Zoom adapter of one pair. `Arc` + `Mutex` (TL-548 C0) only make the
/// adapter `Send`; one lane still evaluates one synchronous frame, so the locks never contend.
#[derive(Default)]
struct SharedOptics {
    fittings: Mutex<Option<FittingCache>>,
    counters: Mutex<OpticsAdapterCounters>,
}

/// A pair-shared value. A poisoned lock keeps its data: the cache and counters stay valid.
fn locked<T>(shared: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    shared.lock().unwrap_or_else(|error| error.into_inner())
}

/// Physical adapter of one optical family. Create both families with [`Self::pair`].
pub(in crate::runtime) struct OpticsAdapter {
    family: OpticsFamily,
    shared: Arc<SharedOptics>,
}

fn invalid(message: impl Into<String>) -> TransitionError {
    IntentError(message.into()).into()
}

pub(in crate::runtime) fn family_owner(family: OpticsFamily) -> ProgrammingOwner {
    match family {
        OpticsFamily::Focus => ProgrammingOwner::Focus,
        OpticsFamily::Zoom => ProgrammingOwner::Zoom,
    }
}

/// The stored request of `family`. Spreads must already be materialized per target.
fn requested(
    family: OpticsFamily,
    value: &AttributeValue,
) -> Result<OpticsRequested, TransitionError> {
    match (family, value) {
        (OpticsFamily::Focus, AttributeValue::Normalized(focus)) => Ok(OpticsRequested {
            family,
            value: f64::from(*focus),
            convention: None,
        }),
        (OpticsFamily::Zoom, AttributeValue::Zoom(zoom)) => match zoom.opening_degrees {
            ScalarIntent::Value(degrees) => Ok(OpticsRequested {
                family,
                value: f64::from(degrees),
                convention: Some(zoom.convention),
            }),
            ScalarIntent::Spread(_) => Err(invalid("Zoom carries an unresolved spread")),
        },
        _ => Err(invalid(format!(
            "{family:?} adapter received another family's value"
        ))),
    }
}

fn fit_request(request: &OpticsRequested) -> OpticsFitRequest {
    match request.family {
        OpticsFamily::Focus => OpticsFitRequest {
            focus: Some(FocusFitRequest {
                normalized: request.value,
                function_id: None,
            }),
            zoom: None,
        },
        OpticsFamily::Zoom => OpticsFitRequest {
            focus: None,
            zoom: Some(ZoomFitRequest {
                degrees: request.value,
                // Checked against the profile's calibrated convention; never converted.
                convention: request.convention,
                function_id: None,
            }),
        },
    }
}

/// Forward evaluation of the written raw: `(achieved, function, quality, nominal, convention)`.
type Forwarded = (
    Option<f64>,
    Option<Uuid>,
    Option<PhysicalDataQuality>,
    bool,
    Option<OpeningConvention>,
);

fn forwarded(family: OpticsFamily, forward: &OpticsForwardResult) -> Forwarded {
    match family {
        OpticsFamily::Focus => match (forward.focus_status, forward.focus) {
            (OpticsForwardStatus::Resolved, Some(focus)) => (
                Some(focus.percent / 100.),
                Some(focus.function_id),
                Some(focus.quality),
                focus.nominal,
                None,
            ),
            _ => (None, None, None, false, None),
        },
        OpticsFamily::Zoom => match (forward.zoom_status, forward.zoom) {
            (OpticsForwardStatus::Resolved, Some(zoom)) => (
                Some(zoom.degrees),
                Some(zoom.function_id),
                Some(zoom.quality),
                false,
                zoom.convention,
            ),
            _ => (None, None, None, false, None),
        },
    }
}

/// Digest of everything that determines the physical response of one owning control: its
/// resolution, transform and inversion, head and sharing, and every function's native range,
/// attribute, priority, behavior and calibration samples/convention/quality. Names and evidence
/// labels are excluded, so an unrelated show edit keeps the digest. Computed per descriptor
/// compile (per installed generation), never per frame.
fn response_digest(fixture: &PatchedFixture, control: &OpticsFitControl) -> u64 {
    use std::hash::{Hash, Hasher};
    let channel = fixture
        .definition
        .profile_snapshot
        .as_deref()
        .zip(fixture.definition.mode_id)
        .and_then(|(profile, mode)| profile.mode(mode))
        .and_then(|mode| mode.channels.get(control.channel_index as usize));
    let canonical = channel.map(|channel| {
        serde_json::json!({
            "id": channel.id,
            "head": channel.head_id,
            "split": channel.split,
            "resolution": channel.resolution,
            "invert": channel.invert,
            "transform": channel.canonical_transform,
            "shared": control.shared,
            "functions": channel.functions.iter().map(|f| serde_json::json!({
                "id": f.id,
                "from": f.dmx_from,
                "to": f.dmx_to,
                "attribute": f.attribute,
                "priority": f.priority,
                "behavior": f.behavior,
                "mapping": f.physical_mapping.as_ref().map(|m| serde_json::json!({
                    "quality": m.quality,
                    "samples": m.samples,
                    "convention": m.opening_convention,
                })),
            })).collect::<Vec<_>>(),
        })
    });
    let mut hasher = std::hash::DefaultHasher::new();
    serde_json::to_vec(&canonical)
        .unwrap_or_default()
        .hash(&mut hasher);
    hasher.finish()
}

fn compile_fitting(
    fixture: &PatchedFixture,
) -> Result<Option<CompiledOpticsFitting>, light_fixture::ProfileError> {
    let (Some(profile), Some(mode)) = (
        fixture.definition.profile_snapshot.as_deref(),
        fixture.definition.mode_id,
    ) else {
        return Ok(None);
    };
    let Some(mode) = profile.mode(mode) else {
        return Ok(None);
    };
    // Same forward model as the engine's physical projection (`CompiledOpticsForward`).
    CompiledOpticsFitting::compile(mode).map(Some)
}

impl OpticsAdapter {
    /// A Focus and a Zoom adapter sharing one fitter cache and one set of counters.
    pub fn pair() -> (Self, Self) {
        let shared = Arc::new(SharedOptics::default());
        (
            Self {
                family: OpticsFamily::Focus,
                shared: Arc::clone(&shared),
            },
            Self {
                family: OpticsFamily::Zoom,
                shared,
            },
        )
    }

    pub fn family(&self) -> OpticsFamily {
        self.family
    }

    pub fn counters(&self) -> OpticsAdapterCounters {
        *locked(&self.shared.counters)
    }

    fn count(&self, update: impl FnOnce(&mut OpticsAdapterCounters)) {
        update(&mut locked(&self.shared.counters));
    }

    /// One shared fitter per patched fixture of this exact fixture list.
    fn fitting(
        &self,
        snapshot: &EngineSnapshot,
        fixture: &PatchedFixture,
    ) -> Option<Arc<CompiledOpticsFitting>> {
        let mut cache = locked(&self.shared.fittings);
        if !cache
            .as_ref()
            .is_some_and(|(fixtures, _)| Arc::ptr_eq(fixtures, &snapshot.fixtures))
        {
            *cache = Some((Arc::clone(&snapshot.fixtures), FxHashMap::default()));
        }
        let entries = &mut cache.as_mut().expect("cache installed").1;
        if let Some(hit) = entries.get(&fixture.fixture_id) {
            self.count(|c| c.fitting_cache_hits += 1);
            return hit.clone();
        }
        let compiled = match compile_fitting(fixture) {
            Ok(compiled) => compiled.map(Arc::new),
            Err(_) => {
                self.count(|c| c.fitting_failures += 1);
                None
            }
        };
        self.count(|c| c.fitting_compiles += 1);
        entries.insert(fixture.fixture_id, compiled.clone());
        compiled
    }

    /// `current` = pre-master scalar baseline with this lane's last accepted write overlaid, only
    /// while the write's baseline and response witnesses still hold. Returns whether accepted
    /// continuity was dropped as stale.
    fn seed_current(
        request: &PhysicalRequest<'_, Self>,
        scratch: &mut OpticsScratch,
    ) -> Result<bool, TransitionError> {
        let descriptor = request.descriptor;
        request
            .frame
            .native_raw_into(request.target, &mut scratch.native)?;
        if scratch.native.destination() != Some(descriptor.destination)
            || scratch.native.token() != Some(request.frame.token)
        {
            return Err(invalid(
                "optics native raw values belong to another destination or frame",
            ));
        }
        scratch.current.clear();
        scratch.current.extend_from_slice(scratch.native.raw());
        let Some(previous) = request
            .previous
            .filter(|previous| previous.destination == descriptor.destination)
        else {
            return Ok(false);
        };
        if let (Some((index, id, raw)), Ok(control)) = (previous.control, descriptor.control)
            && index == control.channel_index
            && id == control.channel_id
            && raw <= control.raw_max
        {
            // A fresh authored native command or another physical response with the same
            // native identities: the old raw is not replayed.
            if previous.baseline == scratch.current.get(index as usize).copied()
                && previous.response == descriptor.response
            {
                scratch.current[index as usize] = raw;
            } else {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn quality(
        fit: &OpticsFit,
        forward: Forwarded,
        held: bool,
        control: Option<OpticsFitControl>,
    ) -> OpticsQuality {
        let (_, function_id, data_quality, nominal, convention) = forward;
        OpticsQuality {
            status: fit.status,
            clipped: fit.clipped,
            held,
            function_id,
            data_quality,
            nominal,
            convention,
            shared: control.is_some_and(|c| c.shared),
        }
    }

    fn record(&self, quality: &OpticsQuality) {
        self.count(|c| {
            c.resolves += 1;
            c.fitted += u64::from(quality.status == OpticsFitStatus::Fitted);
            c.clipped += u64::from(quality.clipped);
            c.held += u64::from(quality.held);
        });
    }
}

impl PhysicalFamilyAdapter for OpticsAdapter {
    type Descriptor = OpticsDescriptor;
    type Continuity = OpticsContinuity;
    type Requested = OpticsRequested;
    type Achieved = Option<f64>;
    type Quality = OpticsQuality;

    fn owns(&self, owner: ProgrammingOwner) -> bool {
        owner == family_owner(self.family)
    }

    fn compile(
        &self,
        snapshot: &EngineSnapshot,
        target: FixtureId,
    ) -> Result<Option<OpticsDescriptor>, TransitionError> {
        self.count(|c| c.descriptor_compiles += 1);
        let mut found = None;
        for head in profile_head_destinations(snapshot, target) {
            let fixture = &snapshot.fixtures[head.fixture_index];
            let Some(fitting) = self.fitting(snapshot, fixture) else {
                continue;
            };
            let Some(index) = fitting.head_index(head.head_id) else {
                continue;
            };
            let control = fitting
                .control(index, self.family)
                .ok_or_else(|| invalid("optics fitter head disappeared"))?;
            if control == Err(OpticsFitStatus::Unsupported) {
                continue;
            }
            if found.is_some() {
                self.count(|c| c.multi_head_targets += 1);
                return Ok(None);
            }
            found = Some((head, fixture, fitting, index, control));
        }
        let Some((head, fixture, fitting, index, control)) = found else {
            return Ok(None);
        };
        let response = control
            .as_ref()
            .map_or(0, |control| response_digest(fixture, control));
        let instances = instances::instance_destinations(fixture);
        let footprint = instances::instance_footprint(&instances, control.as_ref().ok());
        let scratch = OpticsScratch {
            workspace: fitting.create_workspace(),
            output: fitting.create_output(),
            requests: vec![OpticsFitRequest::default(); fitting.head_count()],
            native: CapturedNativeRaw::default(),
            current: Vec::new(),
        };
        Ok(Some(OpticsDescriptor {
            family: self.family,
            destination: head.destination,
            head: index,
            head_id: head.head_id,
            control,
            instances,
            footprint,
            response,
            scratch: parking_lot::Mutex::new(scratch),
            fitting,
        }))
    }

    fn footprint<'d>(&self, descriptor: &'d OpticsDescriptor) -> &'d [NativeControlSlot] {
        &descriptor.footprint
    }

    /// Current adoption of a legacy native Zoom scalar (normalized or raw channel value): the
    /// measured opening of this frame's native output through the compiled forward model. The
    /// percentage is never reinterpreted as degrees. Anything that cannot be measured in the
    /// requested convention stays a `ZoomConvention` requirement.
    fn adopt(
        &self,
        frame: HybridFrameContext<'_>,
        descriptor: &OpticsDescriptor,
        target: FixtureId,
        original: &AttributeValue,
        address: &DynamicValueAddress,
    ) -> Result<AttributeValue, TransitionError> {
        let requires = || {
            Err(TransitionError::Requires(
                TransitionRequirement::ZoomConvention,
            ))
        };
        let DynamicFamilyRepresentation::Zoom { convention } = address.representation else {
            return requires();
        };
        let legacy = matches!(
            original,
            AttributeValue::Normalized(_)
                | AttributeValue::RawDmx(_)
                | AttributeValue::RawDmxExact(_)
        );
        // The native baseline must be produced by exactly this value (no scalar Dynamic on it).
        if descriptor.family != OpticsFamily::Zoom
            || !legacy
            || descriptor.control.is_err()
            || frame.scalar.value(target, &ProgrammingOwner::Zoom.key()) != Some(original)
        {
            return requires();
        }
        let mut scratch = descriptor.scratch.lock();
        let OpticsScratch {
            workspace,
            output,
            requests,
            native,
            ..
        } = &mut *scratch;
        frame.native_raw_into(target, native)?;
        if native.destination() != Some(descriptor.destination)
            || native.token() != Some(frame.token)
        {
            return Err(invalid(
                "optics native raw values belong to another destination or frame",
            ));
        }
        // No request: the fitter only forward-evaluates the current native values.
        requests.fill(OpticsFitRequest::default());
        descriptor
            .fitting
            .fit(native.raw(), requests, workspace, output)
            .map_err(|error| invalid(format!("optics fitting input rejected: {error:?}")))?;
        match forwarded(OpticsFamily::Zoom, &workspace.forward()[descriptor.head]) {
            (Some(degrees), _, _, _, Some(measured))
                if measured == convention && degrees > 0. && degrees < 180. =>
            {
                Ok(AttributeValue::Zoom(Arc::new(ZoomIntent {
                    opening_degrees: ScalarIntent::Value(degrees as f32),
                    convention,
                })))
            }
            _ => requires(),
        }
    }

    fn resolve(
        &self,
        request: PhysicalRequest<'_, Self>,
    ) -> Result<PhysicalResolution<Self>, TransitionError> {
        let descriptor = request.descriptor;
        if request.owner != family_owner(self.family) || descriptor.family != self.family {
            return Err(invalid("optics adapter resolved another family's owner"));
        }
        let wanted = requested(self.family, request.value)?;
        let mut scratch = descriptor.scratch.lock();
        if Self::seed_current(&request, &mut scratch)? {
            self.count(|c| c.stale_continuity += 1);
        }
        let OpticsScratch {
            workspace,
            output,
            requests,
            current,
            native,
        } = &mut *scratch;
        requests.fill(OpticsFitRequest::default());
        requests[descriptor.head] = fit_request(&wanted);
        descriptor
            .fitting
            .fit(current, requests, workspace, output)
            .map_err(|error| invalid(format!("optics fitting input rejected: {error:?}")))?;
        let result = &output[descriptor.head];
        let fit = match self.family {
            OpticsFamily::Focus => result.focus,
            OpticsFamily::Zoom => result.zoom,
        };
        let control = descriptor.control.ok();
        let (writes, held) = match (control, fit.write) {
            (Some(_), Some(write)) => {
                let mut native = NativeControlWrite::from_optics(descriptor.destination, &write);
                native.function_id = fit.function_id;
                (instances::replicate(&descriptor.instances, native), false)
            }
            // Unfittable request: hold the current raw, the stored request stays unchanged.
            (Some(control), None) => (
                instances::replicate(
                    &descriptor.instances,
                    NativeControlWrite {
                        slot: descriptor.footprint[0],
                        channel_id: control.channel_id,
                        function_id: None,
                        raw: current[control.channel_index as usize],
                        parked: true,
                    },
                ),
                true,
            ),
            (None, _) => (Vec::new(), false),
        };
        // The fitter evaluated exactly the written raw (fitted, or held unchanged).
        let forward = forwarded(self.family, &workspace.forward()[descriptor.head]);
        let quality = Self::quality(&fit, forward, held, control);
        self.record(&quality);
        Ok(PhysicalResolution {
            continuity: OpticsContinuity {
                destination: descriptor.destination,
                control: writes
                    .first()
                    .map(|w| (w.slot.channel_index, w.channel_id, w.raw)),
                baseline: writes
                    .first()
                    .and_then(|w| native.raw().get(w.slot.channel_index as usize).copied()),
                response: descriptor.response,
            },
            requested: wanted,
            achieved: forward.0,
            quality,
            writes,
        })
    }
}

/// A Focus lane and a Zoom lane of one evaluating lane (Live or one Preload branch), used as
/// one hybrid resolver and observer. Each owner is routed to its own lane; begin/verify/accept
/// apply to both so the pair commits or abandons together.
pub(in crate::runtime) struct OpticsLanes {
    focus: PhysicalAdapterLane<OpticsAdapter>,
    zoom: PhysicalAdapterLane<OpticsAdapter>,
}

impl OpticsLanes {
    fn with(kind: PhysicalLaneKind) -> Self {
        let (focus, zoom) = OpticsAdapter::pair();
        let lane = |adapter| match kind {
            PhysicalLaneKind::Live => PhysicalAdapterLane::live(adapter),
            PhysicalLaneKind::Preload(branch) => PhysicalAdapterLane::preload(adapter, branch),
        };
        Self {
            focus: lane(focus),
            zoom: lane(zoom),
        }
    }

    pub fn live() -> Self {
        Self::with(PhysicalLaneKind::Live)
    }

    pub fn preload(branch: PreloadBranch) -> Self {
        Self::with(PhysicalLaneKind::Preload(branch))
    }

    pub fn lane(&self, family: OpticsFamily) -> &PhysicalAdapterLane<OpticsAdapter> {
        match family {
            OpticsFamily::Focus => &self.focus,
            OpticsFamily::Zoom => &self.zoom,
        }
    }

    fn lane_for(
        &self,
        owner: ProgrammingOwner,
    ) -> Result<&PhysicalAdapterLane<OpticsAdapter>, TransitionError> {
        match owner {
            ProgrammingOwner::Focus => Ok(&self.focus),
            ProgrammingOwner::Zoom => Ok(&self.zoom),
            other => Err(TransitionError::Requires(owner_requirement(other))),
        }
    }

    /// Shared counters of both lanes.
    pub fn counters(&self) -> OpticsAdapterCounters {
        self.focus.adapter().counters()
    }

    pub fn observe(
        &self,
        observation: HybridFamilyObservation<'_>,
    ) -> Result<(FamilyProjectionMetadata, PhysicalHeadResult<OpticsAdapter>), TransitionError>
    {
        self.lane_for(observation.owner)?.observe(observation)
    }

    /// Owners released by the most recently accepted frame, Focus then Zoom.
    pub fn released(&self) -> Vec<ReleasedPhysicalOwner> {
        let mut released = self.focus.released();
        released.extend(self.zoom.released());
        released
    }

    pub fn abandon(&self) {
        self.focus.abandon();
        self.zoom.abandon();
    }
}

impl HybridFrameResolver for OpticsLanes {
    fn adopt(
        &self,
        frame: HybridFrameContext<'_>,
        target: FixtureId,
        original: &AttributeValue,
        address: &DynamicValueAddress,
    ) -> Result<AttributeValue, TransitionError> {
        self.lane_for(address.owner())?
            .adopt(frame, target, original, address)
    }

    /// Endpoints of one family route to its lane. Zoom convention changes and every other
    /// cross-representation transition stay a passive requirement (never a conversion factor).
    fn resolve(
        &self,
        frame: HybridFrameContext<'_>,
        target: FixtureId,
        requirement: TransitionRequirement,
        from: &AttributeValue,
        to: &AttributeValue,
        operation: FamilyExpressionOperation,
    ) -> Result<(AttributeValue, Option<ProgrammingTransitionTrace>), TransitionError> {
        let lane = match (from, to) {
            (AttributeValue::Zoom(_), AttributeValue::Zoom(_)) => &self.zoom,
            (AttributeValue::Normalized(_), AttributeValue::Normalized(_)) => &self.focus,
            _ => return Err(TransitionError::Requires(requirement)),
        };
        lane.resolve(frame, target, requirement, from, to, operation)
    }

    fn begin_frame(&self, token: &CapturedFrameToken) -> Result<(), TransitionError> {
        self.focus.begin_frame(token)?;
        self.zoom.begin_frame(token).inspect_err(|_| self.abandon())
    }

    /// TL-602: passive requirements of a successfully prepared frame hold prior continuity in
    /// their own lane only (each lane keeps only the owners it owns), so a Zoom limitation never
    /// holds or suppresses Focus and vice versa. A foreign or stale token is rejected by the
    /// Focus lane before either lane changes; a later Zoom rejection abandons the pair.
    fn hold_frame(
        &self,
        token: &CapturedFrameToken,
        requirements: &[HybridFamilyRequirement],
    ) -> Result<(), TransitionError> {
        self.focus.hold_frame(token, requirements)?;
        self.zoom
            .hold_frame(token, requirements)
            .inspect_err(|_| self.abandon())
    }

    fn verify_frame(&self, token: &CapturedFrameToken) -> Result<(), TransitionError> {
        self.focus.verify_frame(token)?;
        self.zoom.verify_frame(token)
    }

    fn accept_frame(&self, token: &CapturedFrameToken) -> bool {
        // Both lanes staged this token (begin/verify); commit both or neither.
        let accepted = self.focus.accept_frame(token);
        accepted && self.zoom.accept_frame(token)
    }
}

/// Independent Before/After Release optics lanes of one retained Preload episode.
pub(in crate::runtime) struct OpticsPreloadLanes {
    before: OpticsLanes,
    after: OpticsLanes,
}

impl Default for OpticsPreloadLanes {
    fn default() -> Self {
        Self {
            before: OpticsLanes::preload(PreloadBranch::BeforeRelease),
            after: OpticsLanes::preload(PreloadBranch::AfterRelease),
        }
    }
}

impl OpticsPreloadLanes {
    pub fn lanes(&self, branch: PreloadBranch) -> &OpticsLanes {
        match branch {
            PreloadBranch::BeforeRelease => &self.before,
            PreloadBranch::AfterRelease => &self.after,
        }
    }

    fn lanes_for(&self, token: &CapturedFrameToken) -> Result<&OpticsLanes, TransitionError> {
        match token.lane().preload_branch() {
            Some(branch) => Ok(self.lanes(branch)),
            None => Err(invalid("a Live token cannot address a Preload optics lane")),
        }
    }

    pub fn observe(
        &self,
        branch: PreloadBranch,
        observation: HybridFamilyObservation<'_>,
    ) -> Result<(FamilyProjectionMetadata, PhysicalHeadResult<OpticsAdapter>), TransitionError>
    {
        if observation.frame.token.lane().preload_branch() != Some(branch) {
            return Err(invalid(
                "Preload observation token belongs to another branch",
            ));
        }
        self.lanes(branch).observe(observation)
    }
}

impl HybridFrameResolver for OpticsPreloadLanes {
    fn adopt(
        &self,
        frame: HybridFrameContext<'_>,
        target: FixtureId,
        original: &AttributeValue,
        address: &DynamicValueAddress,
    ) -> Result<AttributeValue, TransitionError> {
        self.lanes_for(frame.token)?
            .adopt(frame, target, original, address)
    }

    fn resolve(
        &self,
        frame: HybridFrameContext<'_>,
        target: FixtureId,
        requirement: TransitionRequirement,
        from: &AttributeValue,
        to: &AttributeValue,
        operation: FamilyExpressionOperation,
    ) -> Result<(AttributeValue, Option<ProgrammingTransitionTrace>), TransitionError> {
        self.lanes_for(frame.token)?
            .resolve(frame, target, requirement, from, to, operation)
    }

    fn begin_frame(&self, token: &CapturedFrameToken) -> Result<(), TransitionError> {
        self.lanes_for(token)?.begin_frame(token)
    }

    /// Holds route to the Before or After pair named by the token's own branch.
    fn hold_frame(
        &self,
        token: &CapturedFrameToken,
        requirements: &[HybridFamilyRequirement],
    ) -> Result<(), TransitionError> {
        self.lanes_for(token)?.hold_frame(token, requirements)
    }

    fn verify_frame(&self, token: &CapturedFrameToken) -> Result<(), TransitionError> {
        self.lanes_for(token)?.verify_frame(token)
    }

    fn accept_frame(&self, token: &CapturedFrameToken) -> bool {
        self.lanes_for(token)
            .is_ok_and(|lanes| lanes.accept_frame(token))
    }
}

/// [`finalize_live_physical_frame`] for the Focus/Zoom lane pair: every token check and the
/// cross-head shared-control verification run before the engine render; both lanes commit only
/// after it succeeded. On any error nothing is published and both lanes keep their state.
pub(in crate::runtime::output_scheduler::dynamic_projection) fn finalize_live_optics_frame(
    engine: &Engine,
    capture: &PreparedOutputFrame,
    lanes: &OpticsLanes,
    prepared: PreparedHybridFrame<PhysicalHeadResult<OpticsAdapter>>,
) -> Result<PublishedPhysicalFrame<OpticsAdapter>, DynamicRuntimeError> {
    super::live_frame::finalize_live_lanes_frame(engine, capture, lanes, prepared)
}
