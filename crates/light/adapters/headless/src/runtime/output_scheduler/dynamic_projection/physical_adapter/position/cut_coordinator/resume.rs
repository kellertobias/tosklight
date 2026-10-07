//! Heap-owned speculative environments. A child adds an original scoped choice; it never
//! recomputes a producer, relabels an occurrence or accepts physical continuity.
use super::*;
use light_dynamics::{PositionProgramBranch, PositionSourceNode};
use std::collections::HashMap;

const MAX_ENVIRONMENTS: usize = 32;
const MAX_STEPS: usize = 2048;
#[cfg(test)]
thread_local! { static ENVIRONMENT_LIMIT: std::cell::Cell<usize> = const { std::cell::Cell::new(MAX_ENVIRONMENTS) }; }
#[cfg(test)]
pub(super) fn with_environment_limit<T>(limit: usize, run: impl FnOnce() -> T) -> T {
    struct Restore(usize);
    impl Drop for Restore {
        fn drop(&mut self) {
            ENVIRONMENT_LIMIT.with(|value| value.set(self.0));
        }
    }
    let old = ENVIRONMENT_LIMIT.with(|value| value.replace(limit));
    let _restore = Restore(old);
    run()
}
fn environment_limit() -> usize {
    #[cfg(test)]
    {
        ENVIRONMENT_LIMIT.with(|value| value.get().min(MAX_ENVIRONMENTS))
    }
    #[cfg(not(test))]
    {
        MAX_ENVIRONMENTS
    }
}

type Choice = (PositionResumeScope, PositionResumeEndpoint);
#[derive(Clone, Eq, Hash, PartialEq)]
struct EnvironmentKey {
    choices: Vec<Choice>,
    // Preserve original owner-local cut identity and order, independently of endpoint choices.
    boundaries: Vec<Vec<PositionSourceNode>>,
}
struct Waiting {
    work: usize,
    witness: Witness,
    children: [usize; 2],
}
struct Environment {
    choices: Vec<Choice>,
    branches: Vec<PositionProgramBranch>,
    boundaries: Vec<Vec<PositionSourceNode>>,
    initialized: bool,
    work: Vec<Work>,
    waiting: Option<Waiting>,
    fitted: Option<Vec<PhysicalResolution<PositionAdapter>>>,
}
impl Environment {
    fn new(choices: Vec<Choice>, branches: Vec<PositionProgramBranch>) -> Self {
        let boundaries = branches.iter().map(|_| Vec::new()).collect();
        Self {
            choices,
            branches,
            boundaries,
            initialized: false,
            work: Vec::new(),
            waiting: None,
            fitted: None,
        }
    }
}

pub(super) fn attempt(
    observer: &mut PositionFrameObserver<'_>,
    frame: HybridFrameContext<'_>,
    batch: &mut Batch<'_>,
    peers: &[Peer],
) -> Result<Option<Vec<OwnedHybridProjection<PhysicalHeadResult<PositionAdapter>>>>, TransitionError>
{
    let mut environments = vec![Environment::new(
        Vec::new(),
        peers
            .iter()
            .map(|peer| peer.captured.registry().branch())
            .collect(),
    )];
    let result = run(observer, frame, batch, peers, &mut environments);
    let mut cleanup = None;
    for environment in &mut environments {
        if let Err(error) = recycle(batch, &mut environment.work) {
            if cleanup.is_none() {
                cleanup = Some(error);
            }
        }
    }
    if let Some(error) = cleanup {
        return Err(error);
    }
    match result {
        Err(TransitionError::Requires(_)) => Ok(None),
        other => other,
    }
}

fn initialize(
    observer: &PositionFrameObserver<'_>,
    frame: HybridFrameContext<'_>,
    batch: &mut Batch<'_>,
    peers: &[Peer],
    environment: &mut Environment,
    started: &mut usize,
) -> Result<bool, TransitionError> {
    let count = peers
        .iter()
        .map(|peer| peer.descriptor.instances.len())
        .sum::<usize>();
    if count > MAX_EVALUATIONS.saturating_sub(*started) {
        return Ok(false);
    }
    *started += count;
    for (peer_index, peer) in peers.iter().enumerate() {
        for instance in 0..peer.descriptor.instances.len() {
            let bound = bound(observer, frame, peer, instance);
            let adoption = |original: &AttributeValue, address: &DynamicValueAddress| {
                bound.adopt(original, address)
            };
            let evaluation = batch.begin_branch(
                &peer.captured,
                &environment.branches[peer_index],
                peer.descriptor.instances[instance].destination,
                &adoption,
            )?;
            environment.work.push(Work {
                peer: peer_index,
                instance,
                evaluation: Some(evaluation),
                request: None,
            });
            advance(
                observer,
                frame,
                batch,
                peers,
                environment.work.last_mut().unwrap(),
            )?;
        }
    }
    environment.initialized = true;
    Ok(true)
}
fn advance(
    observer: &PositionFrameObserver<'_>,
    frame: HybridFrameContext<'_>,
    batch: &mut Batch<'_>,
    peers: &[Peer],
    item: &mut Work,
) -> Result<(), TransitionError> {
    let bound = bound(observer, frame, &peers[item.peer], item.instance);
    let adoption =
        |original: &AttributeValue, address: &DynamicValueAddress| bound.adopt(original, address);
    item.request = match batch.advance(item.evaluation.as_mut().unwrap(), &bound, &adoption)? {
        PositionCompositionProgress::Complete(_) => None,
        PositionCompositionProgress::NeedsMaterialization(request) => Some(request),
    };
    Ok(())
}

