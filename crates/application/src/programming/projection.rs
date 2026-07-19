use super::{ProgrammingPorts, ProgrammingService};
use crate::{ActionContext, ActionError, ActionErrorKind};
use light_core::SessionId;
use light_programmer::{CommandLineState, ProgrammerRegistry, ProgrammerSelection};
use uuid::Uuid;

/// Authoritative desk-local command and ordered-selection state.
///
/// Programmer values remain user-scoped. This projection intentionally contains only the
/// interaction context shared by software, keyboard, OSC, and attached hardware on one desk.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammingInteractionProjection {
    pub desk_id: Uuid,
    pub command_line: CommandLineState,
    pub selection: ProgrammerSelection,
}

impl ProgrammingInteractionProjection {
    pub(super) fn read(
        programmers: &ProgrammerRegistry,
        desk_id: Uuid,
        session: SessionId,
    ) -> Result<Self, ActionError> {
        let interaction = programmers
            .interaction_state(session)
            .ok_or_else(interaction_unavailable)?;
        Ok(Self {
            desk_id,
            command_line: interaction.command_line,
            selection: interaction.selection,
        })
    }
}

/// Snapshot repair boundary for the desk-local Programming interaction stream.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammingLiveSnapshot {
    pub event_sequence: u64,
    pub interaction: ProgrammingInteractionProjection,
}

impl ProgrammingService {
    pub fn snapshot(
        &self,
        context: &ActionContext,
        ports: &dyn ProgrammingPorts,
    ) -> Result<ProgrammingLiveSnapshot, ActionError> {
        let session = snapshot_session(context)?;
        self.with_desk_gate(context.desk_id, || {
            self.capture_snapshot(context, ports, session)
        })
    }

    fn capture_snapshot(
        &self,
        context: &ActionContext,
        ports: &dyn ProgrammingPorts,
        session: SessionId,
    ) -> Result<ProgrammingLiveSnapshot, ActionError> {
        ports.authorize(context)?;
        // Capturing the cursor before the immutable read permits a duplicate after repair,
        // but cannot miss a mutation which completes while the snapshot is assembled.
        let event_sequence = self.events.latest_sequence();
        let interaction =
            ProgrammingInteractionProjection::read(&self.programmers, context.desk_id, session)?;
        Ok(ProgrammingLiveSnapshot {
            event_sequence,
            interaction,
        })
    }
}

fn snapshot_session(context: &ActionContext) -> Result<SessionId, ActionError> {
    context.session_id.map(SessionId).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Unauthorized,
            "programming snapshots require an operator session",
        )
    })
}

fn interaction_unavailable() -> ActionError {
    ActionError::new(
        ActionErrorKind::NotFound,
        "programming interaction is unavailable",
    )
}
