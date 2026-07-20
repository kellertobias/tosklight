use super::{
    ExecutionPolicy, ProgrammingAction, ProgrammingCaptureModeChange, ProgrammingCommand,
    ProgrammingExecution, ProgrammingInteractionChange, ProgrammingOutcome, ProgrammingPorts,
    ProgrammingPreloadPlaybackQueueChange, ProgrammingResult, ProgrammingValuesChange,
};
use crate::{ActionEnvelope, ActionError, EventBus};
use light_core::{SessionId, UserId};
use light_programmer::command_line::{CommandKeyIntent, command_key_intent};
use light_programmer::{HighlightRegistry, ProgrammerRegistry};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

#[path = "service/external.rs"]
mod external;
#[path = "service/lifecycle_publication.rs"]
mod lifecycle_publication;
#[path = "service/preload_values.rs"]
mod preload_values;
#[path = "service/preload_values_replay.rs"]
mod preload_values_replay;
#[path = "service/preset_recording.rs"]
mod preset_recording;
#[path = "service/preset_recording_replay.rs"]
mod preset_recording_replay;
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
#[path = "service/values_replay_fingerprint.rs"]
mod values_replay_fingerprint;
#[path = "service/values_replay_memory.rs"]
mod values_replay_memory;
#[path = "service/values_validation.rs"]
mod values_validation;

use lifecycle_publication::LifecyclePublicationGate;
use preload_values_replay::PreloadValuesReplayCache;
use preset_recording_replay::PresetRecordingReplayCache;
use state::{interaction_change, reconciliation};
use support::{
    ReplayCache, Snapshot, accepted, command_line, context_session, context_user, replace_error,
    required_session, unknown_programmer, validate_command,
};
use values_replay::ValuesReplayCache;

use super::operation::DeskOperationGates;

struct AppliedProgramming {
    result: ProgrammingResult,
    interaction: Option<ProgrammingInteractionChange>,
    capture_mode: Option<ProgrammingCaptureModeChange>,
    values: Option<ProgrammingValuesChange>,
    preload_values: Option<super::ProgrammingPreloadValuesChange>,
    preload_playback_queue: Option<ProgrammingPreloadPlaybackQueueChange>,
}

#[derive(Clone)]
pub struct ProgrammingService {
    pub(super) programmers: ProgrammerRegistry,
    pub(super) desk_gates: DeskOperationGates,
    replay: Arc<Mutex<ReplayCache>>,
    values_replay: Arc<Mutex<ValuesReplayCache>>,
    preload_values_replay: Arc<Mutex<PreloadValuesReplayCache>>,
    preset_recording_replay: Arc<Mutex<PresetRecordingReplayCache>>,
    pub(super) events: EventBus,
    lifecycle_publication: Arc<Mutex<LifecyclePublicationGate>>,
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
            preload_values_replay: Arc::default(),
            preset_recording_replay: Arc::default(),
            events,
            lifecycle_publication: Arc::default(),
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
            let lifecycle_before = self.active_lifecycle_programmer(user_id);
            let mut applied = self.apply(&action, session, user_id, ports)?;
            applied.result.interaction_event_sequence =
                self.publish_interaction(&action.context, applied.interaction);
            applied.result.capture_mode_event_sequence =
                self.publish_capture_mode(&action.context, applied.capture_mode);
            applied.result.values_event_sequence =
                self.publish_values(&action.context, applied.values);
            applied.result.preload_values_event_sequence =
                self.publish_preload_values(&action.context, applied.preload_values);
            applied.result.preload_playback_queue_event_sequence = self
                .publish_preload_playback_queue(&action.context, applied.preload_playback_queue);
            self.publish_lifecycle_for_context(&action.context, lifecycle_before);
            self.remember(&action, session, &applied.result);
            Ok(applied.result)
        })
    }

    fn apply(
        &self,
        action: &ActionEnvelope<ProgrammingCommand>,
        session: SessionId,
        user_id: UserId,
        ports: &dyn ProgrammingPorts,
    ) -> Result<AppliedProgramming, ActionError> {
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
        let preload_values = self.preload_values_change(
            user_id,
            session,
            before.preload_values_generation,
            after.preload_values_generation,
        )?;
        let preload_playback_queue = self.preload_playback_queue_change(
            user_id,
            session,
            before.preload_playback_queue_generation,
            after.preload_playback_queue_generation,
        )?;
        let capture_mode =
            self.capture_mode_change(user_id, before.capture_mode, after.capture_mode);
        Ok(AppliedProgramming {
            result: before.result(action.context.clone(), outcome, after, selection),
            interaction,
            capture_mode,
            values,
            preload_values,
            preload_playback_queue,
        })
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
