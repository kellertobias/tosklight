use super::{ProgrammingService, support};
use crate::{
    ActionContext, ActionError,
    programming::{
        ProgrammingAction, ProgrammingInteractionChange, ProgrammingOutcome, ProgrammingPorts,
        ProgrammingReconciliation,
    },
};
use light_core::SessionId;
use light_programmer::{CommandLineState, ProgrammerRegistry};
use support::{Snapshot, accepted, action_error, command_line, unknown_programmer};

impl ProgrammingService {
    pub(super) fn clear(
        &self,
        session: SessionId,
        context: &ActionContext,
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

    pub(super) fn undo(
        &self,
        session: SessionId,
        context: &ActionContext,
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

    pub(super) fn preload(
        &self,
        session: SessionId,
        capture_programmer: bool,
        context: &ActionContext,
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
}

pub(super) fn reconciliation(
    before: &Snapshot,
    mutated: &Snapshot,
    _outcome: &ProgrammingOutcome,
) -> Option<ProgrammingReconciliation> {
    if before.capture_mode != mutated.capture_mode {
        return Some(ProgrammingReconciliation::CaptureModeChanged);
    }
    (before.selection_revision != mutated.selection_revision)
        .then_some(ProgrammingReconciliation::SelectionChanged)
}

pub(super) fn interaction_change(
    programmers: &ProgrammerRegistry,
    desk_id: uuid::Uuid,
    session: SessionId,
    before: &Snapshot,
    after: &Snapshot,
) -> Option<ProgrammingInteractionChange> {
    let command_line =
        (before.command_line != after.command_line).then(|| after.command_line.clone());
    let selection = (before.selection_revision != after.selection_revision)
        .then(|| programmers.selection(session).unwrap_or_default());
    ProgrammingInteractionChange::from_components(desk_id, command_line, selection)
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
    context: &ActionContext,
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
