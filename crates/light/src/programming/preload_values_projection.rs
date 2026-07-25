use super::{ProgrammingPorts, ProgrammingService};
use crate::{ActionContext, ActionError, ActionErrorKind};
use light_core::{SessionId, UserId};
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
    pub user_id: UserId,
    pub revision: u64,
    pub fixture_values: Vec<PreloadProgrammerFixtureValue>,
    pub group_values: Vec<PreloadProgrammerGroupValue>,
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
}

impl ProgrammingPreloadValuesContent {
    pub(super) fn read(
        programmers: &ProgrammerRegistry,
        session: SessionId,
        user_id: UserId,
    ) -> Result<Self, ActionError> {
        #[cfg(test)]
        PROJECTION_READS.set(PROJECTION_READS.get() + 1);
        if programmers.user_id(session) != Some(user_id) {
            return Err(ActionError::new(
                ActionErrorKind::Forbidden,
                "the Programmer session does not belong to the requested user",
            ));
        }
        let PreloadProgrammerValuesContent {
            fixture_values,
            group_values,
        } = programmers
            .preload_pending_values(session)
            .ok_or_else(preload_values_unavailable)?;
        Ok(Self {
            fixture_values,
            group_values,
        })
    }

    pub(super) fn projection(
        self,
        user_id: UserId,
        revision: u64,
    ) -> ProgrammingPreloadValuesProjection {
        ProgrammingPreloadValuesProjection {
            user_id,
            revision,
            fixture_values: self.fixture_values,
            group_values: self.group_values,
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
        let (session, user_id) = preload_values_identity(context)?;
        self.with_user_and_desk_gate(context.desk_id, user_id, || {
            ports.authorize(context)?;
            let event_sequence = self.events.latest_sequence();
            let content =
                ProgrammingPreloadValuesContent::read(&self.programmers, session, user_id)?;
            let revision = self.programmers.preload_values_revision(user_id);
            Ok(ProgrammingPreloadValuesSnapshot {
                event_sequence,
                projection: content.projection(user_id, revision),
            })
        })
    }
}

fn preload_values_identity(context: &ActionContext) -> Result<(SessionId, UserId), ActionError> {
    let session = context.session_id.map(SessionId).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Unauthorized,
            "Preload values snapshots require an operator session",
        )
    })?;
    let user_id = context.user_id.map(UserId).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Unauthorized,
            "Preload values snapshots require an authenticated user",
        )
    })?;
    Ok((session, user_id))
}

fn preload_values_unavailable() -> ActionError {
    ActionError::new(ActionErrorKind::NotFound, "Preload values are unavailable")
}
