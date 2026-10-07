//! Opt-in authority from actual Required/Size execution in one original owned driver.
//! Reports never establish cross-owner correspondence, fitting or output acceptance.
use super::*;

#[derive(Clone, Debug)]
pub struct PositionGraphDiscoveryReport {
    pub reached: Vec<PositionGraphOperationLocator>,
    pub inspected_routes: usize,
    pub unmapped_routes: usize,
    /// The original parent or selected operand goal finished (including Inactive).
    pub complete: bool,
}
pub(super) struct GraphDiscoveryState {
    limit: usize,
    reached: Vec<PositionGraphOperationLocator>,
    inspected_routes: usize,
    unmapped_routes: usize,
}
impl GraphDiscoveryState {
    pub(super) fn record(
        &mut self,
        locator: Option<PositionGraphOperationLocator>,
    ) -> Result<(), TransitionError> {
        ensure(
            self.inspected_routes < self.limit,
            "Position graph discovery route limit exceeded",
        )?;
        self.inspected_routes += 1;
        match locator {
            Some(locator) => self.reached.push(locator),
            None => self.unmapped_routes += 1,
        }
        Ok(())
    }
    fn report(&self, complete: bool) -> PositionGraphDiscoveryReport {
        PositionGraphDiscoveryReport {
            reached: self.reached.clone(),
            inspected_routes: self.inspected_routes,
            unmapped_routes: self.unmapped_routes,
            complete,
        }
    }
}
impl PositionCompositionContinuation {
    /// Enable before the first advance of a bound original driver. Collection adds no
    /// producer/Current sampling and leaves the existing pending-only API unchanged.
    pub fn enable_graph_discovery(&mut self, route_limit: usize) -> Result<(), TransitionError> {
        ensure(
            !self.failed,
            "failed Position composition has no graph discovery authority",
        )?;
        ensure(
            self.origin_binding.is_some(),
            "Position graph discovery requires original registry binding",
        )?;
        ensure(
            !self.started,
            "Position graph discovery must be enabled before first advance",
        )?;
        ensure(
            self.graph_discovery.is_none(),
            "Position graph discovery is already enabled",
        )?;
        ensure(
            route_limit > 0,
            "Position graph discovery requires a positive route limit",
        )?;
        self.graph_discovery = Some(GraphDiscoveryState {
            limit: route_limit,
            reached: Vec::new(),
            inspected_routes: 0,
            unmapped_routes: 0,
        });
        Ok(())
    }
    /// A safe pending prefix yields a partial report. Failure/unwind revokes report access.
    pub fn graph_discovery_report(&self) -> Result<PositionGraphDiscoveryReport, TransitionError> {
        ensure(
            !self.failed,
            "failed Position composition has no graph discovery authority",
        )?;
        let state = self
            .graph_discovery
            .as_ref()
            .ok_or_else(|| IntentError("Position graph discovery is not enabled".into()))?;
        Ok(state.report(self.completed || self.stopped_operand.is_some()))
    }
}