fn child(
    environment: &Environment,
    peers: &[Peer],
    witness: &Witness,
    endpoint: PositionResumeEndpoint,
) -> Result<Option<(EnvironmentKey, Vec<PositionProgramBranch>)>, TransitionError> {
    // Every dependency adds a new scope. Re-entering a chosen scope would be a cycle or
    // stale local operation, never authority to substitute an arbitrary numeric operand.
    if environment
        .choices
        .iter()
        .any(|(scope, _)| *scope == witness.scope)
    {
        return Ok(None);
    }
    let mut choices = environment.choices.clone();
    choices.push((witness.scope, endpoint));
    choices.sort_by_key(|(scope, _)| {
        (
            scope.instance_id.as_u128(),
            scope.controller_id.as_u128(),
            scope.occurrence_id.as_u128(),
        )
    });
    let mut branches = environment.branches.clone();
    let mut boundaries = environment.boundaries.clone();
    let mut participating = false;
    for (index, (peer, branch)) in peers.iter().zip(&mut branches).enumerate() {
        // These exact captured gates remove every source before Base evaluation. Preserve
        // the original stack and baseline; hidden scopes gain no endpoint-choice authority.
        if peer.baseline_only {
            continue;
        }
        match branch.resume_scope_membership(witness.scope)? {
            PositionResumeScopeMembership::Active(nodes) => {
                for node in &nodes {
                    if !matches!(peer.captured.registry().node_view(node)?, PositionSourceNodeView::Transition { progress, .. } if progress == witness.progress)
                    {
                        return Ok(None);
                    }
                }
                let Some(view) = branch.at_resume_operand(witness.scope, endpoint)? else {
                    return Ok(None);
                };
                boundaries[index] = view.operand_boundary_nodes()?;
                *branch = view;
                participating = true;
            }
            // An unrelated peer retains its whole original program. A captured scope
            // removed by inherited choices remains inactive; it is never revived.
            PositionResumeScopeMembership::Absent | PositionResumeScopeMembership::Inactive => {}
        }
    }
    Ok(participating.then_some((
        EnvironmentKey {
            choices,
            boundaries,
        },
        branches,
    )))
}

