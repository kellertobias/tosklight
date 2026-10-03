//! Pure inverse of the calibrated Position forward graph. The caller supplies one captured
//! physical-copy mount and native state, resolved world targets and previous commanded joints.
//! Point binding, branch continuity installation and output ownership remain runtime duties.
//!
//! Angles never wrap. Target solving uses the moving lens origin, exact profile geometry and
//! encoded forward verification. Missing/unreachable targets propose no writes. Workspaces are
//! model-bound, reusable and independent; fitting neither allocates nor mutates a stored intent.
use crate::forward::{
    AxisForwardCommand, CompiledPositionForward, LensForwardPose, PositionForwardFlags,
    PositionForwardWorkspace, PositionInstallation,
};
use crate::{FixtureProfile, PhysicalDataQuality, PositionAxisRole, ProfileError};
use light_core::spatial::RigidTransform as R;
use uuid::Uuid;

mod solve;
use solve::TargetGoal;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PositionFitRequest {
    Angles {
        pan: f64,
        tilt: f64,
    },
    /// Metres, in the same PROFILE-world basis as `mount`. A caller resolves a Point's local
    /// offset with its captured rigid transform before supplying it. None is an unresolved aim.
    Target {
        world: Option<[f64; 3]>,
    },
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PositionFitStatus {
    #[default]
    NotRequested,
    Fitted,
    Unsupported,
    AmbiguousAxes,
    UnknownAxis,
    UnavailableInput,
    MissingTarget,
    /// The target coincides with the previous commanded lens origin: hold because the current
    /// aim ray is undefined, even if a distant alternate mechanical pose could reach that point.
    CoincidentTarget,
    /// Bounded search found no forward-consistent solution. This is not a global geometric
    /// impossibility proof; the caller holds the previous native command.
    UnreachableTarget,
    InvalidRequest,
    VelocityAuthority,
    OwnershipConflict,
    UnsupportedGeometry,
    SolverCapacity,
    ForwardMismatch,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionControlWrite {
    pub channel_index: u32,
    pub channel_id: Uuid,
    pub split: u16,
    pub function_id: Uuid,
    pub raw: u32,
}
/// Native channel ownership footprint. Includes absolute and velocity drivers, even when an
/// axis cannot currently be fitted. Channel indices address the full mode's native input slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionFitControlMetadata {
    pub channel_index: u32,
    pub channel_id: Uuid,
    pub split: u16,
    pub raw_max: u32,
}
/// Cold axis metadata in the exact order used by commands, previous joints and achieved axes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PositionFitAxisMetadata {
    pub command_index: usize,
    pub node_id: Uuid,
    pub role: Option<PositionAxisRole>,
    /// All driver channels, deduplicated and ordered by native channel index.
    pub controls: Box<[PositionFitControlMetadata]>,
}
/// Borrowed cold lens metadata in the same order as requests and fit results.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionFitEmitterMetadata<'a> {
    pub emitter_index: usize,
    pub emitter_id: Uuid,
    pub head_id: Option<Uuid>,
    /// Pan then Tilt axis indices. None means a missing or ambiguous role; ancestor ownership
    /// remains available below and must still be protected when the requested family holds.
    pub command_indices: Option<[usize; 2]>,
    /// All physical ancestor axes, leaf to root, indexing `CompiledPositionFitting::axes()`.
    /// Includes unbound axes without a Pan/Tilt role (for example a translating fixture mount).
    pub ancestor_axes: &'a [usize],
    /// Complete ancestor driver footprint, including velocity channels and ambiguous roles.
    pub controls: &'a [PositionFitControlMetadata],
}
#[derive(Clone, Debug, PartialEq)]
pub struct PositionFitResult {
    pub emitter_id: Uuid,
    pub head_id: Option<Uuid>,
    pub requested: Option<PositionFitRequest>,
    pub status: PositionFitStatus,
    /// Forward-decoded unwrapped commanded joints, not measured motor feedback.
    pub achieved: Option<[f64; 2]>,
    pub pose: Option<R>,
    pub angular_error_degrees: Option<f64>,
    pub clipped: bool,
    /// A global candidate-evaluation budget was reached; closest explored solution is reported.
    pub search_limited: bool,
    pub quality: PhysicalDataQuality,
    pub flags: PositionForwardFlags,
    /// Pan then Tilt. Fitted results carry both writes or neither; they form one family.
    pub writes: [Option<PositionControlWrite>; 2],
}
/// One captured physical-copy input. All quantities belong to the caller's same frame;
/// this fixture helper does not look up a show, Point, tracking source or another output lane.
#[derive(Clone, Copy, Debug)]
pub struct PositionFitInput<'a> {
    pub current_raw: &'a [u32],
    pub available: &'a [bool],
    pub requests: &'a [Option<PositionFitRequest>],
    /// Previous commanded joints, plus explicit captured values for unbound physical ancestors.
    /// Continuity never establishes an absolute pose while a velocity command is active.
    pub previous: &'a [Option<f64>],
    pub mount: R,
}
#[derive(Clone, Copy)]
struct FitContext<'a> {
    raw: &'a [u32],
    available: &'a [bool],
    previous: &'a [Option<f64>],
    mount: R,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PositionFitInputError {
    ChannelLayout,
    RawOutOfRange,
    RequestLayout,
    JointLayout,
    OutputLayout,
    WorkspaceLayout,
    NonfiniteJoint,
    InvalidMount,
}
#[derive(Clone, Debug)]
struct LensPlan {
    id: Uuid,
    head: Option<Uuid>,
    axes: Option<[usize; 2]>,
    dependencies: Box<[usize]>,
    ancestry: Box<[usize]>,
    controls: Box<[PositionFitControlMetadata]>,
    unsupported: PositionFitStatus,
}
#[derive(Clone, Copy, Debug)]
struct Channel {
    id: Uuid,
    split: u16,
    maximum: u32,
}
#[derive(Clone, Copy, Debug, Default)]
struct Claim {
    raw: Option<u32>,
    conflict: bool,
}
#[derive(Clone, Debug)]
pub struct CompiledPositionFitting {
    forward: CompiledPositionForward,
    lenses: Box<[LensPlan]>,
    channels: Box<[Channel]>,
    axis_metadata: Box<[PositionFitAxisMetadata]>,
}
#[derive(Clone, Debug)]
pub struct PositionFitWorkspace {
    model: Uuid,
    raw: Vec<u32>,
    trial_raw: Vec<u32>,
    commands: Vec<AxisForwardCommand>,
    axes: Vec<Option<f64>>,
    trial_axes: Vec<Option<f64>>,
    geometry: PositionForwardWorkspace,
    poses: Vec<LensForwardPose>,
    claims: Vec<Claim>,
    evaluations: usize,
}
impl PositionFitWorkspace {
    pub fn proposed_raw(&self) -> &[u32] {
        &self.raw
    }
    /// Final forward-decoded commanded axes after the last successful `fit` call, in compiled
    /// axis order. A fresh workspace contains None; unavailable inputs or active velocity retain
    /// None rather than fabricating an absolute pose. Unbound ancestors use explicit captured
    /// input values. Invalid fit input leaves this slice unchanged. Install continuity only when
    /// the caller accepts the resulting native proposal, never merely because fitting ran.
    pub fn achieved_axes(&self) -> &[Option<f64>] {
        &self.trial_axes
    }
    /// Actual ancestor-ray evaluations in the last solve (bounded globally per physical copy).
    pub fn candidate_evaluations(&self) -> usize {
        self.evaluations
    }
    pub fn forward(&self) -> &[LensForwardPose] {
        &self.poses
    }
}
impl CompiledPositionFitting {
    pub fn compile(
        profile: &FixtureProfile,
        mode_id: Uuid,
        installed: PositionInstallation<'_>,
    ) -> Result<Option<Self>, ProfileError> {
        let Some(forward) = CompiledPositionForward::compile(profile, mode_id, installed)? else {
            return Ok(None);
        };
        let mode = profile.modes.iter().find(|m| m.id == mode_id).unwrap();
        let graph = forward.pose_graph();
        let control_metadata = |channel_index: usize| {
            let channel = &mode.channels[channel_index];
            PositionFitControlMetadata {
                channel_index: channel_index as u32,
                channel_id: channel.id,
                split: channel.split,
                raw_max: channel.resolution.max_raw(),
            }
        };
        let axis_metadata: Box<[PositionFitAxisMetadata]> = forward
            .fitting_axes()
            .iter()
            .enumerate()
            .map(|(command_index, axis)| {
                let mut channels: Vec<usize> = axis.drivers.iter().map(|d| d.channel).collect();
                channels.sort_unstable();
                channels.dedup();
                PositionFitAxisMetadata {
                    command_index,
                    node_id: axis.node_id,
                    role: axis.role,
                    controls: channels.into_iter().map(&control_metadata).collect(),
                }
            })
            .collect();
        let lenses = forward
            .create_output()
            .iter()
            .enumerate()
            .map(|(index, pose)| {
                let dependencies = graph.lens_axis_indices(index);
                let mut axes = [None; 2];
                let mut unsupported = PositionFitStatus::Unsupported;
                for &i in &dependencies {
                    let role = match forward.fitting_axes()[i].role {
                        Some(PositionAxisRole::Pan) => 0,
                        Some(PositionAxisRole::Tilt) => 1,
                        None => continue,
                    };
                    if axes[role].replace(i).is_some() {
                        unsupported = PositionFitStatus::AmbiguousAxes;
                    }
                }
                let pair = axes[0]
                    .zip(axes[1])
                    .filter(|_| unsupported != PositionFitStatus::AmbiguousAxes)
                    .map(|(pan, tilt)| [pan, tilt]);
                let mut controls: Vec<PositionFitControlMetadata> = dependencies
                    .iter()
                    .flat_map(|&i| axis_metadata[i].controls.iter().copied())
                    .collect();
                controls.sort_unstable_by_key(|c| c.channel_index);
                controls.dedup_by_key(|c| c.channel_index);
                LensPlan {
                    id: pose.emitter_id,
                    head: mode
                        .emitter_heads
                        .iter()
                        .find(|h| h.emitter_id == pose.emitter_id)
                        .map(|h| h.head_id)
                        .or(pose.head_id),
                    axes: pair,
                    dependencies,
                    ancestry: forward.fitting_ancestry(index),
                    controls: controls.into_boxed_slice(),
                    unsupported,
                }
            })
            .collect();
        let channels = mode
            .channels
            .iter()
            .map(|c| Channel {
                id: c.id,
                split: c.split,
                maximum: c.resolution.max_raw(),
            })
            .collect();
        Ok(Some(Self {
            forward,
            lenses,
            channels,
            axis_metadata,
        }))
    }
    /// The whole-vector layout and range validation every fit applies to its native values.
    /// Beyond it a fit reads native values and availability only at the axis drivers' channels
    /// (`axes()[..].controls`), which lets a caller key an exact result on those channels.
    pub fn accepts_raw(&self, current_raw: &[u32]) -> bool {
        current_raw.len() == self.channels.len()
            && current_raw
                .iter()
                .zip(&self.channels)
                .all(|(raw, channel)| *raw <= channel.maximum)
    }
    /// Cold axis records. Reading metadata does not allocate or decode native input.
    pub fn axes(&self) -> &[PositionFitAxisMetadata] {
        &self.axis_metadata
    }
    /// Ordered lens/head records sharing the request/output index convention.
    pub fn emitters(&self) -> impl ExactSizeIterator<Item = PositionFitEmitterMetadata<'_>> {
        self.lenses
            .iter()
            .enumerate()
            .map(|(index, lens)| Self::emitter_metadata(index, lens))
    }
    pub fn emitter(&self, index: usize) -> Option<PositionFitEmitterMetadata<'_>> {
        self.lenses
            .get(index)
            .map(|lens| Self::emitter_metadata(index, lens))
    }
    fn emitter_metadata(index: usize, lens: &LensPlan) -> PositionFitEmitterMetadata<'_> {
        PositionFitEmitterMetadata {
            emitter_index: index,
            emitter_id: lens.id,
            head_id: lens.head,
            command_indices: lens.axes,
            ancestor_axes: &lens.dependencies,
            controls: &lens.controls,
        }
    }
    /// Compiled axis order, also used for previous joints. Some non-Pan/Tilt ancestors may be
    /// translations (millimetres); supply their captured physical value, never an invented zero.
    pub fn create_commands(&self) -> Vec<AxisForwardCommand> {
        self.forward.create_commands()
    }
    pub fn create_workspace(&self) -> PositionFitWorkspace {
        PositionFitWorkspace {
            model: self.forward.pose_graph().id(),
            raw: vec![0; self.channels.len()],
            trial_raw: vec![0; self.channels.len()],
            commands: self.forward.create_commands(),
            axes: vec![None; self.forward.fitting_axes().len()],
            trial_axes: vec![None; self.forward.fitting_axes().len()],
            geometry: self.forward.create_workspace(),
            poses: self.forward.create_output(),
            claims: vec![Claim::default(); self.channels.len()],
            evaluations: 0,
        }
    }
    pub fn create_output(&self) -> Vec<PositionFitResult> {
        self.lenses
            .iter()
            .map(|l| PositionFitResult {
                emitter_id: l.id,
                head_id: l.head,
                requested: None,
                status: PositionFitStatus::NotRequested,
                achieved: None,
                pose: None,
                angular_error_degrees: None,
                clipped: false,
                search_limited: false,
                quality: PhysicalDataQuality::Unknown,
                flags: PositionForwardFlags::default(),
                writes: [None; 2],
            })
            .collect()
    }
    /// Requests/output follow create_output's emitter order. Invalid buffer/native/mount inputs
    /// fail before any workspace or output mutation. Passive per-emitter failures preserve native
    /// controls. Previous joints are explicit caller-owned continuity, never borrowed from Live.
    pub fn fit(
        &self,
        input: PositionFitInput<'_>,
        workspace: &mut PositionFitWorkspace,
        output: &mut [PositionFitResult],
    ) -> Result<(), PositionFitInputError> {
        let PositionFitInput {
            current_raw,
            available,
            requests,
            previous,
            mount,
        } = input;
        let context = FitContext {
            raw: current_raw,
            available,
            previous,
            mount,
        };
        self.check_fit_layout(input, workspace, output)?;
        // All mutable buffers are private and created together by this model.
        workspace.raw.copy_from_slice(current_raw);
        workspace.evaluations = 0;
        self.forward
            .decode_commands(current_raw, &mut workspace.commands)
            .unwrap();
        for (i, value) in workspace.axes.iter_mut().enumerate() {
            *value = if self.forward.inputs_available(i, available) {
                workspace.commands[i].absolute_degrees().or_else(|| {
                    workspace.commands[i]
                        .velocity
                        .is_none()
                        .then_some(previous[i])
                        .flatten()
                })
            } else {
                None
            };
        }
        for (index, out) in output.iter_mut().enumerate() {
            out.requested = requests[index];
            out.status = PositionFitStatus::NotRequested;
            out.achieved = None;
            out.pose = None;
            out.angular_error_degrees = None;
            out.clipped = false;
            out.search_limited = false;
            out.writes = [None; 2];
            if let Some(request) = requests[index] {
                self.fit_lens(index, request, context, workspace, out);
            }
        }
        self.withhold_ownership_conflicts(current_raw, workspace, output);
        self.report_achieved(context, workspace, output);
        Ok(())
    }
    /// Reject mismatched buffers, out-of-range native values and non-rigid mounts before any
    /// workspace or output mutation.
    fn check_fit_layout(
        &self,
        input: PositionFitInput<'_>,
        workspace: &PositionFitWorkspace,
        output: &[PositionFitResult],
    ) -> Result<(), PositionFitInputError> {
        let PositionFitInput {
            current_raw,
            available,
            requests,
            previous,
            mount,
        } = input;
        use PositionFitInputError as E;
        if current_raw.len() != self.channels.len() || available.len() != self.channels.len() {
            return Err(E::ChannelLayout);
        }
        if current_raw
            .iter()
            .zip(&self.channels)
            .any(|(r, c)| *r > c.maximum)
        {
            return Err(E::RawOutOfRange);
        }
        if requests.len() != self.lenses.len() {
            return Err(E::RequestLayout);
        }
        if previous.len() != self.forward.fitting_axes().len() {
            return Err(E::JointLayout);
        }
        if previous.iter().flatten().any(|v| !v.is_finite()) {
            return Err(E::NonfiniteJoint);
        }
        if !mount.point([0.; 3]).iter().all(|v| v.is_finite())
            || ![[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]]
                .iter()
                .all(|&a| (dot(mount.direction(a), mount.direction(a)) - 1.).abs() < 1e-8)
            || dot(mount.direction([1., 0., 0.]), mount.direction([0., 1., 0.])).abs() > 1e-8
            || dot(mount.direction([0., 1., 0.]), mount.direction([0., 0., 1.])).abs() > 1e-8
            || dot(mount.direction([1., 0., 0.]), mount.direction([0., 0., 1.])).abs() > 1e-8
            || (dot(
                cross(mount.direction([1., 0., 0.]), mount.direction([0., 1., 0.])),
                mount.direction([0., 0., 1.]),
            ) - 1.)
                .abs()
                > 1e-8
        {
            return Err(E::InvalidMount);
        }
        if output.len() != self.lenses.len()
            || output
                .iter()
                .zip(&self.lenses)
                .any(|(o, l)| o.emitter_id != l.id || o.head_id != l.head)
        {
            return Err(E::OutputLayout);
        }
        if workspace.model != self.forward.pose_graph().id() {
            return Err(E::WorkspaceLayout);
        }
        Ok(())
    }
    /// Claim every fitted write and passive hold, then withhold whole conflicting cohorts.
    fn withhold_ownership_conflicts(
        &self,
        current_raw: &[u32],
        workspace: &mut PositionFitWorkspace,
        output: &mut [PositionFitResult],
    ) {
        workspace.claims.fill(Claim::default());
        for out in output
            .iter()
            .filter(|o| o.status == PositionFitStatus::Fitted)
        {
            for w in out.writes.iter().flatten() {
                let claim = &mut workspace.claims[w.channel_index as usize];
                claim.conflict |= claim.raw.is_some_and(|raw| raw != w.raw);
                claim.raw = Some(w.raw);
            }
        }
        // Requested passive holds own their current controls too. A successful peer may not
        // move a failed target through a shared Pan (or Tilt). Unrequested emitters are observers.
        for (index, out) in output.iter().enumerate() {
            if out.requested.is_some() && out.status != PositionFitStatus::Fitted {
                for &i in &self.lenses[index].dependencies {
                    if self.forward.fitting_axes()[i].role.is_none() {
                        continue;
                    }
                    for driver in &self.forward.fitting_axes()[i].drivers {
                        let claim = &mut workspace.claims[driver.channel];
                        claim.conflict |=
                            claim.raw.is_some_and(|r| r != current_raw[driver.channel]);
                    }
                }
            }
        }
        // Close conflicts over the entire connected Pan/Tilt cohort. A later compatible peer
        // cannot write the other control of an already withheld family. Bounded by emitter count.
        loop {
            let mut changed = false;
            for out in output
                .iter_mut()
                .filter(|o| o.status == PositionFitStatus::Fitted)
            {
                if out
                    .writes
                    .iter()
                    .flatten()
                    .any(|w| workspace.claims[w.channel_index as usize].conflict)
                {
                    for w in out.writes.iter().flatten() {
                        workspace.claims[w.channel_index as usize].conflict = true;
                    }
                    out.status = PositionFitStatus::OwnershipConflict;
                    out.writes = [None; 2];
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
    }
    /// Apply the surviving writes and report the achieved joints and pose of every emitter.
    fn report_achieved(
        &self,
        context: FitContext<'_>,
        workspace: &mut PositionFitWorkspace,
        output: &mut [PositionFitResult],
    ) {
        let FitContext {
            available, mount, ..
        } = context;
        for out in output
            .iter()
            .filter(|o| o.status == PositionFitStatus::Fitted)
        {
            for w in out.writes.iter().flatten() {
                workspace.raw[w.channel_index as usize] = w.raw;
            }
        }
        self.forward
            .decode_commands(&workspace.raw, &mut workspace.commands)
            .unwrap();
        for (i, axis) in workspace.trial_axes.iter_mut().enumerate() {
            *axis = if self.forward.inputs_available(i, available) {
                workspace.commands[i].absolute_degrees().or_else(|| {
                    workspace.commands[i]
                        .velocity
                        .is_none()
                        .then_some(workspace.axes[i])
                        .flatten()
                })
            } else {
                None
            };
        }
        self.forward
            .evaluate_pose(
                &workspace.trial_axes,
                mount,
                &mut workspace.geometry,
                &mut workspace.poses,
            )
            .unwrap();
        for (index, out) in output.iter_mut().enumerate() {
            let pose = &workspace.poses[index];
            out.pose = pose.world;
            out.flags = pose.flags;
            out.quality = pose.geometry_quality;
            if let Some([pan, tilt]) = self.lenses[index].axes {
                out.achieved = workspace.trial_axes[pan]
                    .zip(workspace.trial_axes[tilt])
                    .map(|(p, t)| [p, t]);
                for i in [pan, tilt] {
                    if let Some(a) = workspace.commands[i].absolute {
                        out.quality = quality_min(out.quality, a.quality);
                    }
                }
            }
            if let Some(PositionFitRequest::Target {
                world: Some(target),
            }) = out.requested
            {
                out.angular_error_degrees = pose.world.and_then(|p| ray_error(p, target));
            }
        }
    }
    fn fit_lens(
        &self,
        index: usize,
        request: PositionFitRequest,
        context: FitContext<'_>,
        ws: &mut PositionFitWorkspace,
        out: &mut PositionFitResult,
    ) {
        let FitContext {
            available,
            previous,
            ..
        } = context;
        let lens = &self.lenses[index];
        let Some(pair) = lens.axes else {
            out.status = lens.unsupported;
            return;
        };
        if pair
            .iter()
            .any(|&i| !self.forward.inputs_available(i, available))
        {
            out.status = PositionFitStatus::UnavailableInput;
            return;
        }
        if lens
            .dependencies
            .iter()
            .any(|&i| !pair.contains(&i) && ws.axes[i].is_none())
        {
            out.status = PositionFitStatus::UnknownAxis;
            return;
        }
        if pair.iter().any(|&i| {
            !self.forward.fitting_axes()[i]
                .drivers
                .iter()
                .any(|d| !d.velocity)
        }) {
            out.status = PositionFitStatus::VelocityAuthority;
            return;
        }
        let anchor = pair.map(|i| previous[i].or(ws.axes[i]).unwrap_or(0.));
        match request {
            PositionFitRequest::Angles { pan, tilt } => {
                if !pan.is_finite() || !tilt.is_finite() {
                    out.status = PositionFitStatus::InvalidRequest;
                    return;
                }
                let desired = [pan, tilt];
                let mut chosen = [None; 2];
                for j in 0..2 {
                    let axis = &self.forward.fitting_axes()[pair[j]];
                    let mut distance = f64::INFINITY;
                    for driver in axis.drivers.iter().filter(|d| !d.velocity) {
                        let mapped = driver
                            .mapping
                            .raw_for_physical(axis.calibration.calibrated_to_physical(desired[j]))
                            .unwrap();
                        let achieved = axis.calibration.physical_to_calibrated(mapped.physical);
                        let error = (achieved - desired[j]).abs();
                        if error < distance {
                            distance = error;
                            chosen[j] = Some((driver, mapped));
                        }
                    }
                }
                let writes = std::array::from_fn(|j| {
                    let (driver, mapped) = chosen[j].unwrap();
                    out.clipped |= mapped.clipped;
                    self.write(driver.channel, driver.mapping.function_id, mapped.raw)
                });
                self.verify_pair(index, pair, writes, context, ws, out);
            }
            PositionFitRequest::Target { world: None } => {
                out.status = PositionFitStatus::MissingTarget
            }
            PositionFitRequest::Target {
                world: Some(target),
            } => {
                if target.iter().any(|v| !v.is_finite()) {
                    out.status = PositionFitStatus::InvalidRequest;
                    return;
                }
                self.solve_target(index, pair, TargetGoal { anchor, target }, context, ws, out);
            }
        }
    }
    fn write(&self, channel: usize, function_id: Uuid, raw: u32) -> PositionControlWrite {
        PositionControlWrite {
            channel_index: channel as u32,
            channel_id: self.channels[channel].id,
            split: self.channels[channel].split,
            function_id,
            raw,
        }
    }
    fn verify_pair(
        &self,
        index: usize,
        pair: [usize; 2],
        writes: [PositionControlWrite; 2],
        context: FitContext<'_>,
        ws: &mut PositionFitWorkspace,
        out: &mut PositionFitResult,
    ) -> bool {
        let FitContext { raw, mount, .. } = context;
        if writes[0].channel_index == writes[1].channel_index {
            out.status = PositionFitStatus::OwnershipConflict;
            return false;
        }
        ws.trial_raw.copy_from_slice(raw);
        for w in writes {
            ws.trial_raw[w.channel_index as usize] = w.raw;
        }
        self.forward
            .decode_commands(&ws.trial_raw, &mut ws.commands)
            .unwrap();
        if pair.iter().any(|&i| ws.commands[i].velocity.is_some()) {
            out.status = PositionFitStatus::VelocityAuthority;
            return false;
        }
        if pair.iter().enumerate().any(|(j, &i)| {
            ws.commands[i]
                .absolute
                .is_none_or(|a| a.function_id != writes[j].function_id)
        }) {
            out.status = PositionFitStatus::ForwardMismatch;
            return false;
        }
        ws.trial_axes.copy_from_slice(&ws.axes);
        for &i in &pair {
            ws.trial_axes[i] = ws.commands[i].absolute_degrees();
        }
        self.forward
            .evaluate_pose(&ws.trial_axes, mount, &mut ws.geometry, &mut ws.poses)
            .unwrap();
        if ws.poses[index].world.is_none() {
            out.status = PositionFitStatus::UnsupportedGeometry;
            return false;
        }
        out.status = PositionFitStatus::Fitted;
        out.writes = writes.map(Some);
        true
    }
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| a[i] * b[i]).sum()
}
fn unit(v: [f64; 3]) -> Option<[f64; 3]> {
    let length = v[0].hypot(v[1]).hypot(v[2]);
    (length > 1e-9 && length.is_finite()).then(|| v.map(|a| a / length))
}
fn ray_error(pose: R, target: [f64; 3]) -> Option<f64> {
    let origin = pose.point([0.; 3]);
    let t = unit(std::array::from_fn(|i| target[i] - origin[i]))?;
    let d = unit(pose.direction([0., -1., 0.]))?;
    Some(dot(d, t).clamp(-1., 1.).acos().to_degrees())
}

fn quality_min(a: PhysicalDataQuality, b: PhysicalDataQuality) -> PhysicalDataQuality {
    let rank = |q| match q {
        PhysicalDataQuality::Unknown => 0,
        PhysicalDataQuality::Estimated => 1,
        PhysicalDataQuality::Manufacturer => 2,
        PhysicalDataQuality::Measured => 3,
    };
    if rank(a) < rank(b) { a } else { b }
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
