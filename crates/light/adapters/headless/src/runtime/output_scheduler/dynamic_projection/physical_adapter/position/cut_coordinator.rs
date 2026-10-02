//! Captured shared Resume cuts retain original peer/copy continuations. Bounded child
//! environments resolve nested scopes without sampling producers or accepting speculative output.
use super::super::super::programming_projection::hybrid::{
    HybridCapturedPositionProgram, HybridPositionBatchComposer, HybridPositionBatchResult,
    HybridPositionEvaluation,
};
use super::*;
use light_core::programming::CompiledProgrammingTransition;
use light_dynamics::{
    DynamicTransitionReason, FamilyCompositionSample, FamilyMaterializationOperation,
    PositionCompositionBaseOperation, PositionCompositionOperation, PositionCompositionProgress,
    PositionCompositionRequest, PositionMaterializationOperation, PositionResumeEndpoint,
    PositionResumeScope, PositionResumeScopeMembership, PositionSourceNodeView,
};

// Bound original traversal, owned evaluations and speculative environments. Exhaustion
// preserves the existing passive path without publishing a partial mechanical cohort.
const MAX_PEERS: usize = 16;
const MAX_COPIES: usize = 16;
const MAX_NODES: usize = 512;
const MAX_EVALUATIONS: usize = 256;

mod envelope;
mod mask;
pub(super) mod operation;
mod resume;
#[cfg(test)]
pub(super) fn with_environment_limit<T>(limit: usize, run: impl FnOnce() -> T) -> T {
    resume::with_environment_limit(limit, run)
}

type Batch<'a> = dyn HybridPositionBatchComposer<PhysicalHeadResult<PositionAdapter>> + 'a;
struct Peer {
    target: FixtureId,
    requested: Arc<PositionProgram>,
    captured: Arc<HybridCapturedPositionProgram>,
    descriptor: Arc<PositionDescriptor>,
    previous: Option<PositionContinuity>,
    // Exact captured endpoint gates remove this whole original stack from evaluation.
    // It still contributes its static baseline to every complete physical cohort.
    baseline_only: bool,
}
struct Work {
    peer: usize,
    instance: usize,
    evaluation: Option<HybridPositionEvaluation>,
    request: Option<PositionCompositionRequest>,
}
#[derive(Clone)]
struct Witness {
    scope: PositionResumeScope,
    progress: f32,
}

/// Returns only complete roots. Unsupported roots continue through the original guarded
/// per-group composer; no speculative row or accepted continuity escapes a failed attempt.
pub(super) fn compose(
    observer: &mut PositionFrameObserver<'_>,
    frame: HybridFrameContext<'_>,
    batch: &mut Batch<'_>,
) -> Result<Option<HybridPositionBatchResult<PhysicalHeadResult<PositionAdapter>>>, TransitionError>
{
    let eligible = batch
        .eligible_targets()
        .into_iter()
        .collect::<rustc_hash::FxHashSet<_>>();
    let mut roots = rustc_hash::FxHashSet::default();
    let mut candidates = Vec::new();
    for program in &observer.programs {
        if !eligible.contains(&program.target) {
            continue;
        }
        let descriptor =
            match observer
                .lane
                .descriptor(frame, program.target, ProgrammingOwner::Position)
            {
                Ok(descriptor) => descriptor,
                Err(TransitionError::Requires(_)) => continue,
                Err(error) => return Err(error),
            };
        if roots.insert(descriptor.root) {
            candidates.push(descriptor.root);
        }
    }
    let mut output = HybridPositionBatchResult {
        handled: Vec::new(),
        projections: Vec::new(),
        requirements: Vec::new(),
    };
    for root in candidates {
        let Some(members) = observer.current.complete_members(frame, root)? else {
            continue;
        };
        if members.len() > MAX_PEERS || members.iter().any(|target| !eligible.contains(target)) {
            continue;
        }
        let mut peers = Vec::new();
        for target in members {
            let Some(program) = observer
                .programs
                .iter()
                .find(|program| program.target == target)
            else {
                peers.clear();
                break;
            };
            if program.has_requirements {
                peers.clear();
                break;
            }
            let descriptor = observer
                .lane
                .descriptor(frame, target, ProgrammingOwner::Position)?;
            let baseline_only = batch.original_sources_suppressed(target)?;
            peers.push(Peer {
                target,
                baseline_only,
                requested: Arc::clone(&program.requested),
                captured: Arc::clone(&program.captured),
                descriptor,
                previous: observer.lane.continuity(target, ProgrammingOwner::Position),
            });
        }
        if peers.len() < 2
            || peers
                .iter()
                .filter(|peer| !peer.requested.samples.is_empty())
                .count()
                < 2
            || peers
                .iter()
                .any(|peer| peer.descriptor.instances.len() > MAX_COPIES)
            || peers
                .iter()
                .map(|peer| peer.descriptor.instances.len())
                .sum::<usize>()
                > MAX_EVALUATIONS / 3
        {
            continue;
        }
        #[cfg(test)]
        operation::evidence(|evidence| evidence.roots += 1);
        if let Some(rows) = envelope::attempt(observer, frame, batch, &peers)? {
            output.handled.extend(peers.iter().map(|peer| peer.target));
            output.projections.extend(rows);
            continue;
        }
        if let Some(rows) = mask::attempt(observer, frame, batch, &peers)? {
            output.handled.extend(peers.iter().map(|peer| peer.target));
            output.projections.extend(rows);
            continue;
        }
        if let Some(rows) = operation::attempt(observer, frame, batch, &peers)? {
            output.handled.extend(peers.iter().map(|peer| peer.target));
            output.projections.extend(rows);
            continue;
        }
        if !eligible_programs(&peers, batch)? {
            continue;
        }
        if let Some(rows) = resume::attempt(observer, frame, batch, &peers)? {
            output.handled.extend(peers.iter().map(|peer| peer.target));
            output.projections.extend(rows);
        }
    }
    Ok((!output.handled.is_empty()).then_some(output))
}

