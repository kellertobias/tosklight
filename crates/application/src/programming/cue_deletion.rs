#[path = "cue_deletion/active_show.rs"]
mod active_show;
#[path = "cue_deletion/candidate.rs"]
mod candidate;

use crate::{
    ActionContext, ActionError, ActiveShowObjectChange, ActiveShowPorts, ApplicationCommand,
    CommandFamily, CueNumber,
};
use light_core::{CueListId, Revision, ShowId};
use light_show::PortableShowRevision;
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ProgrammingCueDeletionAddress {
    Pool { playback_number: u16 },
    CurrentPage { expected_page: u8, slot: u8 },
    PageSlot { page: u8, slot: u8 },
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ProgrammingCueDeletionAuthority {
    pub playback_number: u16,
    pub cue_list_id: CueListId,
    pub object_id: String,
    pub object_revision: Revision,
    pub cue_id: Uuid,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum ProgrammingCueDeletionExpectation {
    Current,
    Exact(ProgrammingCueDeletionAuthority),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingCueDeletionRequest {
    pub show_id: ShowId,
    pub address: ProgrammingCueDeletionAddress,
    pub cue_number: CueNumber,
    pub expectation: ProgrammingCueDeletionExpectation,
}

impl ApplicationCommand for ProgrammingCueDeletionRequest {
    type Value = ProgrammingCueDeletionResult;

    const FAMILY: CommandFamily = CommandFamily::Programmer;
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingCueDeletionObjectProjection {
    pub cue_list_id: CueListId,
    pub object_id: String,
    pub object_revision: Revision,
    pub raw_body: Arc<Value>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProgrammingDeletedCue {
    pub id: Uuid,
    pub number: CueNumber,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgrammingCueDeletionState {
    Changed { show_event_sequence: u64 },
    NoChange,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingCueDeletionOutcome {
    pub show_id: ShowId,
    pub show_revision: PortableShowRevision,
    pub cue_list: ProgrammingCueDeletionObjectProjection,
    pub deleted_cue: ProgrammingDeletedCue,
    pub state: ProgrammingCueDeletionState,
    pub persistence_warning: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingCueDeletionResult {
    pub context: ActionContext,
    pub request_id: String,
    pub correlation_id: Uuid,
    pub replayed: bool,
    pub outcome: ProgrammingCueDeletionOutcome,
}

pub trait ProgrammingCueDeletionPorts: ActiveShowPorts {
    /// Authenticates immutable operator identity. Replay lookup follows this check and precedes
    /// every mutable desk/show/address precondition.
    fn authorize_cue_deletion_identity(&self, context: &ActionContext) -> Result<(), ActionError>;

    fn current_cue_deletion_page(
        &self,
        context: &ActionContext,
        show_id: ShowId,
    ) -> Result<u8, ActionError>;

    fn reconcile_cue_deletion(&self, _changes: &[ActiveShowObjectChange]) {}

    fn persist_cue_deletion(&self, _context: &ActionContext) -> Option<String> {
        None
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::programming) struct ResolvedCueDeletionRequest {
    pub show_id: ShowId,
    pub address: super::cue_list_resolution::CueListAddress,
    pub cue_number: CueNumber,
    pub expectation: ProgrammingCueDeletionExpectation,
}

#[cfg(test)]
#[path = "cue_deletion/tests.rs"]
mod tests;
