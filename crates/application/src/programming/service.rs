use super::{
    ExecutionPolicy, ProgrammingAction, ProgrammingCommand, ProgrammingExecution,
    ProgrammingOutcome, ProgrammingPorts, ProgrammingResult,
};
use crate::{ActionEnvelope, ActionError};
use light_core::SessionId;
use light_programmer::command_line::{CommandKeyIntent, command_key_intent};
use light_programmer::{CommandLineState, ProgrammerRegistry};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

#[path = "service/support.rs"]
mod support;

use support::{
    ReplayCache, Snapshot, accepted, action_error, command_line, replace_error, required_session,
    unknown_programmer, validate_command,
};

#[derive(Clone)]
pub struct ProgrammingService {
    programmers: ProgrammerRegistry,
    desk_locks: Arc<Mutex<HashMap<Uuid, Arc<Mutex<()>>>>>,
    replay: Arc<Mutex<ReplayCache>>,
}

impl ProgrammingService {
    pub fn new(programmers: ProgrammerRegistry) -> Self {
        Self {
            programmers,
            desk_locks: Arc::default(),
            replay: Arc::default(),
        }
    }

    /// Transitional lock access for legacy adapter operations not yet expressed as service
    /// commands. It preserves one desk order while Stage 3 migrates those operations vertically.
    pub fn desk_lock(&self, desk_id: Uuid) -> Arc<Mutex<()>> {
        let mut locks = self.desk_locks.lock();
        locks.retain(|id, lock| *id == desk_id || Arc::strong_count(lock) > 1);
        Arc::clone(
            locks
                .entry(desk_id)
                .or_insert_with(|| Arc::new(Mutex::new(()))),
        )
    }

    pub fn handle(
        &self,
        action: ActionEnvelope<ProgrammingCommand>,
        ports: &dyn ProgrammingPorts,
    ) -> Result<ProgrammingResult, ActionError> {
        let session = required_session(&action)?;
        let lock = self.desk_lock(action.context.desk_id);
        let _ordered = lock.lock();
        ports.authorize(&action.context)?;
        if let Some(cached) = self.cached(&action, session)? {
            return Ok(cached);
        }
        let result = self.apply(&action, session, ports)?;
        self.remember(&action, session, &result);
        Ok(result)
    }

    fn apply(
        &self,
        action: &ActionEnvelope<ProgrammingCommand>,
        session: SessionId,
        ports: &dyn ProgrammingPorts,
    ) -> Result<ProgrammingResult, ActionError> {
        let before = Snapshot::read(&self.programmers, session)?;
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
                self.replace(session, *expected_revision, text.clone())?
            }
            ProgrammingCommand::Execute { command, policy } => {
                self.execute_command(session, command.as_deref(), *policy, &action.context, ports)?
            }
            ProgrammingCommand::ClearStep => self.clear(session, &action.context, ports)?,
            ProgrammingCommand::Undo => self.undo(session, &action.context, ports)?,
            ProgrammingCommand::Preload { capture_programmer } => {
                self.preload(session, *capture_programmer, &action.context, ports)?
            }
        };
        let after = Snapshot::read(&self.programmers, session)?;
        Ok(before.result(action.context.clone(), outcome, after))
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
            Ok(accepted(ProgrammingAction::Edited, None, None))
        }
    }

    fn replace(
        &self,
        session: SessionId,
        expected_revision: u64,
        text: String,
    ) -> Result<ProgrammingOutcome, ActionError> {
        self.programmers
            .replace_command_line(session, expected_revision, text)
            .map_err(replace_error)?;
        Ok(accepted(ProgrammingAction::Edited, None, None))
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

    fn clear(
        &self,
        session: SessionId,
        context: &crate::ActionContext,
        ports: &dyn ProgrammingPorts,
    ) -> Result<ProgrammingOutcome, ActionError> {
        let command = command_line(&self.programmers, session)?;
        let action = self
            .programmers
            .with_staged_transaction(session, |staged| clear_staged(staged, session, &command))
            .map_err(action_error)?;
        let warning = persist_clear(action, context, ports);
        Ok(accepted(action, None, warning))
    }

    fn undo(
        &self,
        session: SessionId,
        context: &crate::ActionContext,
        ports: &dyn ProgrammingPorts,
    ) -> Result<ProgrammingOutcome, ActionError> {
        let changed = self
            .programmers
            .with_staged_transaction(session, |staged| Ok::<_, String>(staged.undo(session)))
            .map_err(action_error)?;
        let warning = changed
            .then(|| ports.persist(context, "programmer.undo"))
            .flatten();
        Ok(accepted(
            if changed {
                ProgrammingAction::Undone
            } else {
                ProgrammingAction::NoChange
            },
            None,
            warning,
        ))
    }

    fn preload(
        &self,
        session: SessionId,
        capture_programmer: bool,
        context: &crate::ActionContext,
        ports: &dyn ProgrammingPorts,
    ) -> Result<ProgrammingOutcome, ActionError> {
        let programmer = self
            .programmers
            .get(session)
            .ok_or_else(unknown_programmer)?;
        if programmer.blind {
            return Ok(match ports.commit_preload(context) {
                Ok(warning) => accepted(ProgrammingAction::PreloadCommitted, None, warning),
                Err(error) => ProgrammingOutcome::Rejected { error },
            });
        }
        self.programmers
            .with_staged_transaction(session, |staged| {
                staged
                    .arm_preload(session, capture_programmer)
                    .then_some(())
                    .ok_or_else(|| "programmer does not exist".to_owned())
            })
            .map_err(action_error)?;
        let warning = ports.persist(context, "preload.enter");
        Ok(accepted(ProgrammingAction::PreloadEntered, None, warning))
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

fn clear_staged(
    staged: &ProgrammerRegistry,
    session: SessionId,
    command: &CommandLineState,
) -> Result<ProgrammingAction, String> {
    let programmer = staged.get(session).ok_or("programmer does not exist")?;
    let action = if programmer.blind {
        staged.clear_preload_pending(session);
        ProgrammingAction::ClearedPreload
    } else if !programmer.selected.is_empty() {
        staged.select(session, []);
        ProgrammingAction::ClearedSelection
    } else if !programmer.values.is_empty() || !programmer.group_values.is_empty() {
        staged.clear_values(session);
        ProgrammingAction::ClearedValues
    } else if command.pristine {
        ProgrammingAction::NoChange
    } else {
        ProgrammingAction::ClearedCommandLine
    };
    staged
        .update_command_line(session, |current| (String::new(), current.target, true))
        .ok_or("programmer command line does not exist")?;
    Ok(action)
}

fn persist_clear(
    action: ProgrammingAction,
    context: &crate::ActionContext,
    ports: &dyn ProgrammingPorts,
) -> Option<String> {
    let operation = match action {
        ProgrammingAction::ClearedPreload => "programmer.clear_preload",
        ProgrammingAction::ClearedSelection => "programmer.clear_selection",
        ProgrammingAction::ClearedValues => "programmer.clear_values",
        _ => return None,
    };
    ports.persist(context, operation)
}