fn eligible_programs(peers: &[Peer], batch: &Batch<'_>) -> Result<bool, TransitionError> {
    let mut visited_count = 0;
    for peer in peers {
        // Count original slots as well as traversed nodes. Suppression must not turn a
        // materialized or released many-source stack into an unbounded preparation loan.
        if peer.requested.samples.len() > MAX_NODES.saturating_sub(visited_count) {
            return Ok(false);
        }
        visited_count += peer.requested.samples.len();
        if !peer.baseline_only
            && (peer.requested.samples.len() > 1
                || !batch.noninterpolating_controls(&peer.captured)?)
        {
            return Ok(false);
        }
        for sample in peer
            .requested
            .samples
            .iter()
            .filter(|_| !peer.baseline_only)
        {
            let activation = match sample {
                FamilyCompositionSample::Known(sample) => {
                    if sample.address().address().component.is_some()
                        || sample.materialized_value().is_none()
                    {
                        return Ok(false);
                    }
                    if sample.is_fix_at() {
                        // One complete fully active mask is an independently constant peer.
                        // It contributes its original program to every child environment; it
                        // does not acquire the changing owner's Resume or a shared mask scope.
                        let constant = match sample.materialized_value() {
                            Some(light_dynamics::DynamicValue::Family(
                                AttributeValue::Position(value),
                            )) => match value.as_ref() {
                                PositionIntent::Angles {
                                    pan_degrees: ScalarIntent::Value(_),
                                    tilt_degrees: ScalarIntent::Value(_),
                                } => true,
                                PositionIntent::Target { offset_metres, .. } => offset_metres
                                    .iter()
                                    .all(|value| matches!(value, ScalarIntent::Value(_))),
                                _ => false,
                            },
                            _ => false,
                        };
                        if !constant {
                            return Ok(false);
                        }
                    }
                    sample.activation_mix
                }
                FamilyCompositionSample::WholeExpression { activation_mix, .. }
                | FamilyCompositionSample::CoupledExpression { activation_mix, .. } => {
                    *activation_mix
                }
            };
            if activation != 1. {
                return Ok(false);
            }
        }
        let registry = peer.captured.registry();
        let mut pending = (0..registry.source_count())
            .map(|index| registry.source_root(index))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let mut visited = std::collections::HashSet::new();
        while let Some(node) = pending.pop() {
            if !visited.insert(node.clone()) {
                continue;
            }
            visited_count += 1;
            if visited_count > MAX_NODES {
                return Ok(false);
            }
            match registry.node_view(&node)? {
                PositionSourceNodeView::Transition { from, to, .. } => {
                    pending.extend(from);
                    pending.extend(to);
                }
                PositionSourceNodeView::Whole { root, .. } => pending.push(root),
                PositionSourceNodeView::SourceCohort { members } => pending.extend(members),
                PositionSourceNodeView::Scale { value, .. } => pending.extend(value),
                PositionSourceNodeView::Leaf => {}
            }
        }
    }
    Ok(true)
}

