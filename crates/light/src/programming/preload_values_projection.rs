use super::{ProgrammingPorts, ProgrammingService};
use crate::{ActionContext, ActionError, ActionErrorKind};
use light_core::SessionId;
use light_dynamics::DynamicAddressValue;
use light_programmer::{
    PreloadProgrammerFixtureValue, PreloadProgrammerGroupValue, PreloadProgrammerValuesContent,
    ProgrammerRegistry,
};
use std::sync::Arc;

#[cfg(test)]
use std::cell::Cell;

#[cfg(test)]
thread_local! {
    static PROJECTION_READS: Cell<usize> = const { Cell::new(0) };
}

/// Complete pending fixture and Group values prepared by one user's Preload capture.
///
/// Active Preload output, queued playback actions, normal Programmer values, capture mode,
/// selection, Highlight, and transient controls deliberately live outside this boundary.
#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingPreloadValuesProjection {
    pub revision: u64,
    pub fixture_values: Vec<PreloadProgrammerFixtureValue>,
    pub group_values: Vec<PreloadProgrammerGroupValue>,
    pub dynamic_values: Vec<DynamicAddressValue>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingPreloadValuesChange {
    pub projection: Arc<ProgrammingPreloadValuesProjection>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingPreloadValuesSnapshot {
    pub event_sequence: u64,
    pub projection: ProgrammingPreloadValuesProjection,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct ProgrammingPreloadValuesContent {
    fixture_values: Vec<PreloadProgrammerFixtureValue>,
    group_values: Vec<PreloadProgrammerGroupValue>,
    dynamic_values: Vec<DynamicAddressValue>,
}

impl ProgrammingPreloadValuesContent {
    pub(super) fn read(
        programmers: &ProgrammerRegistry,
        session: SessionId,
    ) -> Result<Self, ActionError> {
        #[cfg(test)]
        PROJECTION_READS.set(PROJECTION_READS.get() + 1);
        if !programmers.knows_session(session) {
            return Err(ActionError::new(
                ActionErrorKind::Forbidden,
                "the Programmer session does not belong to the requested user",
            ));
        }
        let PreloadProgrammerValuesContent {
            fixture_values,
            group_values,
            dynamic_values,
        } = programmers
            .preload_pending_values(session)
            .ok_or_else(preload_values_unavailable)?;
        Ok(Self {
            fixture_values,
            group_values,
            dynamic_values,
        })
    }

    pub(super) fn projection(self, revision: u64) -> ProgrammingPreloadValuesProjection {
        ProgrammingPreloadValuesProjection {
            revision,
            fixture_values: self.fixture_values,
            group_values: self.group_values,
            dynamic_values: self.dynamic_values,
        }
    }
}

#[cfg(test)]
pub(super) fn reset_projection_read_count() {
    PROJECTION_READS.set(0);
}

#[cfg(test)]
pub(super) fn projection_read_count() -> usize {
    PROJECTION_READS.get()
}

impl ProgrammingService {
    pub fn preload_values_snapshot(
        &self,
        context: &ActionContext,
        ports: &dyn ProgrammingPorts,
    ) -> Result<ProgrammingPreloadValuesSnapshot, ActionError> {
        let session = preload_values_identity(context)?;
        self.with_programmer_and_desk_gate(context.desk_id, || {
            ports.authorize(context)?;
            let event_sequence = self.events.latest_sequence();
            let content = ProgrammingPreloadValuesContent::read(&self.programmers, session)?;
            let revision = self.programmers.preload_values_revision();
            Ok(ProgrammingPreloadValuesSnapshot {
                event_sequence,
                projection: content.projection(revision),
            })
        })
    }
}

fn preload_values_identity(context: &ActionContext) -> Result<SessionId, ActionError> {
    let session = context.session_id.map(SessionId).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Unauthorized,
            "Preload values snapshots require an operator session",
        )
    })?;
    Ok(session)
}

fn preload_values_unavailable() -> ActionError {
    ActionError::new(ActionErrorKind::NotFound, "Preload values are unavailable")
}
