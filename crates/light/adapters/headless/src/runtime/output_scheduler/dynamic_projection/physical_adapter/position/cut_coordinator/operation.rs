//! Original Required/Size cuts with exact owner-local lexical replay. Producer correspondence and owner-local operands are
//! separate proofs; every speculative endpoint includes all mechanical owners and copies.
//! Nested and sequential authenticated graph cuts retain heap-owned drivers; only complete cohorts publish.
use super::*;
use light_dynamics::{
    DynamicControllerSizeRole, DynamicOperationCorrespondence, DynamicOperationHandle,
    DynamicOperationSite, DynamicSampleExpression, PositionEnvelopeLocator,
    PositionEnvelopeOperand, PositionGraphOperationKind, PositionGraphOperationLocator,
    PositionGraphOperationOperand, PositionGraphOperationOperandProgress, PositionMaskLocator,
    PositionMaskOperand, PositionResumeOperandLocator, PositionResumeOperandProgress,
    PositionStageOperandProgress,
};

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default)]
pub(in crate::runtime::output_scheduler::dynamic_projection::physical_adapter::position) struct AttemptEvidence
{
    /// Mechanical roots admitted past the coordinator's peer gate.
    pub roots: usize,
    pub parents: usize,
    pub endpoint_cohorts: usize,
    pub completed: usize,
    /// Completed changing copies re-issued at their one corresponding original site.
    pub synchronous: usize,
    /// Completed changing copies refused for zero or several corresponding reached sites.
    pub unmatched: usize,
    /// Original Resume operand cuts formed from owner-issued locators of one exact scope.
    pub resume_cuts: usize,
    /// Resume cuts initiated by a locator issued from a non-Full (operand) driver.
    pub resume_nested_cuts: usize,
    /// Resume cuts refused for an unissued owner whose original scope membership is Active.
    pub resume_active_unissued: usize,
}
#[cfg(test)]
thread_local! {
    static ATTEMPT_EVIDENCE: std::cell::Cell<Option<AttemptEvidence>> = const { std::cell::Cell::new(None) };
}
#[cfg(test)]
pub(in crate::runtime::output_scheduler::dynamic_projection::physical_adapter::position) fn inspect_attempt<
    T,
>(
    run: impl FnOnce() -> T,
) -> (T, AttemptEvidence) {
    struct Restore(Option<AttemptEvidence>);
    impl Drop for Restore {
        fn drop(&mut self) {
            ATTEMPT_EVIDENCE.with(|cell| cell.set(self.0));
        }
    }
    let previous = ATTEMPT_EVIDENCE.with(|cell| cell.replace(Some(AttemptEvidence::default())));
    let _restore = Restore(previous);
    let result = run();
    let evidence = ATTEMPT_EVIDENCE.with(|cell| cell.get().unwrap());
    (result, evidence)
}
#[cfg(test)]
pub(super) fn evidence(update: impl FnOnce(&mut AttemptEvidence)) {
    ATTEMPT_EVIDENCE.with(|cell| {
        if let Some(mut value) = cell.get() {
            update(&mut value);
            cell.set(Some(value));
        }
    });
}

#[derive(Clone, Copy)]
enum Operation {
    Required { progress: f32 },
    Size { factor: f32 },
}
impl Operation {
    fn endpoints(self) -> [PositionGraphOperationOperand; 2] {
        match self {
            Self::Required { .. } => [
                PositionGraphOperationOperand::RequiredOutgoing,
                PositionGraphOperationOperand::RequiredIncoming,
            ],
            Self::Size { .. } => [
                PositionGraphOperationOperand::SizeBaseline,
                PositionGraphOperationOperand::SizeValue,
            ],
        }
    }
    fn equals(self, other: Self) -> bool {
        match (self, other) {
            (Self::Required { progress: a }, Self::Required { progress: b }) => a == b,
            (Self::Size { factor: a }, Self::Size { factor: b }) => a == b,
            _ => false,
        }
    }
    fn same_kind(self, other: Self) -> bool {
        matches!(
            (self, other),
            (Self::Required { .. }, Self::Required { .. }) | (Self::Size { .. }, Self::Size { .. })
        )
    }
    fn apply(
        self,
        from: AttributeValue,
        to: AttributeValue,
    ) -> Result<AttributeValue, TransitionError> {
        let transition = CompiledProgrammingTransition::new(from, to, None)?;
        match self {
            Self::Required { progress } => transition.sample(progress),
            Self::Size { factor } => transition.scale(factor),
        }
    }
}
struct PendingOperation {
    locator: PositionGraphOperationLocator,
    handle: DynamicOperationHandle,
    operation: Operation,
}