fn bound<'a>(
    observer: &'a PositionFrameObserver<'_>,
    frame: HybridFrameContext<'a>,
    peer: &'a Peer,
    instance: usize,
) -> destination::PositionDestinationFrame<'a> {
    destination::PositionDestinationFrame {
        adapter: observer.lane.adapter(),
        frame,
        descriptor: &peer.descriptor,
        target: peer.target,
        instance: &peer.descriptor.instances[instance],
        previous: peer.previous.as_ref(),
        current: &observer.current,
        active_programs: &observer.active_programs,
    }
}
fn recycle(batch: &mut Batch<'_>, work: &mut Vec<Work>) -> Result<(), TransitionError> {
    let mut first = None;
    for item in work.drain(..) {
        if let Some(evaluation) = item.evaluation {
            if let Err(error) = batch.recycle(evaluation) {
                if first.is_none() {
                    first = Some(error);
                }
            }
        }
    }
    first.map_or(Ok(()), Err)
}
fn observe_completed(
    observer: &mut PositionFrameObserver<'_>,
    batch: &mut Batch<'_>,
    peers: &[Peer],
    work: &mut [Work],
) -> Result<Vec<OwnedHybridProjection<PhysicalHeadResult<PositionAdapter>>>, TransitionError> {
    let mut rows = Vec::new();
    let mut pending_rows = Vec::new();
    for (peer_index, peer) in peers.iter().enumerate() {
        let mut representative = None;
        let mut destinations = Vec::new();
        for item in work.iter_mut().filter(|item| item.peer == peer_index) {
            let pending_start = observer.pending.len();
            let result = batch.observe(item.evaluation.as_mut().unwrap(), &mut |observation| {
                observer.observe(observation)
            });
            observer.pending.truncate(pending_start);
            let row = result?;
            destinations.push(PositionProgramDestination {
                destination: peer.descriptor.instances[item.instance].destination,
                value: row.value.clone(),
                provenance: row.sidecar.provenance.clone(),
            });
            if representative.is_none() {
                representative = Some(row);
            }
        }
        let mut row =
            representative.ok_or_else(|| invalid("complete Position cut omitted its owner"))?;
        let program = (!peer.requested.samples.is_empty()).then(|| Arc::clone(&peer.requested));
        if let Some(program) = &program {
            row.sidecar.requested = PositionRequest::Program(Arc::clone(program));
        }
        pending_rows.push(PendingPosition {
            target: peer.target,
            descriptor: Arc::clone(&peer.descriptor),
            previous: peer.previous.clone(),
            program,
            destinations,
        });
        rows.push(row);
    }
    observer.pending.extend(pending_rows);
    Ok(rows)
}
fn request_witness(
    peer: &Peer,
    branch: &light_dynamics::PositionProgramBranch,
    request: &PositionCompositionRequest,
) -> Result<Option<Witness>, TransitionError> {
    let PositionCompositionOperation::Base { request: base, .. } = &request.operation else {
        return Ok(None);
    };
    let (progress, reason) = match &base.operation {
        PositionCompositionBaseOperation::Coupled(inner) => match &inner.operation {
            PositionMaterializationOperation::Transition {
                progress, reason, ..
            } => (*progress, *reason),
            _ => return Ok(None),
        },
        PositionCompositionBaseOperation::Whole(inner) => match &inner.operation {
            FamilyMaterializationOperation::Transition {
                progress, reason, ..
            } => (*progress, *reason),
            _ => return Ok(None),
        },
        _ => return Ok(None),
    };
    let DynamicTransitionReason::Resume { occurrence_id } = reason else {
        return Ok(None);
    };
    let Some(origin) = request.origin() else {
        return Ok(None);
    };
    let Some(node) = peer.captured.registry().source_node_for_origin(origin)? else {
        return Ok(None);
    };
    let Some(scope) = peer.captured.registry().resume_scope(&node)? else {
        return Ok(None);
    };
    if occurrence_id != scope.occurrence_id
        || !matches!(peer.captured.registry().node_view(&node)?, PositionSourceNodeView::Transition { progress: original, .. } if original == progress)
        || !matches!(branch.resume_scope_membership(scope)?, PositionResumeScopeMembership::Active(nodes) if nodes.contains(&node))
    {
        return Ok(None);
    }
    Ok(Some(Witness { scope, progress }))
}

fn fitted_pair(
    resolution: &PhysicalResolution<PositionAdapter>,
    destination: FixtureId,
) -> Option<AttributeValue> {
    let mut outcomes = resolution
        .achieved
        .outcomes
        .iter()
        .filter(|outcome| outcome.destination == destination);
    let first = outcomes.next()?;
    let angles = first.result.achieved?;
    let usable = |outcome: &PositionOutcome| {
        !outcome.missing_mount
            && !outcome.input_requirement
            && outcome.result.status == PositionFitStatus::Fitted
            && outcome
                .result
                .achieved
                .is_some_and(|pair| pair.iter().zip(angles).all(|(a, b)| (a - b).abs() <= 1e-5))
    };
    if !usable(first) || !outcomes.all(usable) {
        return None;
    }
    Some(AttributeValue::Position(Arc::new(PositionIntent::angles(
        angles[0] as f32,
        angles[1] as f32,
    ))))
}
