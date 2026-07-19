use super::{
    ExecutionPolicy, ProgrammingAction, ProgrammingCommand, ProgrammingExecution,
    ProgrammingInteractionChange, ProgrammingInteractionResult, ProgrammingOutcome,
    ProgrammingPorts, ProgrammingResult, ProgrammingValuesChange,
};
use crate::{ActionContext, ActionEnvelope, ActionError, EventBus};
use light_core::{SessionId, UserId};
use light_programmer::command_line::{CommandKeyIntent, command_key_intent};
use light_programmer::{HighlightRegistry, ProgrammerRegistry};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

#[path = "service/publication.rs"]
mod publication;
#[path = "service/selection.rs"]
mod selection;
#[path = "service/selection_refresh.rs"]
mod selection_refresh;
#[path = "service/state.rs"]
mod state;
#[path = "service/support.rs"]
mod support;
#[path = "service/values.rs"]
mod values;
#[path = "service/values_replay.rs"]
mod values_replay;
#[path = "service/values_validation.rs"]
mod values_validation;

use state::{interaction_change, reconciliation};
use support::{
    ReplayCache, Snapshot, accepted, command_line, context_session, context_user, replace_error,
    required_session, unknown_programmer, validate_command,
};
use values_replay::ValuesReplayCache;

use super::operation::DeskOperationGates;

#[derive(Clone)]
pub struct ProgrammingService {
    pub(super) programmers: ProgrammerRegistry,
    pub(super) desk_gates: DeskOperationGates,
    replay: Arc<Mutex<ReplayCache>>,
    values_replay: Arc<Mutex<ValuesReplayCache>>,
    pub(super) events: EventBus,
    nested_selection_publications: Arc<Mutex<HashMap<uuid::Uuid, u64>>>,
    _highlight: Arc<HighlightRegistry>,
}

impl ProgrammingService {
    pub fn new(
        programmers: ProgrammerRegistry,
        events: EventBus,
        highlight: Arc<HighlightRegistry>,
    ) -> Self {
        Self {
            programmers,
            desk_gates: DeskOperationGates::default(),
            replay: Arc::default(),
            values_replay: Arc::default(),
            events,
            nested_selection_publications: Arc::default(),
            _highlight: highlight,
        }
    }

    pub const fn events(&self) -> &EventBus {
        &self.events
    }

    #[cfg(test)]
    pub(super) const fn highlight_registry(&self) -> &Arc<HighlightRegistry> {
        &self._highlight
    }

    pub fn handle(
        &self,
        action: ActionEnvelope<ProgrammingCommand>,
        ports: &dyn ProgrammingPorts,
    ) -> Result<ProgrammingResult, ActionError> {
        let session = required_session(&action)?;
        let user_id = context_user(&action.context)?;
        self.with_user_and_desk_gate(action.context.desk_id, user_id, || {
            ports.authorize(&action.context)?;
            if let Some(cached) = self.cached(&action, session)? {
                return Ok(cached);
            }
            let (mut result, interaction, values) = self.apply(&action, session, user_id, ports)?;
            result.interaction_event_sequence =
                self.publish_interaction(&action.context, interaction);
            result.values_event_sequence = self.publish_values(&action.context, values);
            self.remember(&action, session, &result);
            Ok(result)
        })
    }

    /// Serializes adapter-owned Programming mutations with typed commands on the same desk.
    ///
    /// Authorization runs under the desk gate. The closure must finish validation, mutation,
    /// persistence, and reconciliation without deleting the session or re-entering this desk's
    /// Programming gate. The boundary captures final state even when the closure returns an error
    /// as its output, then publishes the sparse authoritative change before releasing the gate.
    pub fn run_external_interaction<T>(
        &self,
        context: &ActionContext,
        ports: &dyn ProgrammingPorts,
        operation: impl FnOnce() -> T,
    ) -> Result<ProgrammingInteractionResult<T>, ActionError> {
        let session = context_session(context)?;
        let user_id = context_user(context)?;
        self.with_user_and_desk_gate(context.desk_id, user_id, || {
            ports.authorize(context)?;
            self.capture_external_interaction(context, session, user_id, operation)
        })
    }