/// Only the outstanding original request binds a graph boundary. A subtree provenance
/// census cannot select a descendant or replace its lexical prefix.
fn operation(
    peer: &Peer,
    request: &PositionCompositionRequest,
    locator: Option<PositionGraphOperationLocator>,
) -> Result<Option<PendingOperation>, TransitionError> {
    let PositionCompositionOperation::Base { request: base, .. } = &request.operation else {
        return Ok(None);
    };
    let mut leaf = base;
    let mut depth = 0;
    while let PositionCompositionBaseOperation::SourceCohort { request, .. } = &leaf.operation {
        depth += 1;
        if depth > MAX_NODES {
            return Ok(None);
        }
        leaf = request;
    }
    let operation = match &leaf.operation {
        PositionCompositionBaseOperation::Whole(inner) => match &inner.operation {
            FamilyMaterializationOperation::Transition {
                progress,
                reason: DynamicTransitionReason::Required { .. },
                ..
            } => Operation::Required {
                progress: *progress,
            },
            FamilyMaterializationOperation::Scale { factor, .. } => {
                Operation::Size { factor: *factor }
            }
            _ => return Ok(None),
        },
        PositionCompositionBaseOperation::Coupled(inner) => match &inner.operation {
            PositionMaterializationOperation::Transition {
                progress,
                reason: DynamicTransitionReason::Required { .. },
                ..
            } => Operation::Required {
                progress: *progress,
            },
            PositionMaterializationOperation::Scale { factor, .. } => {
                Operation::Size { factor: *factor }
            }
            _ => return Ok(None),
        },
        _ => return Ok(None),
    };
    let Some(locator) = locator else {
        return Ok(None);
    };
    let registry = peer.captured.registry();
    let Some(origin) = request.origin() else {
        return Ok(None);
    };
    if registry.source_node_for_origin(origin)?.as_ref() != Some(locator.operation_node())
        || site_operation(peer, &locator)?.is_none_or(|original| !original.equals(operation))
    {
        return Ok(None);
    }
    let Some(handle) = site_authority(peer, &locator, operation)? else {
        return Ok(None);
    };
    Ok(Some(PendingOperation {
        locator,
        handle,
        operation,
    }))
}

const MAX_ENVIRONMENTS: usize = 32;
const MAX_STEPS: usize = 2048;
#[cfg(test)]
thread_local! { static ENVIRONMENT_LIMIT: std::cell::Cell<usize> = const { std::cell::Cell::new(MAX_ENVIRONMENTS) }; }
#[cfg(test)]
pub(in crate::runtime::output_scheduler::dynamic_projection::physical_adapter::position) fn with_environment_limit<
    T,
