use super::{CueMoveCopyChoice, ExecutionPolicy};
use crate::{ActionContext, ActionError};
use light_programmer::ProgrammerRegistry;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProgrammingExecution {
    Accepted {
        applied: usize,
        warning: Option<String>,
    },
    ChoiceRequired {
        pending_choice: CueMoveCopyChoice,
    },
    Rejected {
        error: String,
    },
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

    fn persist(&self, context: &ActionContext, operation: &'static str) -> Option<String>;

    fn capture_programmer_on_preload(&self, _context: &ActionContext) -> bool {
        true
    }

    fn commit_preload(&self, context: &ActionContext) -> Result<Option<String>, String>;
}