    fn capture_external_interaction<T>(
        &self,
        context: &ActionContext,
        session: SessionId,
        user_id: UserId,
        operation: impl FnOnce() -> T,
    ) -> Result<ProgrammingInteractionResult<T>, ActionError> {
        let before = Snapshot::read(&self.programmers, context.desk_id, session, user_id)?;
        let output = operation();
        let after = Snapshot::read(&self.programmers, context.desk_id, session, user_id)?;
        let change =
            interaction_change(&self.programmers, context.desk_id, session, &before, &after);
        let values = self.values_change(
            user_id,
            session,
            before.values_generation,
            after.values_generation,
        )?;
        Ok(ProgrammingInteractionResult {
            output,
            event_sequence: self.publish_interaction(context, change),
            values_event_sequence: self.publish_values(context, values),
        })
    }

    fn apply(
        &self,
        action: &ActionEnvelope<ProgrammingCommand>,
        session: SessionId,
        user_id: UserId,
        ports: &dyn ProgrammingPorts,
    ) -> Result<
        (
            ProgrammingResult,
            Option<ProgrammingInteractionChange>,
            Option<ProgrammingValuesChange>,
        ),
        ActionError,
    > {
        let before = Snapshot::read(&self.programmers, action.context.desk_id, session, user_id)?;
        let outcome = match &action.command {
            ProgrammingCommand::ApplyKey {
                key,
                phase,
                execute_policy,
            } => self.apply_key(
                session,
                *key,
                *phase,
                *execute_policy,
                &action.context,
                ports,
            )?,
            ProgrammingCommand::ReplaceCommandLine {
                text,
                expected_revision,
            } => {
                validate_command(text)?;
                self.replace(
                    session,
                    *expected_revision,
                    text.clone(),
                    &action.context,
                    ports,
                )?
            }
            ProgrammingCommand::Execute { command, policy } => {
                self.execute_command(session, command.as_deref(), *policy, &action.context, ports)?
            }
            ProgrammingCommand::ClearStep => self.clear(session, &action.context, ports)?,
            ProgrammingCommand::Undo => self.undo(session, &action.context, ports)?,
            ProgrammingCommand::Preload { capture_programmer } => {
                self.preload(session, *capture_programmer, &action.context, ports)?
            }
            command @ (ProgrammingCommand::ReplaceSelection { .. }
            | ProgrammingCommand::ApplySelectionGesture { .. }
            | ProgrammingCommand::SelectGroup { .. }
            | ProgrammingCommand::ApplySelectionRule { .. }) => {
                self.apply_selection(session, command, &action.context, ports)?
            }
        };
        let mutated = Snapshot::read(&self.programmers, action.context.desk_id, session, user_id)?;
        if let Some(reason) = reconciliation(&before, &mutated, &outcome) {
            ports.reconcile(&action.context, reason);
        }
        let after = Snapshot::read(&self.programmers, action.context.desk_id, session, user_id)?;
        let interaction = interaction_change(
            &self.programmers,
            action.context.desk_id,
            session,
            &before,
            &after,
        );
        let selection = if action.command.returns_selection() {
            Some(
                self.programmers
                    .selection(session)
                    .ok_or_else(unknown_programmer)?,
            )
        } else {
            None
        };
        let values = self.values_change(
            user_id,
            session,
            before.values_generation,
            after.values_generation,
        )?;
        Ok((
            before.result(action.context.clone(), outcome, after, selection),
            interaction,
            values,
        ))
    }

    fn apply_key(
        &self,
        session: SessionId,
        key: light_programmer::command_line::CommandKey,
        phase: light_programmer::command_line::CommandKeyPhase,
        policy: ExecutionPolicy,
        context: &crate::ActionContext,
        ports: &dyn ProgrammingPorts,
    ) -> Result<ProgrammingOutcome, ActionError> {
        let current = command_line(&self.programmers, session)?;
        match command_key_intent(&current, key, phase) {
            CommandKeyIntent::NoOp => Ok(accepted(ProgrammingAction::IgnoredRelease, None, None)),
            CommandKeyIntent::Shift { pressed } => Ok(accepted(
                if pressed {
                    ProgrammingAction::ShiftPressed
                } else {
                    ProgrammingAction::ShiftReleased
                },
                None,
                None,
            )),
            CommandKeyIntent::Clear => self.clear(session, context, ports),
            CommandKeyIntent::Undo => self.undo(session, context, ports),
            CommandKeyIntent::Preload => self.preload(
                session,
                ports.capture_programmer_on_preload(context),
                context,
                ports,
            ),
            CommandKeyIntent::Edit(edit) => {
                validate_command(&edit.text)?;
                self.edit_or_execute(session, key, phase, policy, context, ports)
            }
        }
    }

