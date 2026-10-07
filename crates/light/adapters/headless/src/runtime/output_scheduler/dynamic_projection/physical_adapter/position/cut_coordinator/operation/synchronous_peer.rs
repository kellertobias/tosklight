//! Synchronous changing peers. A completed owner whose corresponding original Required/Size
//! finished inline is not a constant, and its completed parent value is not the operand. Its
//! one exact reached site is re-issued as a request of a fresh replay of the same captured goal,
//! so the ordinary cut fits it with every mechanical peer/copy and its suffix resumes once.
//! Reports alone grant nothing: site authority plus the issued materialized request does.
use super::*;
use light_dynamics::{PositionGraphDiscoveryReport, PositionGraphMaterializationStatus};

/// Bound on reached Required/Size routes recorded per original driver.
pub(super) const DISCOVERY_ROUTES: usize = MAX_NODES * 4;

#[cfg(test)]
thread_local! {
    static EVALUATION_LIMIT: std::cell::Cell<usize> = const { std::cell::Cell::new(MAX_EVALUATIONS) };
    static DUPLICATE_REACH: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
/// Lower the per-attempt evaluation loan budget (initial goals plus restarts) for one call.
#[cfg(test)]
pub(in crate::runtime::output_scheduler::dynamic_projection::physical_adapter::position) fn with_evaluation_limit<
    T,
>(
    limit: usize,
    run: impl FnOnce() -> T,
) -> T {
    struct Restore(usize);
    impl Drop for Restore {
        fn drop(&mut self) {
            EVALUATION_LIMIT.with(|value| value.set(self.0));
        }
    }
    let _restore = Restore(EVALUATION_LIMIT.with(|value| value.replace(limit)));
    run()
}
/// A genuine double reach of one corresponding site is not constructible through the producer
/// (see the domain TL-556 chunk A handoff). This repeats every genuine reached locator once.
#[cfg(test)]
pub(in crate::runtime::output_scheduler::dynamic_projection::physical_adapter::position) fn with_duplicated_reach<
    T,
>(
    run: impl FnOnce() -> T,
) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            DUPLICATE_REACH.with(|value| value.set(self.0));
        }
    }
    let _restore = Restore(DUPLICATE_REACH.with(|value| value.replace(true)));
    run()
}
pub(super) fn evaluation_limit() -> usize {
    #[cfg(test)]
    {
        EVALUATION_LIMIT.with(|value| value.get().min(MAX_EVALUATIONS))
    }
    #[cfg(not(test))]
    {
        MAX_EVALUATIONS
    }
}

impl Driver {
    pub(super) fn enable_graph_discovery(&mut self, routes: usize) -> Result<(), TransitionError> {
        match self {
            Self::Full(driver) => driver.enable_graph_discovery(routes),
            Self::Graph(driver) => driver.enable_graph_discovery(routes),
            Self::Stage(driver) => driver.enable_graph_discovery(routes),
            Self::Resume(driver) => driver.enable_graph_discovery(routes),
        }
    }
    fn graph_discovery_report(&self) -> Result<PositionGraphDiscoveryReport, TransitionError> {
        match self {
            Self::Full(driver) => driver.graph_discovery_report(),
            Self::Graph(driver) => driver.graph_discovery_report(),
            Self::Stage(driver) => driver.graph_discovery_report(),
            Self::Resume(driver) => driver.graph_discovery_report(),
        }
    }
    fn enable_graph_materialization(
        &mut self,
        site: &PositionGraphOperationLocator,
    ) -> Result<(), TransitionError> {
        match self {
            Self::Full(driver) => driver.enable_graph_materialization(site),
            Self::Graph(driver) => driver.enable_graph_materialization(site),
            Self::Stage(driver) => driver.enable_graph_materialization(site),
            Self::Resume(driver) => driver.enable_graph_materialization(site),
        }
    }
    fn graph_materialization_status(
        &self,
    ) -> Result<PositionGraphMaterializationStatus, TransitionError> {
        match self {
            Self::Full(driver) => driver.graph_materialization_status(),
            Self::Graph(driver) => driver.graph_materialization_status(),
            Self::Stage(driver) => driver.graph_materialization_status(),
            Self::Resume(driver) => driver.graph_materialization_status(),
        }
    }
}