>(
    limit: usize,
    run: impl FnOnce() -> T,
) -> T {
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

// Exact local route and operand role, in stable peer/copy order. We do not pair numeric
// values, pending UUIDs, ranks or graph paths between owners. All branches in this bounded
// executor are original captured branches; inherited Resume conditioning remains separate.
#[derive(Clone, Eq, Hash, PartialEq)]
enum Goal {
    Full,
    Graph(PositionGraphOperationLocator, PositionGraphOperationOperand),
    Envelope(PositionEnvelopeLocator, PositionEnvelopeOperand),
    Mask(PositionMaskLocator, PositionMaskOperand),
    Resume(PositionResumeOperandLocator, PositionResumeEndpoint),
    Constant,
}
mod local_stage;
mod resume_operand;
mod site_authority;
mod synchronous_peer;
use site_authority::{site_authority, site_operation};
#[cfg(test)]
pub(in crate::runtime::output_scheduler::dynamic_projection::physical_adapter::position) use synchronous_peer::{
    with_duplicated_reach, with_evaluation_limit,
};

enum Driver {
    Full(HybridPositionEvaluation),
    Graph(
        super::super::super::super::programming_projection::hybrid::HybridPositionGraphEvaluation,
    ),
    Stage(
        super::super::super::super::programming_projection::hybrid::HybridPositionStageEvaluation,
    ),
    Resume(
        super::super::super::super::programming_projection::hybrid::HybridPositionResumeEvaluation,
    ),
}
impl Driver {
    fn graph_locator(
        &self,
        request_id: uuid::Uuid,
    ) -> Result<Option<PositionGraphOperationLocator>, TransitionError> {
        match self {
            Self::Full(driver) => driver.pending_graph_operation_locator(request_id),
            Self::Graph(driver) => driver.pending_graph_operation_locator(request_id),
            Self::Stage(driver) => driver.pending_graph_operation_locator(request_id),
            Self::Resume(driver) => driver.pending_graph_operation_locator(request_id),
        }
    }
    fn mask_locator(
        &self,
        request_id: uuid::Uuid,
    ) -> Result<Option<PositionMaskLocator>, TransitionError> {
        match self {
            Self::Full(driver) => driver.pending_mask_locator(request_id),
            Self::Graph(driver) => driver.pending_mask_locator(request_id),
            Self::Stage(driver) => driver.pending_mask_locator(request_id),
            Self::Resume(driver) => driver.pending_mask_locator(request_id),
        }
    }
    fn envelope_locator(
        &self,
        request_id: uuid::Uuid,
    ) -> Result<Option<PositionEnvelopeLocator>, TransitionError> {
        match self {
            Self::Full(driver) => driver.pending_envelope_locator(request_id),
            Self::Graph(driver) => driver.pending_envelope_locator(request_id),
            Self::Stage(driver) => driver.pending_envelope_locator(request_id),
            Self::Resume(driver) => driver.pending_envelope_locator(request_id),
        }
    }
}
enum Calculation {
    Graph(Operation),
    Blend(f32),
    Adopt,
}
impl Calculation {
    fn apply(
        &self,
        from: AttributeValue,
        to: AttributeValue,
    ) -> Result<AttributeValue, TransitionError> {
        match self {
            Self::Graph(operation) => operation.apply(from, to),
            Self::Blend(progress) => {
                CompiledProgrammingTransition::new(from, to, None)?.sample(*progress)
            }
            Self::Adopt => Ok(from),
        }
    }
}
struct PendingCut {
    operands: [Goal; 2],
    calculation: Calculation,
}
struct OwnedWork {
    peer: usize,
    instance: usize,
    driver: Option<Driver>,
    request: Option<PositionCompositionRequest>,
    value: Option<AttributeValue>,
}
struct Waiting {
    operations: Vec<Option<PendingCut>>,
    children: [usize; 2],
}
struct Environment {
    goals: Vec<Goal>,
    // Only independently constant original programs can be frozen into both operands.
    constants: Vec<Option<AttributeValue>>,
    initialized: bool,
    work: Vec<OwnedWork>,
    waiting: Option<Waiting>,
    fitted: Option<Vec<PhysicalResolution<PositionAdapter>>>,
}
impl Environment {
    fn new(goals: Vec<Goal>, constants: Vec<Option<AttributeValue>>) -> Self {
        Self {
            goals,
            constants,
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
    if !eligible_operation_programs(peers, batch)? || environment_limit() == 0 {
        return Ok(None);
    }
    let count = peers
        .iter()
        .map(|peer| peer.descriptor.instances.len())
        .sum();
    let mut environments = vec![Environment::new(vec![Goal::Full; count], vec![None; count])];
    // A resolver unwind also recycles every retained parent/operand before propagating.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run(observer, frame, batch, peers, &mut environments)
    }));
    let mut cleanup = None;
    for environment in &mut environments {
        if let Err(error) = recycle_owned(batch, &mut environment.work) {
            if cleanup.is_none() {
                cleanup = Some(error);
            }
        }
    }
    let result = match result {
        Ok(result) => result,
        Err(panic) => std::panic::resume_unwind(panic),
    };
    if let Some(error) = cleanup {
        return Err(error);
    }
    match result {
        Err(TransitionError::Requires(_)) => Ok(None),
        other => other,
    }
}
fn recycle_owned(batch: &mut Batch<'_>, work: &mut Vec<OwnedWork>) -> Result<(), TransitionError> {
    let mut first = None;
    for mut item in work.drain(..) {
        let result = match item.driver.take() {
            Some(Driver::Full(driver)) => batch.recycle(driver),
            Some(Driver::Graph(driver)) => batch.recycle_graph(driver),
            Some(Driver::Stage(driver)) => batch.recycle_stage(driver),
            Some(Driver::Resume(driver)) => batch.recycle_resume(driver),
            None => Ok(()),
        };
        if let Err(error) = result {
            if first.is_none() {
                first = Some(error);
            }
        }
    }
    first.map_or(Ok(()), Err)
}
fn advance(
    observer: &PositionFrameObserver<'_>,
    frame: HybridFrameContext<'_>,
    batch: &mut Batch<'_>,
    peers: &[Peer],
    item: &mut OwnedWork,
) -> Result<bool, TransitionError> {
    let peer = &peers[item.peer];
    let bound = bound(observer, frame, peer, item.instance);
    let adoption =
        |original: &AttributeValue, address: &DynamicValueAddress| bound.adopt(original, address);
    item.request = None;
    item.value = None;
    match item
        .driver
        .as_mut()
        .ok_or_else(|| invalid("Position graph work lost its driver"))?
    {
        Driver::Full(driver) => match batch.advance(driver, &bound, &adoption)? {
            PositionCompositionProgress::Complete(value) => item.value = Some(value),
            PositionCompositionProgress::NeedsMaterialization(request) => {
                item.request = Some(request)
            }
        },
        Driver::Graph(driver) => match batch.advance_graph(driver, &bound, &adoption)? {
            PositionGraphOperationOperandProgress::OperandReady(value) => item.value = Some(value),
            PositionGraphOperationOperandProgress::NeedsMaterialization(request) => {
                item.request = Some(request)
            }
            PositionGraphOperationOperandProgress::Inactive => return Ok(false),
        },
        Driver::Stage(driver) => match batch.advance_stage(driver, &bound, &adoption)? {
            PositionStageOperandProgress::OperandReady(value) => item.value = Some(value),
            PositionStageOperandProgress::NeedsMaterialization(request) => {
                item.request = Some(request)
            }
            PositionStageOperandProgress::Inactive => return Ok(false),
        },
        Driver::Resume(driver) => match batch.advance_resume(driver, &bound, &adoption)? {
            PositionResumeOperandProgress::OperandReady(value) => item.value = Some(value),
            PositionResumeOperandProgress::NeedsMaterialization(request) => {
                item.request = Some(request)
            }
            // The original enclosing goal is inactive; never a Full fallback.
            PositionResumeOperandProgress::Inactive => return Ok(false),
        },
    }
    Ok(true)
}
/// Begin one exact original goal for one owner copy. A Constant goal owns no driver.
fn begin_goal(
    observer: &PositionFrameObserver<'_>,
    frame: HybridFrameContext<'_>,
    batch: &mut Batch<'_>,
    peer: &Peer,
    instance: usize,
    goal: &Goal,
) -> Result<Option<Driver>, TransitionError> {
    let bound = bound(observer, frame, peer, instance);
    let adoption =
        |original: &AttributeValue, address: &DynamicValueAddress| bound.adopt(original, address);
    let branch = peer.captured.registry().branch();
    let destination = peer.descriptor.instances[instance].destination;
    Ok(Some(match goal {
        Goal::Full => {
            Driver::Full(batch.begin_branch(&peer.captured, &branch, destination, &adoption)?)
        }
        Goal::Graph(locator, operand) => Driver::Graph(batch.begin_graph_branch(
            &peer.captured,
            &branch,
            locator,
            *operand,
            destination,
            &adoption,
        )?),
        Goal::Envelope(locator, operand) => Driver::Stage(batch.begin_envelope_branch(
            &peer.captured,
            &branch,
            locator,
            *operand,
            destination,
            &adoption,
        )?),
        Goal::Mask(locator, operand) => Driver::Stage(batch.begin_mask_branch(
            &peer.captured,
            &branch,
            locator,
            *operand,
            destination,
            &adoption,
        )?),
        Goal::Resume(locator, endpoint) => Driver::Resume(batch.begin_resume_branch(
            &peer.captured,
            &branch,
            locator,
            *endpoint,
            destination,
            &adoption,
        )?),
        Goal::Constant => return Ok(None),
    }))
}
fn initialize(
    observer: &PositionFrameObserver<'_>,
    frame: HybridFrameContext<'_>,
    batch: &mut Batch<'_>,
    peers: &[Peer],
    environment: &mut Environment,
    started: &mut usize,
) -> Result<bool, TransitionError> {
    let loans = environment
        .goals
        .iter()
        .filter(|goal| !matches!(goal, Goal::Constant))
        .count();
    if loans > synchronous_peer::evaluation_limit().saturating_sub(*started) {
        return Ok(false);
    }
    *started += loans;
    for (peer_index, peer) in peers.iter().enumerate() {
        for instance in 0..peer.descriptor.instances.len() {
            let index = environment.work.len();
            let goal = &environment.goals[index];
            let mut driver = begin_goal(observer, frame, batch, peer, instance, goal)?;
            if let Some(driver) = &mut driver {
                // Owner-local reached-site evidence for a possible synchronous changing peer.
                driver.enable_graph_discovery(synchronous_peer::DISCOVERY_ROUTES)?;
            }
            environment.work.push(OwnedWork {
                peer: peer_index,
                instance,
                driver,
                request: None,
                value: environment.constants[index].clone(),
            });
            let item = environment.work.last_mut().unwrap();
            if matches!(goal, Goal::Constant) {
                if !envelope::constant(peer, batch)?
                    || item
                        .value
                        .as_ref()
                        .is_none_or(|value| !envelope::literal(value))
                {
                    return Ok(false);
                }
            } else if !advance(observer, frame, batch, peers, item)? {
                return Ok(false);
            }
        }
    }
    environment.initialized = true;
    Ok(true)
}
fn pending_operations(
    environment: &Environment,
    peers: &[Peer],
    batch: &Batch<'_>,
    root_first: bool,
) -> Result<Option<Vec<Option<PendingCut>>>, TransitionError> {
    let Some(initiator) = environment.work.iter().find(|item| item.request.is_some()) else {
        return Ok(None);
    };
    let request = initiator.request.as_ref().unwrap();
    let driver = initiator.driver.as_ref().unwrap();
    if let Some(locator) = driver.mask_locator(request.request_id)? {
        return local_stage::mask(environment, initiator.peer, &locator);
    }
    if let Some(locator) = driver.envelope_locator(request.request_id)? {
        return local_stage::envelope(environment, initiator.peer, &locator);
    }
    if let Some(locator) = driver.resume_locator(request.request_id)? {
        return resume_operand::cut(environment, peers, batch, &locator, root_first);
    }
    let mut operations = Vec::new();
    let mut authority: Option<DynamicOperationHandle> = None;
    let mut kind: Option<Operation> = None;
    for item in &environment.work {
        let pending = if let Some(request) = &item.request {
            let locator = item
                .driver
                .as_ref()
                .unwrap()
                .graph_locator(request.request_id)?;
            let Some(pending) = operation(&peers[item.peer], request, locator)? else {
                return Ok(None);
            };
            if let Some(original) = &authority {
                if !matches!(
                    original.correspondence(&pending.handle),
                    DynamicOperationCorrespondence::Shared { .. }
                ) || !kind.unwrap().same_kind(pending.operation)
                {
                    return Ok(None);
                }
            } else {
                authority = Some(pending.handle.clone());
                kind = Some(pending.operation);
            }
            Some(PendingCut {
                operands: pending
                    .operation
                    .endpoints()
                    .map(|operand| Goal::Graph(pending.locator.clone(), operand)),
                calculation: Calculation::Graph(pending.operation),
            })
        } else {
            // A changing peer that completed synchronously is not an independently constant
            // endpoint. Pending-only locators cannot authorize its corresponding operands.
            if !envelope::constant(&peers[item.peer], batch)? {
                return Ok(None);
            }
            None
        };
        operations.push(pending);
    }
    Ok(authority.map(|_| operations))
}
fn run(
    observer: &mut PositionFrameObserver<'_>,
    frame: HybridFrameContext<'_>,
    batch: &mut Batch<'_>,
    peers: &[Peer],
    environments: &mut Vec<Environment>,
) -> Result<Option<Vec<OwnedHybridProjection<PhysicalHeadResult<PositionAdapter>>>>, TransitionError>
{
    let mut memo = std::collections::HashMap::new();
    memo.insert(environments[0].goals.clone(), 0usize);
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
            if !answer(observer, frame, batch, peers, environments, index)? {
                return Ok(None);
            }
            materialized = true;
            continue;
        }
        if environments[index]
            .work
            .iter()
            .any(|item| item.request.is_some())
        {
            // Completed changing peers expose their one corresponding original site first.
            if !synchronous_peer::materialize(
                observer,
                frame,
                batch,
                peers,
                &mut environments[index],
                &mut started,
            )? {
                return Ok(None);
            }
            let root_first = index == 0 && !materialized;
            let Some(operations) =
                pending_operations(&environments[index], peers, batch, root_first)?
            else {
                return Ok(None);
            };
            #[cfg(test)]
            evidence(|evidence| {
                evidence.parents += operations
                    .iter()
                    .filter(|operation| operation.is_some())
                    .count()
            });
            let mut children = [0; 2];
            for operand_index in 0..2 {
                let mut goals = environments[index].goals.clone();
                let mut constants = vec![None; goals.len()];
                for (offset, (pending, item)) in
                    operations.iter().zip(&environments[index].work).enumerate()
                {
                    if let Some(pending) = pending {
                        goals[offset] = pending.operands[operand_index].clone();
                    } else if envelope::constant(&peers[item.peer], batch)? && item.value.is_some()
                    {
                        goals[offset] = Goal::Constant;
                        constants[offset] = item.value.clone();
                    }
                }
                children[operand_index] = if let Some(&known) = memo.get(&goals) {
                    known
                } else {
                    if environments.len() >= environment_limit() {
                        return Ok(None);
                    }
                    let next = environments.len();
                    memo.insert(goals.clone(), next);
                    environments.push(Environment::new(goals, constants));
                    next
                };
            }
            environments[index].waiting = Some(Waiting {
                operations,
                children,
            });
            continue;
        }
        if index == 0 {
            return finish_root(observer, batch, peers, &mut environments[0], materialized);
        }
        let values = environments[index]
            .work
            .iter()
            .map(|item| {
                item.value
                    .clone()
                    .ok_or_else(|| invalid("Position graph child omitted completed copy"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if values.iter().any(|value| !envelope::literal(value)) {
            return Ok(None);
        }
        let members = environments[index]
            .work
            .iter()
            .map(|item| (item.peer, item.instance))
            .collect::<Vec<_>>();
        let Some(fitted) = envelope::fit_members(observer, frame, peers, &members, &values)? else {
            return Ok(None);
        };
        environments[index].fitted = Some(fitted);
        recycle_owned(batch, &mut environments[index].work)?;
        #[cfg(test)]
        evidence(|evidence| evidence.endpoint_cohorts += 1);
        stack.pop();
    }
    Ok(None)
}
/// Both child cohorts are fitted: answer each suspended parent with its own calculation.
fn answer(
    observer: &PositionFrameObserver<'_>,
    frame: HybridFrameContext<'_>,
    batch: &mut Batch<'_>,
    peers: &[Peer],
    environments: &mut [Environment],
    index: usize,
) -> Result<bool, TransitionError> {
    let waiting = environments[index].waiting.take().unwrap();
    let mut responses = Vec::new();
    for (offset, pending) in waiting.operations.iter().enumerate() {
        let Some(pending) = pending else {
            continue;
        };
        let item = &environments[index].work[offset];
        let destination = peers[item.peer].descriptor.instances[item.instance].destination;
        let (Some(from), Some(to)) = (
            fitted_pair(
                &environments[waiting.children[0]].fitted.as_ref().unwrap()[item.peer],
                destination,
            ),
            fitted_pair(
                &environments[waiting.children[1]].fitted.as_ref().unwrap()[item.peer],
                destination,
            ),
        ) else {
            return Ok(false);
        };
        responses.push((offset, pending.calculation.apply(from, to)?));
    }
    // Each parent's actual factor/progress and exact UUID survive both child fits.
    // A following cut simply re-enters this loop with the same retained driver.
    for (offset, value) in responses {
        let item = &mut environments[index].work[offset];
        let request = item
            .request
            .as_ref()
            .ok_or_else(|| invalid("Position graph cut lost its suspended request"))?;
        match item.driver.as_mut().unwrap() {
            Driver::Full(driver) => batch.resume(driver, request.request_id, value, None)?,
            Driver::Graph(driver) => batch.resume_graph(driver, request.request_id, value, None)?,
            Driver::Stage(driver) => batch.resume_stage(driver, request.request_id, value, None)?,
            Driver::Resume(driver) => {
                batch.resume_resume(driver, request.request_id, value, None)?
            }
        }
        if !advance(observer, frame, batch, peers, item)? {
            return Ok(false);
        }
    }
    Ok(true)
}
/// Only completed original root Full evaluations are observed and may publish.
fn finish_root(
    observer: &mut PositionFrameObserver<'_>,
    batch: &mut Batch<'_>,
    peers: &[Peer],
    root: &mut Environment,
    materialized: bool,
) -> Result<Option<Vec<OwnedHybridProjection<PhysicalHeadResult<PositionAdapter>>>>, TransitionError>
{
    if !materialized {
        return Ok(None);
    }
    let mut work = Vec::new();
    for item in &mut root.work {
        let Some(Driver::Full(driver)) = item.driver.take() else {
            return Err(invalid("Position root lost original parent evaluation"));
        };
        work.push(Work {
            peer: item.peer,
            instance: item.instance,
            evaluation: Some(driver),
            request: None,
        });
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        observe_completed(observer, batch, peers, &mut work)
    }));
    let cleanup = recycle(batch, &mut work);
    let result = match result {
        Ok(result) => result,
        Err(panic) => std::panic::resume_unwind(panic),
    };
    cleanup?;
    #[cfg(test)]
    if result.is_ok() {
        evidence(|evidence| evidence.completed += 1);
    }
    result.map(Some)
}

// Keep Resume eligibility unchanged; only this proven graph replay path accepts multiple sources.
fn eligible_operation_programs(
    peers: &[Peer],
    _batch: &Batch<'_>,
) -> Result<bool, TransitionError> {
    let mut visited_count = 0;
    for peer in peers {
        // Count original slots as well as traversed nodes. Suppression must not turn a
        // materialized or released many-source stack into an unbounded preparation loan.
        if peer.requested.samples.len() > MAX_NODES.saturating_sub(visited_count) {
            return Ok(false);
        }
        visited_count += peer.requested.samples.len();
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
