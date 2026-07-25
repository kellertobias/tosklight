use crate::{
    ActionContext, ActionError, ActionErrorKind, ActiveShowPorts, ApplicationCommand, CommandFamily,
};
use light_core::{FixtureId, Revision, ShowId};
use light_show::PortableShowRevision;
use std::sync::Arc;

/// Manages one exact stored Group without reading or serializing Programmer selection state.
///
/// Every operation addresses one Group storage ID at one expected object revision. Membership is
/// resolved inside the owning Show transaction so a client can never supply, forge, or race the
/// authoritative derived/frozen source.
#[derive(Clone, Debug, PartialEq)]
pub struct GroupManagementRequest {
    pub show_id: ShowId,
    pub group_id: String,
    pub operation: GroupManagementOperation,
    pub expected_object_revision: Revision,
    pub expected_show_revision: Option<PortableShowRevision>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum GroupManagementOperation {
    /// Replaces the operator-visible label fields, retaining every other stored field.
    UpdateProperties(GroupPropertiesUpdate),
    /// Restores the exact previous stored body from adapter-owned object history.
    Undo,
    /// Recaptures the frozen membership from the current source Group.
    RefreshFrozen {
        expected_source: Option<GroupSourceExpectation>,
    },
    /// Freezes the currently resolved derived membership and drops the derivation.
    DetachDerived {
        expected_source: Option<GroupSourceExpectation>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupPropertiesUpdate {
    pub name: String,
    pub color: Option<String>,
    pub icon: Option<String>,
}

/// Optional exact source authority. A mismatch fails the action before anything mutates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupSourceExpectation {
    pub source_group_id: String,
    pub expected_source_revision: Option<Revision>,
}

impl ApplicationCommand for GroupManagementRequest {
    type Value = GroupManagementResult;

    const FAMILY: CommandFamily = CommandFamily::Programmer;
}

impl GroupManagementOperation {
    pub const fn expected_source(&self) -> Option<&GroupSourceExpectation> {
        match self {
            Self::RefreshFrozen { expected_source } | Self::DetachDerived { expected_source } => {
                expected_source.as_ref()
            }
            Self::UpdateProperties(_) | Self::Undo => None,
        }
    }

    /// Undo restores an exact historical body, so it can never be reported as a semantic no-op.
    pub const fn always_changes(&self) -> bool {
        matches!(self, Self::Undo)
    }

    pub const fn backup_label(&self) -> &'static str {
        match self {
            Self::UpdateProperties(_) => "update-group-properties",
            Self::Undo => "undo-group",
            Self::RefreshFrozen { .. } => "refresh-frozen-group",
            Self::DetachDerived { .. } => "detach-derived-group",
        }
    }
}

/// Application-owned semantic input to one adapter-owned active-show transaction.
#[derive(Clone, Debug, PartialEq)]
pub struct GroupManagementCommit {
    pub show_id: ShowId,
    pub group_id: String,
    pub expected_object_revision: Revision,
    pub expected_show_revision: Option<PortableShowRevision>,
    operation: GroupManagementOperation,
}

impl GroupManagementCommit {
    pub(crate) fn new(request: &GroupManagementRequest) -> Self {
        Self {
            show_id: request.show_id,
            group_id: request.group_id.clone(),
            expected_object_revision: request.expected_object_revision,
            expected_show_revision: request.expected_show_revision,
            operation: request.operation.clone(),
        }
    }

    pub const fn operation(&self) -> &GroupManagementOperation {
        &self.operation
    }
}

/// Exact losslessly merged Group body. These operations never delete their target.
#[derive(Clone, Debug, PartialEq)]
pub struct GroupManagementProjection {
    pub show_id: ShowId,
    pub object_id: String,
    pub object_revision: Revision,
    pub raw_body: Arc<serde_json::Value>,
}

/// Desk selection produced by a frozen refresh, published before the owning Show event.
#[derive(Clone, Debug, PartialEq)]
pub struct GroupManagementSelection {
    pub source_group_id: String,
    pub source_revision: u64,
    pub fixtures: Vec<FixtureId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GroupManagementCommitResult {
    pub changed: bool,
    pub projection: GroupManagementProjection,
    pub show_revision: PortableShowRevision,
    pub event_sequence: Option<u64>,
    pub selection: Option<GroupManagementSelection>,
}

pub trait GroupManagementPorts: Send + Sync {
    fn authorize_group_management(&self, context: &ActionContext) -> Result<(), ActionError>;

    /// Atomically validates revisions, resolves any source under the same document, applies the
    /// mutation, publishes any selection event before exactly one retained Show event, and returns
    /// the authoritative lossless projection. Implementations must not re-enter the Programming
    /// user or actor desk gates.
    fn commit_group_management(
        &self,
        context: &ActionContext,
        commit: &GroupManagementCommit,
    ) -> Result<GroupManagementCommitResult, ActionError>;

    /// Reports an adapter-owned persistence warning for an otherwise successful action.
    fn persist_group_management(&self, _context: &ActionContext) -> Option<String> {
        None
    }
}

/// Active-show adapter hooks for the narrow application-owned Group management transaction.
pub trait GroupManagementActiveShowPorts: ActiveShowPorts {
    /// Installs the refreshed frozen selection on the originating desk and publishes its selection
    /// event.
    ///
    /// This runs inside the held show-mutation gate and strictly before the owning Show event, so
    /// it must not acquire the Programming desk or user gates and must not mutate the Show.
    fn apply_frozen_group_selection(
        &self,
        context: &ActionContext,
        selection: &GroupManagementSelection,
    );
}

#[derive(Clone, Debug, PartialEq)]
pub enum GroupManagementOutcome {
    Changed {
        projection: Arc<GroupManagementProjection>,
        show_revision: PortableShowRevision,
        event_sequence: u64,
    },
    NoChange {
        projection: Arc<GroupManagementProjection>,
        show_revision: PortableShowRevision,
    },
}

impl GroupManagementOutcome {
    pub fn projection(&self) -> &GroupManagementProjection {
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
pub struct GroupManagementResult {
    pub context: ActionContext,
    pub request_id: String,
    pub replayed: bool,
    pub outcome: GroupManagementOutcome,
    pub persistence_warning: Option<String>,
}

pub(crate) fn invalid(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message)
}

pub(crate) fn not_found(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::NotFound, message)
}

#[path = "group_management/active_show.rs"]
mod active_show;
#[path = "group_management/candidate.rs"]
mod candidate;

#[cfg(test)]
#[path = "group_management/tests.rs"]
mod tests;