/// Runs before `pending_operations` while a graph initiator is pending. Each completed changing
/// copy must become the issued materialized request of its one corresponding original site, or
/// the whole cut is refused (`false`). Constants and owner-local stage initiators are untouched.
pub(super) fn materialize(
    observer: &PositionFrameObserver<'_>,
    frame: HybridFrameContext<'_>,
    batch: &mut Batch<'_>,
    peers: &[Peer],
    environment: &mut Environment,
    started: &mut usize,
) -> Result<bool, TransitionError> {
    let Some(initiator) = environment.work.iter().find(|item| item.request.is_some()) else {
        return Ok(true);
    };
    let request = initiator.request.as_ref().unwrap();
    let driver = initiator.driver.as_ref().unwrap();
    if driver.mask_locator(request.request_id)?.is_some()
        || driver.envelope_locator(request.request_id)?.is_some()
        || driver.resume_locator(request.request_id)?.is_some()
    {
        return Ok(true);
    }
    let locator = driver.graph_locator(request.request_id)?;
    let Some(authority) = operation(&peers[initiator.peer], request, locator)? else {
        // pending_operations applies its unchanged refusal to an unauthenticated initiator.
        return Ok(true);
    };
    for offset in 0..environment.work.len() {
        let item = &environment.work[offset];
        let Some(driver) = item.driver.as_ref() else {
            continue;
        };
        if item.request.is_some() || envelope::constant(&peers[item.peer], batch)? {
            continue;
        }
        let Some(site) = candidate(&peers[item.peer], driver, &authority)? else {
            #[cfg(test)]
            evidence(|evidence| evidence.unmatched += 1);
            return Ok(false);
        };
        if !replay(
            observer,
            frame,
            batch,
            peers,
            environment,
            offset,
            &site,
            started,
        )? {
            return Ok(false);
        }
        #[cfg(test)]
        evidence(|evidence| evidence.synchronous += 1);
    }
    Ok(true)
}

/// Exactly one reached original site of this completed driver passes site authority with
/// Shared correspondence to the initiator and the same operation kind. Zero or several refuse.
fn candidate(
    peer: &Peer,
    driver: &Driver,
    authority: &PendingOperation,
) -> Result<Option<PositionGraphOperationLocator>, TransitionError> {
    // A replayed driver has no report: a second materialization of one driver is refused.
    let Ok(report) = driver.graph_discovery_report() else {
        return Ok(None);
    };
    if !report.complete || report.unmapped_routes != 0 {
        return Ok(None);
    }
    #[allow(unused_mut)]
    let mut reached = report.reached;
    #[cfg(test)]
    if DUPLICATE_REACH.with(|value| value.get()) {
        reached.extend(reached.clone());
    }
    let mut found = Vec::new();
    for locator in reached {
        let Some(operation) = site_operation(peer, &locator)? else {
            continue;
        };
        if !operation.same_kind(authority.operation) {
            continue;
        }
        let Some(handle) = site_authority(peer, &locator, operation)? else {
            continue;
        };
        if matches!(
            authority.handle.correspondence(&handle),
            DynamicOperationCorrespondence::Shared { .. }
        ) {
            found.push(locator);
        }
    }
    Ok(match <[_; 1]>::try_from(found) {
        Ok([site]) => Some(site),
        Err(_) => None,
    })
}

/// Recycle the completed driver and begin the same captured goal again with the site armed
/// before its first advance. Its inline value is discarded; NotReached refuses, never freezes.
#[allow(clippy::too_many_arguments)]
fn replay(
    observer: &PositionFrameObserver<'_>,
    frame: HybridFrameContext<'_>,
    batch: &mut Batch<'_>,
    peers: &[Peer],
    environment: &mut Environment,
    offset: usize,
    site: &PositionGraphOperationLocator,
    started: &mut usize,
) -> Result<bool, TransitionError> {
    if *started >= evaluation_limit() {
        return Ok(false);
    }
    *started += 1;
    let goal = environment.goals[offset].clone();
    let item = &mut environment.work[offset];
    item.value = None;
    match item.driver.take() {
        Some(Driver::Full(driver)) => batch.recycle(driver)?,
        Some(Driver::Graph(driver)) => batch.recycle_graph(driver)?,
        Some(Driver::Stage(driver)) => batch.recycle_stage(driver)?,
        Some(Driver::Resume(driver)) => batch.recycle_resume(driver)?,
        None => return Err(invalid("Position synchronous peer lost its driver")),
    }
    let peer = &peers[item.peer];
    let Some(mut driver) = begin_goal(observer, frame, batch, peer, item.instance, &goal)? else {
        return Ok(false);
    };
    let armed = driver.enable_graph_materialization(site).is_ok();
    item.driver = Some(driver);
    if !armed || !advance(observer, frame, batch, peers, item)? {
        return Ok(false);
    }
    let Some(request) = &item.request else {
        return Ok(false);
    };
    let driver = item.driver.as_ref().unwrap();
    Ok(request.is_materialized_reached_site()
        && driver.graph_materialization_status()? == PositionGraphMaterializationStatus::Issued
        && driver.graph_locator(request.request_id)?.as_ref() == Some(site))
}
