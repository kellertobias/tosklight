use super::{ProgrammingPorts, ProgrammingService};
use crate::{ActionContext, ActionError, ActionErrorKind};
use light_core::{SessionId, UserId};
use light_programmer::{
    ProgrammerFixtureUpdate, ProgrammerGroupUpdate, ProgrammerRegistry, ProgrammerUpdateContent,
};
use std::sync::Arc;

#[cfg(test)]
use std::cell::Cell;

#[cfg(test)]
thread_local! {
    static PROJECTION_READS: Cell<usize> = const { Cell::new(0) };
}

/// Complete user-owned normal Programmer values used by recordable UI projections.
///
/// Selection, command interaction, Preload, modes, priority, connectivity, Highlight, and
/// transient control actions deliberately live outside this boundary.
#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingValuesProjection {
    pub user_id: UserId,
    pub revision: u64,
    pub fixture_values: Vec<ProgrammerFixtureUpdate>,
    pub group_values: Vec<ProgrammerGroupUpdate>,
}

/// One semantic normal-value transition. Events carry the full retained projection so a
/// replaceable delivery can supersede an older queued value update safely.
#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingValuesChange {
    pub projection: Arc<ProgrammingValuesProjection>,
}

/// Authoritative gap-repair snapshot for one authenticated user's normal Programmer values.
#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingValuesSnapshot {
    pub event_sequence: u64,
    pub projection: ProgrammingValuesProjection,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct ProgrammingValuesContent {
    pub(super) fixture_values: Vec<ProgrammerFixtureUpdate>,
    pub(super) group_values: Vec<ProgrammerGroupUpdate>,
}

impl ProgrammingValuesContent {
    pub(super) fn read(
        programmers: &ProgrammerRegistry,
        session: SessionId,
        user_id: UserId,
    ) -> Result<Self, ActionError> {
        #[cfg(test)]
        PROJECTION_READS.set(PROJECTION_READS.get() + 1);
        let state = programmers
            .get(session)
            .ok_or_else(programmer_values_unavailable)?;
        if state.user_id != user_id {
            return Err(ActionError::new(
                ActionErrorKind::Forbidden,
                "the Programmer session does not belong to the requested user",
            ));
        }
        let ProgrammerUpdateContent {
            fixture_values,
            group_values,
            selected_fixtures: _,
        } = state.update_content();
        Ok(Self {
            fixture_values,
            group_values,
        })
    }

    pub(super) fn projection(self, user_id: UserId, revision: u64) -> ProgrammingValuesProjection {
        ProgrammingValuesProjection {
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
    pub fn values_snapshot(
        &self,
        context: &ActionContext,
        ports: &dyn ProgrammingPorts,
    ) -> Result<ProgrammingValuesSnapshot, ActionError> {
        let (session, user_id) = values_identity(context)?;
        self.with_user_and_desk_gate(context.desk_id, user_id, || {
            ports.authorize(context)?;
            // Reading the cursor first permits a duplicate after repair, but cannot skip a
            // same-user mutation because that transition uses this same user gate.
            let event_sequence = self.events.latest_sequence();
            let content = ProgrammingValuesContent::read(&self.programmers, session, user_id)?;
            let revision = self.programmers.normal_values_revision(user_id);
            Ok(ProgrammingValuesSnapshot {
                event_sequence,
                projection: content.projection(user_id, revision),
            })
        })
    }
}

fn values_identity(context: &ActionContext) -> Result<(SessionId, UserId), ActionError> {
    let session = context.session_id.map(SessionId).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Unauthorized,
            "Programmer value snapshots require an operator session",
        )
    })?;
    let user_id = context.user_id.map(UserId).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Unauthorized,
            "Programmer value snapshots require an authenticated user",
        )
    })?;
    Ok((session, user_id))
}

fn programmer_values_unavailable() -> ActionError {
    ActionError::new(
        ActionErrorKind::NotFound,
        "Programmer values are unavailable",
    )
}
