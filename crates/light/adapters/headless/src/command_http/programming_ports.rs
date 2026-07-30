use super::adapter::{ExistingCommandOutcome, ExistingCommandPolicy, execute_existing_command};
use super::events::persist_with_warning;
use light_application::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, ExecutionPolicy,
    ProgrammingExecution, ProgrammingPorts, ProgrammingSelectionEnvironment,
    ProgrammingSelectionQuery, ProgrammingValuesEnvironment,
};
use light_programmer::ProgrammerRegistry;

use super::super::{AppState, Session, capability_resources::ProgrammingResource};

pub(crate) trait CommandLineProgrammer {
    fn clear_command_line(&self, session_id: light_core::SessionId) -> bool;
}

impl CommandLineProgrammer for ProgrammerRegistry {
    fn clear_command_line(&self, session_id: light_core::SessionId) -> bool {
        self.update_command_line(session_id, |current| (String::new(), current.target, true))
            .is_some()
    }
}

impl CommandLineProgrammer for ProgrammingResource {
    fn clear_command_line(&self, session_id: light_core::SessionId) -> bool {
        self.update_command_line(session_id, |current| (String::new(), current.target, true))
            .is_some()
    }
}

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

    pub(super) const fn session(&self) -> &'a Session {
        self.session
    }

    pub(super) const fn source(&self) -> &'static str {
        self.source
    }

    pub(crate) fn record_typed_command(
        &self,
        programmers: &dyn CommandLineProgrammer,
        context: &ActionContext,
        command: &str,
        policy: ExecutionPolicy,
    ) -> Option<ProgrammingExecution> {
        self.record_group_command(programmers, context, command)
            .or_else(|| self.record_preset_command(programmers, context, command))
            .or_else(|| self.record_cue_command(programmers, context, command))
            .or_else(|| self.delete_cue_command(programmers, context, command, policy))
            .or_else(|| self.transfer_cue_command(programmers, context, command))
            .or_else(|| self.navigate_cue_command(programmers, context, command, policy))
            .or_else(|| self.speed_group_command(programmers, context, command, policy))
    }

    fn record_group_command(
        &self,
        programmers: &dyn CommandLineProgrammer,
        context: &ActionContext,
        command: &str,
    ) -> Option<ProgrammingExecution> {
        let (group_id, operation) = super::adapter::group_record_command(command)
            .ok()
            .flatten()?;
        let result =
            self.execute_group_recording(programmers, context, group_id, operation, command);
        Some(self.recording_execution(context, command, result))
    }

    pub(crate) fn record_preset_command(
        &self,
        programmers: &dyn CommandLineProgrammer,
        context: &ActionContext,
        command: &str,
    ) -> Option<ProgrammingExecution> {
        let address = super::adapter::preset_record_address(command)
            .ok()
            .flatten()?;
        let result = self.execute_preset_recording(programmers, context, address, command);
        Some(self.recording_execution(
            context,
            command,
            result.map(|(warning, replayed)| (1, warning, replayed)),
        ))
    }

    fn execute_group_recording(
        &self,
        programmers: &dyn CommandLineProgrammer,
        context: &ActionContext,
        group_id: String,
        operation: light_application::ProgrammingGroupRecordOperation,
        raw_command: &str,
    ) -> Result<(usize, Option<String>, bool), String> {
        let show_id = self.active_show_id()?;
        let context = recording_context(context, "group-record");
        let command = light_application::ProgrammingGroupRecordRequest {
            show_id,
            group_id,
            operation,
            expected_object_revision:
                light_application::ProgrammingGroupRevisionExpectation::Current,
            expected_show_revision: None,
        };
        let result = self
            .state
            .programming
            .record_group_within_interaction(
                ActionEnvelope {
                    context: context.clone(),
                    command,
                },
                self,
            )
            .map_err(|error| error.message)?;
        if result.replayed {
            return Ok((result.applied, None, true));
        }
        clear_command_line(programmers, self.session)?;
        let warning = self.accepted_recording_command(&context, raw_command, result.applied);
        Ok((result.applied, warning, false))
    }

    fn execute_preset_recording(
        &self,
        programmers: &dyn CommandLineProgrammer,
        context: &ActionContext,
        address: light_programmer::PresetAddress,
        raw_command: &str,
    ) -> Result<(Option<String>, bool), String> {
        let show_id = self.active_show_id()?;
        let context = recording_context(context, "preset-record");
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
            return Ok((None, true));
        }
        clear_command_line(programmers, self.session)?;
        Ok((
            self.accepted_recording_command(&context, raw_command, 1),
            false,
        ))
    }

    pub(super) fn active_show_id(&self) -> Result<light_core::ShowId, String> {
        self.state
            .active_show
            .current()
            .as_ref()
            .map(|entry| entry.id)
            .ok_or_else(|| "no show is open".to_owned())
    }

    pub(super) fn accepted_recording_command(
        &self,
        context: &ActionContext,
        raw_command: &str,
        applied: usize,
    ) -> Option<String> {
        let warning = persist_with_warning(
            self.state,
            self.session,
            self.source,
            context.request_id.as_deref(),
            "programmer.execute",
        );
        let feedback = warning.as_ref().map_or_else(
            || format!("Applied to {applied} target(s)"),
            |warning| format!("Applied to {applied} target(s); {warning}"),
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

    pub(super) fn recording_execution(
        &self,
        context: &ActionContext,
        command: &str,
        result: Result<(usize, Option<String>, bool), String>,
    ) -> ProgrammingExecution {
        match result {
            Ok((applied, warning, replayed)) => ProgrammingExecution::Accepted {
                applied,
                warning,
                replayed,
            },
            Err(error) => {
                self.rejected_recording_command(context, command, &error);
                ProgrammingExecution::Rejected { error }
            }
        }
    }

    pub(super) fn rejected_recording_command(
        &self,
        context: &ActionContext,
        command: &str,
        error: &str,
    ) {
        super::super::record_command_history(
            self.state,
            self.session,
            command,
            "rejected",
            error,
            self.source,
            context.request_id.as_deref(),
        );
    }
}

pub(super) fn recording_context(context: &ActionContext, prefix: &str) -> ActionContext {
    if context.request_id.is_some() {
        context.clone()
    } else {
        context
            .clone()
            .with_request_id(format!("{prefix}-{}", context.correlation_id))
    }
}

pub(super) fn clear_command_line(
    programmers: &dyn CommandLineProgrammer,
    session: &Session,
) -> Result<(), String> {
    programmers
        .clear_command_line(session.id)
        .then_some(())
        .ok_or_else(|| "programmer command line does not exist".to_owned())
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
        if let Some(outcome) = self.record_typed_command(programmers, context, command, policy) {
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
                replayed,
            } => ProgrammingExecution::Accepted {
                applied,
                warning: persistence_warning,
                replayed,
            },
            ExistingCommandOutcome::ChoiceRequired { pending_choice } => {
                ProgrammingExecution::ChoiceRequired { pending_choice }
            }
            ExistingCommandOutcome::Rejected { error } => ProgrammingExecution::Rejected { error },
        }
    }

    fn selection_environment(
        &self,
        _context: &ActionContext,
        query: &ProgrammingSelectionQuery,
    ) -> Result<ProgrammingSelectionEnvironment, ActionError> {
        super::selection_environment::selection_environment(self.state, query)
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

    fn undo_show_recording(
        &self,
        context: &ActionContext,
        target: &light_application::ProgrammingShowUndoTarget,
    ) -> Result<light_core::Revision, ActionError> {
        let owner = super::super::ProgrammingInstallOwner {
            desk_id: context.desk_id,
            user_id: self.session.user.id,
            gesture: super::super::ProgrammingOwnerGesturePolicy::Preserve,
            highlight: super::super::ProgrammingOwnerHighlightPolicy::DeferToOuterInteraction,
        };
        if !target.portable_objects.is_empty() {
            let ports = super::super::ServerSelectiveImportPorts::with_programming_owner(
                self.state.clone(),
                owner,
            );
            return self.state.active_show.undo_selective_import(
                context,
                &light_application::SelectiveShowImportUndoTarget {
                    show_id: target.show_id,
                    objects: target.portable_objects.clone(),
                },
                &ports,
            );
        }
        let ports = super::super::ServerActiveShowPorts::show_objects_with_programming_owner(
            self.state.clone(),
            owner,
        );
        let action = light_application::ActionEnvelope {
            context: context.clone(),
            command: light_application::UndoActiveShowRecordingCommand {
                show_id: target.show_id,
                objects: target
                    .objects
                    .iter()
                    .map(|object| light_application::UndoActiveShowRecordingObject {
                        kind: object.kind,
                        object_id: object.object_id.clone(),
                        expected_object_revision: object.expected_object_revision,
                        operation: match object.operation {
                            light_application::ProgrammingShowUndoOperation::RestorePrevious => {
                                light_application::UndoActiveShowRecordingOperation::RestorePrevious
                            }
                            light_application::ProgrammingShowUndoOperation::DeleteCreated => {
                                light_application::UndoActiveShowRecordingOperation::DeleteCreated
                            }
                        },
                    })
                    .collect(),
            },
        };
        let result = self.state.active_show.undo_recording(action, &ports)?;
        super::super::emit_migration_object_changes(
            self.state,
            target.show_id,
            &result.migration_changes,
        );
        super::super::emit_migrated_route_changes(
            self.state,
            target.show_id,
            &result.migrated_routes,
        );
        result
            .changes
            .first()
            .map(|change| change.object_revision)
            .ok_or_else(|| {
                ActionError::new(
                    ActionErrorKind::Internal,
                    "show recording undo produced no object change",
                )
            })
    }

    fn capture_programmer_on_preload(&self, _context: &ActionContext) -> bool {
        self.state
            .installation
            .configuration()
            .preload_programmer_changes
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
