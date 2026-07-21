use super::{CueMoveCopyChoice, ExecutionPolicy};
use crate::{ActionContext, ActionError};
use light_core::FixtureId;
use light_programmer::GroupDefinition;
use light_programmer::ProgrammerRegistry;
use std::collections::HashMap;
use std::collections::HashSet;

#[derive(Clone, Debug, Default)]
pub struct ProgrammingSelectionEnvironment {
    pub show_revision: u64,
    pub selectable_fixtures: HashMap<FixtureId, Vec<FixtureId>>,
    pub groups: HashMap<String, GroupDefinition>,
}

#[derive(Clone, Debug, Default)]
pub struct ProgrammingValuesEnvironment {
    pub fixture_ids: HashSet<FixtureId>,
    pub group_ids: HashSet<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProgrammingSelectionQuery {
    Fixtures(Vec<FixtureId>),
    Groups(Vec<String>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProgrammingExecution {
    Accepted {
        applied: usize,
        warning: Option<String>,
        /// The owning application action was replayed and must not repeat interaction cleanup.
        replayed: bool,
    },
    ChoiceRequired {
        pending_choice: CueMoveCopyChoice,
    },
    Rejected {
        error: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgrammingReconciliation {
    SelectionChanged,
    CaptureModeChanged,
}

/// Server-owned capabilities needed while the legacy parser, persistence, and Preload output
/// transaction are moved behind application boundaries. Transport adapters implement this port;
/// the service remains the sole owner of ordering, replay, and Programmer mutations.
pub trait ProgrammingPorts: Send + Sync {
    fn authorize(&self, _context: &ActionContext) -> Result<(), ActionError> {
        Ok(())
    }

    fn execute(
        &self,
        programmers: &ProgrammerRegistry,
        context: &ActionContext,
        command: &str,
        policy: ExecutionPolicy,
    ) -> ProgrammingExecution;

    fn selection_environment(
        &self,
        _context: &ActionContext,
        _query: &ProgrammingSelectionQuery,
    ) -> Result<ProgrammingSelectionEnvironment, ActionError> {
        Err(ActionError::new(
            crate::ActionErrorKind::Unavailable,
            "selection environment is unavailable",
        ))
    }

    fn values_environment(
        &self,
        _context: &ActionContext,
    ) -> Result<ProgrammingValuesEnvironment, ActionError> {
        Err(ActionError::new(
            crate::ActionErrorKind::Unavailable,
            "Programmer values environment is unavailable",
        ))
    }

    fn persist(&self, context: &ActionContext, operation: &'static str) -> Option<String>;

    fn capture_programmer_on_preload(&self, _context: &ActionContext) -> bool {
        true
    }

    /// Reconciles selection-derived state before the authoritative projection is captured and
    /// published. Implementations must not re-enter the Programming desk gate.
    fn reconcile(&self, context: &ActionContext, reason: ProgrammingReconciliation);

    fn commit_preload(&self, context: &ActionContext) -> Result<Option<String>, String>;
}
