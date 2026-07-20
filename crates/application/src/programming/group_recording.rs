use crate::{ActionContext, ActionError, ActiveShowPorts, ApplicationCommand, CommandFamily};
use light_core::{Revision, SessionId, ShowId};
use light_programmer::{GroupDefinition, GroupRecordingCapture, ProgrammerRegistry};
use light_show::PortableShowRevision;
use std::collections::HashMap;
use std::sync::Arc;

/// Records or deletes one exact opaque Group object from the action-time Programmer selection.
///
/// Selection content is intentionally absent so clients cannot forge or race the actor's
/// desk-local authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammingGroupRecordRequest {
    pub show_id: ShowId,
    pub group_id: String,
    pub operation: ProgrammingGroupRecordOperation,
    pub expected_object_revision: ProgrammingGroupRevisionExpectation,
    pub expected_show_revision: Option<PortableShowRevision>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgrammingGroupRecordOperation {
    Overwrite,
    Merge,
    Subtract,
    Delete,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgrammingGroupRevisionExpectation {
    Exact(Revision),
    Current,
}

impl ApplicationCommand for ProgrammingGroupRecordRequest {
    type Value = ProgrammingGroupRecordResult;

    const FAMILY: CommandFamily = CommandFamily::Programmer;
}

/// Application-owned semantic input to one adapter-owned active-show transaction.
#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingGroupCommit {
    pub show_id: ShowId,
    pub group_id: String,
    pub expected_object_revision: ProgrammingGroupRevisionExpectation,
    pub expected_show_revision: Option<PortableShowRevision>,
    operation: ProgrammingGroupRecordOperation,
    capture: GroupRecordingCapture,
    session_id: SessionId,
    within_interaction: bool,
}

impl ProgrammingGroupCommit {
    pub(crate) fn new(
        request: &ProgrammingGroupRecordRequest,
        capture: GroupRecordingCapture,
        session_id: SessionId,
        within_interaction: bool,
    ) -> Self {
        Self {
            show_id: request.show_id,
            group_id: request.group_id.clone(),
            expected_object_revision: request.expected_object_revision,
            expected_show_revision: request.expected_show_revision,
            operation: request.operation,
            capture,
            session_id,
            within_interaction,
        }
    }

    pub const fn operation(&self) -> ProgrammingGroupRecordOperation {
        self.operation
    }

    pub const fn within_interaction(&self) -> bool {
        self.within_interaction
    }

    pub const fn actor_session_id(&self) -> SessionId {
        self.session_id
    }

    pub fn deletes_target(&self) -> bool {
        self.operation == ProgrammingGroupRecordOperation::Delete
            || (self.operation == ProgrammingGroupRecordOperation::Subtract
                && self.capture.fixtures().is_empty())
    }

    /// Record operations close the ordinary selection gesture; explicit Delete preserves it.
    pub const fn finishes_actor_gesture(&self) -> bool {
        !matches!(self.operation, ProgrammingGroupRecordOperation::Delete)
    }

    pub fn applied(&self) -> usize {
        if self.deletes_target() {
            1
        } else {
            self.capture.fixtures().len()
        }
    }

    /// Finish the actor's ordinary surface-selection gesture while the caller owns the existing
    /// Programming interaction. Changed installs call this inside their coalesced topology
    /// refresh; the application calls it again after a successful commit to cover no-change.
    pub fn finish_actor_selection_gesture(&self, programmers: &ProgrammerRegistry) -> bool {
        self.finishes_actor_gesture()
            && programmers.finish_selection_gesture_within_interaction(self.session_id)
    }

    pub fn updated_group(
        &self,
        existing: Option<&GroupDefinition>,
        groups: &HashMap<String, GroupDefinition>,
    ) -> Result<Option<GroupDefinition>, ActionError> {
        if self.deletes_target() {
            return Ok(None);
        }
        let group = match self.operation {
            ProgrammingGroupRecordOperation::Overwrite => {
                self.capture.overwrite(&self.group_id, existing, groups)
            }
            ProgrammingGroupRecordOperation::Merge => self
                .capture
                .merge(
                    &self.group_id,
                    existing.ok_or_else(|| missing_group(&self.group_id))?,
                    groups,
                )
                .map_err(invalid)?,
            ProgrammingGroupRecordOperation::Subtract => self
                .capture
                .subtract(
                    &self.group_id,
                    existing.ok_or_else(|| missing_group(&self.group_id))?,
                    groups,
                )
                .map_err(invalid)?,
            ProgrammingGroupRecordOperation::Delete => unreachable!("delete returned above"),
        };
        Ok(Some(group))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingGroupProjection {
    pub show_id: ShowId,
    pub object_id: String,
    pub object_revision: Revision,
    /// Exact losslessly merged body. A deletion is represented by `None` and `deleted = true`.
    pub raw_body: Option<Arc<serde_json::Value>>,
    pub deleted: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingGroupCommitResult {
    pub changed: bool,
    pub projection: ProgrammingGroupProjection,
    pub show_revision: PortableShowRevision,
    pub event_sequence: Option<u64>,
}

pub trait ProgrammingGroupRecordingPorts: Send + Sync {
    fn authorize_group_recording(&self, context: &ActionContext) -> Result<(), ActionError>;

    /// Atomically validates revisions, applies the domain-owned mutation, installs the candidate,
    /// coalesces Group-topology selection repair, and emits one retained Show event when changed.
    /// Implementations must not re-enter the Programming user or actor desk gates.
    fn commit_group(
        &self,
        context: &ActionContext,
        commit: &ProgrammingGroupCommit,
    ) -> Result<ProgrammingGroupCommitResult, ActionError>;
}

/// Active-show adapter hooks for the narrow application-owned Group transaction.
pub trait ProgrammingGroupActiveShowPorts: ActiveShowPorts {}

#[derive(Clone, Debug, PartialEq)]
pub enum ProgrammingGroupRecordOutcome {
    Changed {
        projection: Arc<ProgrammingGroupProjection>,
        show_revision: PortableShowRevision,
        event_sequence: u64,
    },
    NoChange {
        projection: Arc<ProgrammingGroupProjection>,
        show_revision: PortableShowRevision,
    },
}

impl ProgrammingGroupRecordOutcome {
    pub fn projection(&self) -> &ProgrammingGroupProjection {
        match self {
            Self::Changed { projection, .. } | Self::NoChange { projection, .. } => projection,
        }
    }

    pub const fn show_revision(&self) -> PortableShowRevision {
        match self {
            Self::Changed { show_revision, .. } | Self::NoChange { show_revision, .. } => {
                *show_revision
            }
        }
    }

    pub const fn event_sequence(&self) -> Option<u64> {
        match self {
            Self::Changed { event_sequence, .. } => Some(*event_sequence),
            Self::NoChange { .. } => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingGroupRecordResult {
    pub context: ActionContext,
    pub request_id: String,
    pub replayed: bool,
    pub applied: usize,
    pub outcome: ProgrammingGroupRecordOutcome,
}

fn missing_group(group_id: &str) -> ActionError {
    ActionError::new(
        crate::ActionErrorKind::NotFound,
        format!("Group {group_id} does not exist"),
    )
}

fn invalid(message: impl Into<String>) -> ActionError {
    ActionError::new(crate::ActionErrorKind::Invalid, message)
}