fn run(
    observer: &mut PositionFrameObserver<'_>,
    frame: HybridFrameContext<'_>,
    batch: &mut Batch<'_>,
    peers: &[Peer],
    environments: &mut Vec<Environment>,
) -> Result<Option<Vec<OwnedHybridProjection<PhysicalHeadResult<PositionAdapter>>>>, TransitionError>
{
    let mut memo = HashMap::new();
    memo.insert(
        EnvironmentKey {
            choices: Vec::new(),
            boundaries: environments[0].boundaries.clone(),
        },
        0usize,
    );
    let mut stack = vec![0usize];
    let mut started = 0;
    let mut materialized = false;
    for _ in 0..MAX_STEPS {
        let Some(&index) = stack.last() else {
            return Ok(None);
        };
        if !environments[index].initialized
            && !initialize(
                observer,
                frame,
                batch,
                peers,
                &mut environments[index],
                &mut started,
            )?
        {
            return Ok(None);
        }
        if let Some(waiting) = &environments[index].waiting {
            if let Some(&child) = waiting
                .children
                .iter()
                .find(|&&child| environments[child].fitted.is_none())
            {
                if stack.contains(&child) {
                    return Ok(None);
                }
                stack.push(child);
                continue;
            }
            let waiting = environments[index].waiting.take().unwrap();
            let item = &environments[index].work[waiting.work];
            let destination = peers[item.peer].descriptor.instances[item.instance].destination;
            let from = fitted_pair(
                &environments[waiting.children[0]].fitted.as_ref().unwrap()[item.peer],
                destination,
            );
            let to = fitted_pair(
                &environments[waiting.children[1]].fitted.as_ref().unwrap()[item.peer],
                destination,
            );
            let (Some(from), Some(to)) = (from, to) else {
                return Ok(None);
            };
            let value = CompiledProgrammingTransition::new(from, to, None)?
                .sample(waiting.witness.progress)?;
            let item = &mut environments[index].work[waiting.work];
            let request = item
                .request
                .as_ref()
                .ok_or_else(|| invalid("Position cut lost its suspended parent"))?;
            // Nonlinear fitting has no proven Target field transfer. Keep it unknown.
            batch.resume(
                item.evaluation.as_mut().unwrap(),
                request.request_id,
                value,
                None,
            )?;
            advance(observer, frame, batch, peers, item)?;
            materialized = true;
            continue;
        }
        if let Some(work_index) = environments[index]
            .work
            .iter()
            .position(|item| item.request.is_some())
        {
            let item = &environments[index].work[work_index];
            let Some(witness) = request_witness(
                &peers[item.peer],
                &environments[index].branches[item.peer],
                item.request.as_ref().unwrap(),
            )?
            else {
                return Ok(None);
            };
            let mut children = [0; 2];
            for (offset, endpoint) in [
                PositionResumeEndpoint::Outgoing,
                PositionResumeEndpoint::Incoming,
            ]
            .into_iter()
            .enumerate()
            {
                let Some((key, branches)) = child(&environments[index], peers, &witness, endpoint)?
                else {
                    return Ok(None);
                };
                children[offset] = if let Some(&known) = memo.get(&key) {
                    known
                } else {
                    if environments.len() >= environment_limit() {
                        return Ok(None);
                    }
                    let next = environments.len();
                    memo.insert(key.clone(), next);
                    let mut environment = Environment::new(key.choices, branches);
                    environment.boundaries = key.boundaries;
                    environments.push(environment);
                    next
                };
            }
            environments[index].waiting = Some(Waiting {
                work: work_index,
                witness,
                children,
            });
            continue;
        }
        if index == 0 {
            if !materialized {
                return Ok(None);
            }
            return observe_completed(observer, batch, peers, &mut environments[0].work).map(Some);
        }
        let Some(fitted) = fit_complete(observer, frame, peers, &environments[index].work)? else {
            return Ok(None);
        };
        environments[index].fitted = Some(fitted);
        recycle(batch, &mut environments[index].work)?;
        stack.pop();
    }
    Ok(None)
}

fn fit_complete(
    observer: &PositionFrameObserver<'_>,
    frame: HybridFrameContext<'_>,
    peers: &[Peer],
    work: &[Work],
) -> Result<Option<Vec<PhysicalResolution<PositionAdapter>>>, TransitionError> {
    let mut programs = Vec::new();
    for (peer_index, peer) in peers.iter().enumerate() {
        let mut destinations = Vec::new();
        for item in work.iter().filter(|item| item.peer == peer_index) {
            let value = item
                .evaluation
                .as_ref()
                .and_then(|evaluation| evaluation.completed_value())
                .ok_or_else(|| invalid("Position child cut omitted a completed physical copy"))?
                .clone();
            let fields = ProgrammingFieldScope::for_value(ProgrammingOwner::Position, &value)?;
            destinations.push(PositionProgramDestination {
                destination: peer.descriptor.instances[item.instance].destination,
                value,
                provenance: PhysicalProvenance {
                    fields,
                    sources: Default::default(),
                    controls: None,
                },
            });
        }
        if destinations.len() != peer.descriptor.instances.len() {
            return Err(invalid("Position child cut has incomplete copy membership"));
        }
        programs.push((peer.target, destinations));
    }
    let requests = peers
        .iter()
        .map(|peer| PhysicalRequest {
            frame,
            target: peer.target,
            owner: ProgrammingOwner::Position,
            descriptor: peer.descriptor.as_ref(),
            value: &peer.requested.base,
            previous: peer.previous.as_ref(),
        })
        .collect::<Vec<_>>();
    let views = programs
        .iter()
        .map(|(target, destinations)| (*target, destinations.as_slice()))
        .collect::<Vec<_>>();
    let resolved = observer
        .lane
        .adapter()
        .resolve_cohort_programs(&requests, &[], &views)?;
    if resolved.len() != peers.len()
        || resolved.iter().zip(peers).any(|(resolution, peer)| {
            peer.descriptor
                .instances
                .iter()
                .any(|instance| fitted_pair(resolution, instance.destination).is_none())
        })
    {
        return Ok(None);
    }
    Ok(Some(resolved))
}