    fn edit_or_execute(
        &self,
        session: SessionId,
        key: light_programmer::command_line::CommandKey,
        phase: light_programmer::command_line::CommandKeyPhase,
        policy: ExecutionPolicy,
        context: &crate::ActionContext,
        ports: &dyn ProgrammingPorts,
    ) -> Result<ProgrammingOutcome, ActionError> {
        let mut execute = false;
        self.programmers
            .update_command_line(session, |current| {
                let CommandKeyIntent::Edit(edit) = command_key_intent(current, key, phase) else {
                    unreachable!("an editing key retains its intent under the Programmer gate")
                };
                execute = edit.execute;
                (edit.text, edit.target, edit.pristine)
            })
            .ok_or_else(unknown_programmer)?;
        if execute {
            self.execute_command(session, None, policy, context, ports)
        } else {
            let warning = ports.persist(context, "programmer.command_line");
            Ok(accepted(ProgrammingAction::Edited, None, warning))
        }
    }

    fn replace(
        &self,
        session: SessionId,
        expected_revision: u64,
        text: String,
        context: &crate::ActionContext,
        ports: &dyn ProgrammingPorts,
    ) -> Result<ProgrammingOutcome, ActionError> {
        self.programmers
            .replace_command_line(session, expected_revision, text)
            .map_err(replace_error)?;
        let warning = ports.persist(context, "programmer.command_line");
        Ok(accepted(ProgrammingAction::Edited, None, warning))
    }

    fn execute_command(
        &self,
        session: SessionId,
        supplied: Option<&str>,
        policy: ExecutionPolicy,
        context: &crate::ActionContext,
        ports: &dyn ProgrammingPorts,
    ) -> Result<ProgrammingOutcome, ActionError> {
        if let Some(command) = supplied {
            validate_command(command)?;
        }
        let current = command_line(&self.programmers, session)?;
        let command = supplied.unwrap_or_else(|| current.visible_text());
        let outcome = ports.execute(&self.programmers, context, command, policy);
        if !matches!(outcome, ProgrammingExecution::Accepted { .. }) {
            self.retain_supplied(session, supplied)?;
        }
        Ok(match outcome {
            ProgrammingExecution::Accepted { applied, warning } => {
                accepted(ProgrammingAction::Executed, Some(applied), warning)
            }
            ProgrammingExecution::ChoiceRequired { pending_choice } => {
                ProgrammingOutcome::ChoiceRequired { pending_choice }
            }
            ProgrammingExecution::Rejected { error } => ProgrammingOutcome::Rejected { error },
        })
    }

    fn retain_supplied(
        &self,
        session: SessionId,
        supplied: Option<&str>,
    ) -> Result<(), ActionError> {
        let Some(supplied) = supplied else {
            return Ok(());
        };
        self.programmers
            .update_command_line(session, |current| {
                let pristine = supplied.trim().is_empty()
                    || supplied
                        .trim()
                        .eq_ignore_ascii_case(current.target.as_str());
                (supplied.to_owned(), current.target, pristine)
            })
            .ok_or_else(unknown_programmer)?;
        Ok(())
    }

    fn cached(
        &self,
        action: &ActionEnvelope<ProgrammingCommand>,
        session: SessionId,
    ) -> Result<Option<ProgrammingResult>, ActionError> {
        let Some(request_id) = action.context.request_id.as_deref() else {
            return Ok(None);
        };
        self.replay
            .lock()
            .get(action.context.desk_id, session, request_id, &action.command)
    }

    fn remember(
        &self,
        action: &ActionEnvelope<ProgrammingCommand>,
        session: SessionId,
        result: &ProgrammingResult,
    ) {
        let Some(request_id) = action.context.request_id.clone() else {
            return;
        };
        self.replay.lock().insert(
            action.context.desk_id,
            session,
            request_id,
            action.command.clone(),
            result.clone(),
        );
    }
}
