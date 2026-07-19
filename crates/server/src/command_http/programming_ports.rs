use super::adapter::{ExistingCommandOutcome, ExistingCommandPolicy, execute_existing_command};
use super::events::persist_with_warning;
use super::wire::application_choice;
use light_application::{
    ActionContext, ActionError, ActionErrorKind, ExecutionPolicy, ProgrammingExecution,
    ProgrammingPorts, ProgrammingSelectionEnvironment, ProgrammingSelectionQuery,
    ProgrammingValuesEnvironment,
};
use light_programmer::ProgrammerRegistry;

use super::super::{AppState, Session};

pub(crate) struct ServerProgrammingPorts<'a> {
    state: &'a AppState,
    session: &'a Session,
    source: &'static str,
    require_unlocked: bool,
}

impl<'a> ServerProgrammingPorts<'a> {
    pub(crate) const fn new(
        state: &'a AppState,
        session: &'a Session,
        source: &'static str,
        require_unlocked: bool,
    ) -> Self {
        Self {
            state,
            session,
            source,
            require_unlocked,
        }
    }
}

impl ProgrammingPorts for ServerProgrammingPorts<'_> {
    fn authorize(&self, context: &ActionContext) -> Result<(), ActionError> {
        let identity_matches = context.desk_id == self.session.desk.id
            && context.session_id == Some(self.session.id.0)
            && context.user_id == Some(self.session.user.id.0);
        if !identity_matches {
            return Err(ActionError::new(
                ActionErrorKind::Forbidden,
                "the action context does not match the authenticated operator session",
            ));
        }
        if self.require_unlocked && super::super::read_desk_lock(self.state, context.desk_id).locked
        {
            return Err(ActionError::new(
                ActionErrorKind::Conflict,
                "desk is locked",
            ));
        }
        Ok(())
    }

    fn execute(
        &self,
        _programmers: &ProgrammerRegistry,
        context: &ActionContext,
        command: &str,
        policy: ExecutionPolicy,
    ) -> ProgrammingExecution {
        let policy = match policy {
            ExecutionPolicy::AtomicProgrammer => ExistingCommandPolicy::AtomicProgrammer,
            ExecutionPolicy::Compatibility => ExistingCommandPolicy::Compatibility,
        };
        match execute_existing_command(
            self.state,
            self.session,
            command,
            self.source,
            context,
            policy,
        ) {
            ExistingCommandOutcome::Accepted {
                applied,
                persistence_warning,
            } => ProgrammingExecution::Accepted {
                applied,
                warning: persistence_warning,
            },
            ExistingCommandOutcome::ChoiceRequired { pending_choice } => {
                match application_choice(pending_choice) {
                    Ok(pending_choice) => ProgrammingExecution::ChoiceRequired { pending_choice },
                    Err(error) => ProgrammingExecution::Rejected { error },
                }
            }
            ExistingCommandOutcome::Rejected { error } => ProgrammingExecution::Rejected { error },
        }
    }

    fn selection_environment(
        &self,
        _context: &ActionContext,
        query: &ProgrammingSelectionQuery,
    ) -> Result<ProgrammingSelectionEnvironment, ActionError> {
        Ok(super::selection_environment::selection_environment(
            self.state, query,
        ))
    }

    fn values_environment(
        &self,
        _context: &ActionContext,
    ) -> Result<ProgrammingValuesEnvironment, ActionError> {
        Ok(super::values_environment::values_environment(self.state))
    }

    fn persist(&self, context: &ActionContext, operation: &'static str) -> Option<String> {
        persist_with_warning(
            self.state,
            self.session,
            self.source,
            context.request_id.as_deref(),
            operation,
        )
    }

    fn capture_programmer_on_preload(&self, _context: &ActionContext) -> bool {
        self.state.configuration.read().preload_programmer_changes
    }

    fn reconcile(
        &self,
        _context: &ActionContext,
        reason: light_application::ProgrammingReconciliation,
    ) {
        let osc = self.source == "osc";
        match reason {
            light_application::ProgrammingReconciliation::SelectionChanged => {
                let source = if osc {
                    "osc_programmer_selection"
                } else {
                    "programmer_selection"
                };
                super::super::reconcile_highlight_selection(self.state, self.session, source);
            }
            light_application::ProgrammingReconciliation::CaptureModeChanged => {
                let source = if osc { "osc_preload" } else { "preload" };
                super::super::reconcile_highlight_capture_mode(self.state, self.session, source);
            }
        }
    }

    fn commit_preload(&self, _context: &ActionContext) -> Result<Option<String>, String> {
        let committed = super::super::commit_preload(self.state, self.session)?;
        Ok(committed
            .get("warnings")
            .and_then(serde_json::Value::as_array)
            .map(|warnings| {
                warnings
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .collect::<Vec<_>>()
                    .join("; ")
            })
            .filter(|warning| !warning.is_empty()))
    }
}
