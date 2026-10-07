//! Opt-in materialization at one exact original reached Required/Size site of the same bound
//! driver goal. The inline value is discarded; the ordinary request path exposes the original
//! operation parameters and the parent suffix resumes once from the supplied result.
//! A locator is owner-local: this grants no cross-owner, fitting or output authority.
use super::super::position_conditioning::origins::PositionOriginBinding;
use super::graph_discovery::GraphDiscoveryState;
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PositionGraphMaterializationStatus {
    /// Enabled; the site has not been reached by the actual goal so far.
    Armed,
    /// The site's request is outstanding.
    Issued,
    /// The site's request was answered; later suffix values derive from that response.
    Resumed,
    /// The goal completed, stopped or became Inactive without reaching the site. Its value
    /// is an ordinary inline result, never a materialized one.
    NotReached,
}
pub(super) struct GraphMaterializationState {
    target: PositionGraphOperationLocator,
    fired: bool,
    issued: Option<Uuid>,
}

/// Reached-site decision for one actual route: record discovery, then compare the exact
/// original locator. A second reach of the same armed site is an ambiguity, not a retry.
pub(super) fn reached_action(
    (discovery, materialization): (
        &mut Option<GraphDiscoveryState>,
        &mut Option<GraphMaterializationState>,
    ),
    binding: Option<&PositionOriginBinding>,
    route: &PreparedPositionGraphRoute,
) -> Result<GraphReachedAction, TransitionError> {
    let binding = binding.ok_or_else(|| {
        IntentError("Position reached graph operation lost its original binding".into())
    })?;
    let locator = binding.graph_locator(route)?;
    let matched = materialization
        .as_ref()
        .is_some_and(|state| locator.as_ref() == Some(&state.target));
    if let Some(discovery) = discovery {
        discovery.record(locator)?;
    }
    let Some(state) = materialization.as_mut().filter(|_| matched) else {
        return Ok(GraphReachedAction::Continue);
    };
    ensure(
        !state.fired,
        "Position graph materialization site was reached ambiguously",
    )?;
    state.fired = true;
    Ok(GraphReachedAction::Materialize)
}

impl PositionCompositionRequest {
    /// True only for the request issued at the armed reached site of this driver.
    pub fn is_materialized_reached_site(&self) -> bool {
        self.materialized_reached_site
    }
}

impl PositionCompositionContinuation {
    /// Arm one exact original reached Required/Size site before the first advance. Every
    /// identity check happens here, before the driver evaluates or lends its workspace.
    pub fn enable_graph_materialization(
        &mut self,
        site: &PositionGraphOperationLocator,
    ) -> Result<(), TransitionError> {
        ensure(
            !self.failed,
            "failed Position composition has no graph materialization authority",
        )?;
        let binding = self.origin_binding.as_ref().ok_or_else(|| {
            IntentError("Position graph materialization requires original registry binding".into())
        })?;
        ensure(
            binding.owns_graph_locator(site),
            "Position graph materialization site belongs to another captured registry",
        )?;
        ensure(
            !self.started,
            "Position graph materialization must be enabled before first advance",
        )?;
        ensure(
            self.graph_materialization.is_none(),
            "Position graph materialization is already enabled",
        )?;
        ensure(
            !self.completed,
            "completed Position composition has no reachable graph operation",
        )?;
        ensure(
            !self
                .graph_replay
                .as_ref()
                .is_some_and(|(locator, _)| locator == site),
            "Position graph operand stops before its own operation and cannot materialize it",
        )?;
        self.graph_materialization = Some(GraphMaterializationState {
            target: site.clone(),
            fired: false,
            issued: None,
        });
        Ok(())
    }

    pub fn graph_materialization_status(
        &self,
    ) -> Result<PositionGraphMaterializationStatus, TransitionError> {
        ensure(
            !self.failed,
            "failed Position composition has no graph materialization authority",
        )?;
        let state = self
            .graph_materialization
            .as_ref()
            .ok_or_else(|| IntentError("Position graph materialization is not enabled".into()))?;
        Ok(match state.issued {
            Some(id)
                if self
                    .pending
                    .as_ref()
                    .is_some_and(|request| request.request_id == id) =>
            {
                PositionGraphMaterializationStatus::Issued
            }
            Some(_) => PositionGraphMaterializationStatus::Resumed,
            None if self.completed || self.stopped_operand.is_some() => {
                PositionGraphMaterializationStatus::NotReached
            }
            None => PositionGraphMaterializationStatus::Armed,
        })
    }

    /// Called by wait() with its freshly minted id. The first request after the armed site
    /// fired must be that exact original site; it keeps the ordinary request identity.
    pub(super) fn issue_graph_materialization(
        &mut self,
        operation: &PositionCompositionOperation,
        request_id: Uuid,
    ) -> Result<bool, TransitionError> {
        let Some(state) = self.graph_materialization.as_mut() else {
            return Ok(false);
        };
        if !state.fired || state.issued.is_some() {
            return Ok(false);
        }
        let binding = self.origin_binding.as_ref().ok_or_else(|| {
            IntentError("Position graph materialization lost its original binding".into())
        })?;
        let route = match operation {
            PositionCompositionOperation::Base { request, .. } => request.graph_route.as_ref(),
            _ => None,
        };
        let locator = match route {
            Some(route) => binding.graph_locator(route)?,
            None => None,
        };
        ensure(
            locator.as_ref() == Some(&state.target),
            "Position graph materialization request names another operation",
        )?;
        state.issued = Some(request_id);
        Ok(true)
    }
}
