use super::adapter::{ExistingCommandOutcome, ExistingCommandPolicy, execute_existing_command};
use super::events::persist_with_warning;
use super::wire::application_choice;
use light_application::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, ExecutionPolicy,
    ProgrammingExecution, ProgrammingPorts, ProgrammingPresetRecordingPorts,
    ProgrammingSelectionEnvironment, ProgrammingSelectionQuery, ProgrammingValuesEnvironment,
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

    pub(super) const fn state(&self) -> &'a AppState {
        self.state
    }

    pub(crate) fn record_preset_command(
        &self,
        programmers: &ProgrammerRegistry,
        context: &ActionContext,
        command: &str,
    ) -> Option<ProgrammingExecution> {
        let address = super::adapter::preset_record_address(command)
            .ok()
            .flatten()?;
        let result = self.execute_preset_recording(programmers, context, address, command);
        Some(match result {
            Ok(warning) => ProgrammingExecution::Accepted {
                applied: 1,
                warning,
            },
            Err(error) => {
                super::super::record_command_history(
                    self.state,
                    self.session,
                    command,
                    "rejected",
                    &error,
                    self.source,
                    context.request_id.as_deref(),
                );
                ProgrammingExecution::Rejected { error }
            }
        })
    }

    fn execute_preset_recording(
        &self,
        programmers: &ProgrammerRegistry,
        context: &ActionContext,
        address: light_programmer::PresetAddress,
        raw_command: &str,
    ) -> Result<Option<String>, String> {
        let show_id = self
            .state
            .active_show
            .read()
            .as_ref()
            .map(|entry| entry.id)
            .ok_or("no show is open")?;
        let context = preset_record_context(context);
        let command = light_application::ProgrammingPresetRecordRequest {
            show_id,
            address,
            name: format!("Preset {}", address.storage_key()),
            mode: light_programmer::PresetStoreMode::Overwrite,
            expected_object_revision:
                light_application::ProgrammingPresetRevisionExpectation::Current,
            expected_show_revision: None,
        };
        let result = self
            .state
            .programming
            .record_preset_within_interaction(
                ActionEnvelope {
                    context: context.clone(),
                    command,
                },
                self,
            )
            .map_err(|error| error.message)?;
        if result.replayed {
            return Ok(None);
        }
        clear_command_line(programmers, self.session)?;
        Ok(self.accepted_preset_command(&context, raw_command))
    }

    fn accepted_preset_command(
        &self,
        context: &ActionContext,
        raw_command: &str,
    ) -> Option<String> {
        let warning = persist_with_warning(
            self.state,
            self.session,
            self.source,
            context.request_id.as_deref(),
            "programmer.execute",
        );
        let feedback = warning.as_ref().map_or_else(
            || "Applied to 1 target(s)".to_owned(),
            |warning| format!("Applied to 1 target(s); {warning}"),
        );
        super::super::record_command_history(
            self.state,
            self.session,
            raw_command,
            "accepted",
            &feedback,
            self.source,
            context.request_id.as_deref(),
        );
        warning
    }
}

fn preset_record_context(context: &ActionContext) -> ActionContext {
    if context.request_id.is_some() {
        context.clone()
    } else {
        context
            .clone()
            .with_request_id(format!("preset-record-{}", context.correlation_id))
    }
}

fn clear_command_line(programmers: &ProgrammerRegistry, session: &Session) -> Result<(), String> {
    programmers
        .update_command_line(session.id, |current| (String::new(), current.target, true))
        .ok_or_else(|| "programmer command line does not exist".to_owned())?;
    Ok(())
}

impl ProgrammingPresetRecordingPorts for ServerProgrammingPorts<'_> {
    fn authorize_preset_recording(&self, context: &ActionContext) -> Result<(), ActionError> {
        <Self as ProgrammingPorts>::authorize(self, context)
    }

    fn commit_preset(
        &self,
        context: &ActionContext,
        commit: &light_application::ProgrammingPresetCommit,
    ) -> Result<light_application::ProgrammingPresetCommitResult, ActionError> {
        super::preset_recording_ports::commit(self.state(), context, commit)
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
        programmers: &ProgrammerRegistry,
        context: &ActionContext,
        command: &str,
        policy: ExecutionPolicy,
    ) -> ProgrammingExecution {
        if let Some(outcome) = self.record_preset_command(programmers, context, command) {
            return outcome;
        }
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
        // Every command-line/OSC Programming caller already owns the active-show gate before
        // entering the application boundary. Re-entering it would reject every valid Preload GO.
        let committed = super::super::commit_preload_while_show_stable(self.state, self.session)?;
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
