use crate::{ActionContext, ApplicationCommand, CommandFamily};
use light_core::{Revision, ShowId};
use light_output::OutputRoute;
use light_show::PortableShowRevision;
use serde_json::Value;

/// One typed output-route edit performed against the active portable show.
#[derive(Clone, Debug, PartialEq)]
pub struct MutateOutputRouteCommand {
    pub show_id: ShowId,
    pub route_id: String,
    /// Compatibility revision from the v1 object endpoint. The application service still commits
    /// the complete candidate against the document's whole-show revision.
    pub expected_object_revision: Revision,
    pub mutation: OutputRouteMutation,
}

impl ApplicationCommand for MutateOutputRouteCommand {
    type Value = MutateOutputRouteResult;

    const FAMILY: CommandFamily = CommandFamily::Output;
}

#[derive(Clone, Debug, PartialEq)]
pub enum OutputRouteMutation {
    Put { body: Value },
    Delete,
}

/// Targeted active-show projection published after one committed route edit.
#[derive(Clone, Debug, PartialEq)]
pub struct OutputRouteChange {
    pub show_id: ShowId,
    pub show_revision: PortableShowRevision,
    pub route_id: String,
    pub object_revision: Revision,
    pub route: Option<OutputRoute>,
    pub deleted: bool,
}

/// Mutation result plus the one old network route requiring targeted termination, if any.
#[derive(Clone, Debug, PartialEq)]
pub struct MutateOutputRouteResult {
    pub context: ActionContext,
    pub change: OutputRouteChange,
    pub route_to_terminate: Option<OutputRoute>,
    pub event_sequence: u64,
}
