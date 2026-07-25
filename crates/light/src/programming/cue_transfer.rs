#[path = "cue_transfer/active_show.rs"]
mod active_show;
#[path = "cue_transfer/candidate.rs"]
mod candidate;
#[path = "cue_transfer/resolution.rs"]
mod resolution;

use crate::{
    ActionContext, ActionError, ActiveShowObjectChange, ActiveShowPorts, ApplicationCommand,
    CommandFamily, CueNumber,
};
use light_core::{CueListId, Revision, ShowId};
use light_programmer::{CommandLineState, CueTransferOperation};
use light_show::PortableShowRevision;
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ProgrammingCueTransferAddress {
    Pool { playback_number: u16 },
    PageSlot { page: u8, slot: u8 },
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ProgrammingCueTransferEndpoint {
    pub address: ProgrammingCueTransferAddress,
    pub cue_number: CueNumber,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammingCueTransferChoiceRequest {
    pub show_id: ShowId,
    pub operation: CueTransferOperation,
    pub source: ProgrammingCueTransferEndpoint,
    pub destination: ProgrammingCueTransferEndpoint,
    pub command: String,
    pub plain_command: String,
    pub status_command: String,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ProgrammingCueTransferMode {
    Plain,
    Status,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ProgrammingCueTransferRequest {
    pub show_id: ShowId,
    pub choice_id: Uuid,
    pub mode: ProgrammingCueTransferMode,
    pub expected_command_line_revision: Revision,
}

impl ApplicationCommand for ProgrammingCueTransferRequest {
    type Value = ProgrammingCueTransferResult;

    const FAMILY: CommandFamily = CommandFamily::Programmer;
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingCueTransferObjectProjection {
    pub cue_list_id: CueListId,
    pub object_id: String,
    pub object_revision: Revision,
    pub raw_body: Arc<Value>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProgrammingCueTransferSummary {
    pub operation: CueTransferOperation,
    pub mode: ProgrammingCueTransferMode,
    pub source_cue_id: Uuid,
    pub source_cue_number: CueNumber,
    pub destination_cue_id: Uuid,
    pub destination_cue_number: CueNumber,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingCueTransferOutcome {
    pub show_id: ShowId,
    pub summary: ProgrammingCueTransferSummary,
    pub show_revision: PortableShowRevision,
    pub projections: Arc<[ProgrammingCueTransferObjectProjection]>,
    pub show_event_sequence: u64,
    pub command_line: CommandLineState,
    pub interaction_event_sequence: Option<u64>,
    pub persistence_warning: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingCueTransferResult {
    pub context: ActionContext,
    pub request_id: String,
    pub choice_id: Uuid,
    pub correlation_id: Uuid,
    pub replayed: bool,
    pub outcome: ProgrammingCueTransferOutcome,
}

pub trait ProgrammingCueTransferPorts: ActiveShowPorts {
    fn authorize_cue_transfer(&self, context: &ActionContext) -> Result<(), ActionError>;

    fn reconcile_cue_transfer(&self, _changes: &[ActiveShowObjectChange]) {}

    fn persist_cue_transfer(&self, _context: &ActionContext) -> Option<String> {
        None
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CueTransferAuthority {
    pub choice_id: Uuid,
    pub show_id: ShowId,
    pub show_revision: PortableShowRevision,
    pub operation: CueTransferOperation,
    pub source: ResolvedCueTransferEndpoint,
    pub destination: ResolvedCueTransferEndpoint,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResolvedCueTransferEndpoint {
    pub requested: ProgrammingCueTransferEndpoint,
    pub playback_number: u16,
    pub cue_list_id: CueListId,
    pub object_id: String,
    pub object_revision: Revision,
    pub cue_id: Option<Uuid>,
}

#[cfg(test)]
#[path = "cue_transfer/tests.rs"]
mod tests;
