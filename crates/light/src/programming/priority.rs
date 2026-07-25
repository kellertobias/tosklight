use crate::{ActionContext, ApplicationCommand, CommandFamily};
use chrono::{DateTime, Utc};
use light_core::UserId;

/// One revision-checked update of user-owned Programmer priority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgrammingPriorityRequest {
    pub expected_revision: ProgrammingPriorityRevisionExpectation,
    pub priority: i16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgrammingPriorityRevisionExpectation {
    Exact(u64),
    Current,
}

impl ApplicationCommand for ProgrammingPriorityRequest {
    type Value = ProgrammingPriorityResult;

    const FAMILY: CommandFamily = CommandFamily::Programmer;
}

/// Lightweight authority for metadata stamped onto Programmer contributions.
///
/// Normal values are deliberately absent so changing priority never materializes or serializes
/// the complete Programmer-values projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammingPriorityProjection {
    pub user_id: UserId,
    pub revision: u64,
    pub priority: i16,
    pub changed_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProgrammingPriorityChange {
    Upsert {
        projection: ProgrammingPriorityProjection,
    },
    Remove {
        user_id: UserId,
        revision: u64,
    },
}

impl ProgrammingPriorityChange {
    pub const fn user_id(&self) -> UserId {
        match self {
            Self::Upsert { projection } => projection.user_id,
            Self::Remove { user_id, .. } => *user_id,
        }
    }

    pub const fn revision(&self) -> u64 {
        match self {
            Self::Upsert { projection } => projection.revision,
            Self::Remove { revision, .. } => *revision,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammingPrioritySnapshot {
    pub event_sequence: u64,
    pub projection: ProgrammingPriorityProjection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgrammingPriorityActionState {
    Changed { event_sequence: u64 },
    NoChange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammingPriorityResult {
    pub context: ActionContext,
    pub request_id: String,
    pub projection: ProgrammingPriorityProjection,
    pub outcome: ProgrammingPriorityActionState,
    pub replayed: bool,
    pub warning: Option<String>,
}

impl ProgrammingPriorityResult {
    pub const fn event_sequence(&self) -> Option<u64> {
        match self.outcome {
            ProgrammingPriorityActionState::Changed { event_sequence } => Some(event_sequence),
            ProgrammingPriorityActionState::NoChange => None,
        }
    }
}
