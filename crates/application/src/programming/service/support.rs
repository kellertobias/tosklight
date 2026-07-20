use super::super::{ProgrammingAction, ProgrammingCommand, ProgrammingOutcome, ProgrammingResult};
use crate::{ActionContext, ActionEnvelope, ActionError, ActionErrorKind};
use light_core::{SessionId, UserId};
use light_programmer::{
    CommandLineReplaceError, CommandLineState, ProgrammerCaptureMode, ProgrammerRegistry,
    ProgrammerSelection, SelectionReplaceError,
};
use std::collections::{HashMap, VecDeque};
use uuid::Uuid;

const REQUEST_CACHE_LIMIT: usize = 4_096;
const COMMAND_LINE_LIMIT: usize = 16 * 1024;

pub(super) fn accepted(
    action: ProgrammingAction,
    applied: Option<usize>,
    warning: Option<String>,
) -> ProgrammingOutcome {
    ProgrammingOutcome::Accepted {
        action,
        applied,
        warning,
    }
}

pub(super) fn command_line(
    programmers: &ProgrammerRegistry,
    session: SessionId,
) -> Result<CommandLineState, ActionError> {
    programmers
        .command_line_state(session)
        .ok_or_else(unknown_programmer)
}

pub(super) fn required_session(
    action: &ActionEnvelope<ProgrammingCommand>,
) -> Result<SessionId, ActionError> {
    action.context.session_id.map(SessionId).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Unauthorized,
            "programming actions require an operator session",
        )
    })
}

pub(super) fn context_session(context: &ActionContext) -> Result<SessionId, ActionError> {
    context.session_id.map(SessionId).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Unauthorized,
            "programming interactions require an operator session",
        )
    })
}

pub(super) fn context_user(context: &ActionContext) -> Result<UserId, ActionError> {
    context.user_id.map(UserId).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Unauthorized,
            "programming interactions require an authenticated user",
        )
    })
}

pub(super) fn validate_command(command: &str) -> Result<(), ActionError> {
    (command.len() <= COMMAND_LINE_LIMIT)
        .then_some(())
        .ok_or_else(|| {
            ActionError::new(
                ActionErrorKind::Invalid,
                "command line must not exceed 16384 bytes",
            )
        })
}

pub(super) fn unknown_programmer() -> ActionError {
    ActionError::new(
        ActionErrorKind::NotFound,
        "programmer command line does not exist",
    )
}

pub(super) fn action_error(error: String) -> ActionError {
    ActionError::new(ActionErrorKind::NotFound, error)
}

pub(super) fn replace_error(error: CommandLineReplaceError) -> ActionError {
    match error {
        CommandLineReplaceError::UnknownSession => unknown_programmer(),
        CommandLineReplaceError::RevisionConflict { expected, actual } => ActionError::new(
            ActionErrorKind::Conflict,
            format!("command-line revision conflict: expected {expected}, actual {actual}"),
        )
        .at_revision(actual),
    }
}

pub(super) fn selection_replace_error(error: SelectionReplaceError) -> ActionError {
    match error {
        SelectionReplaceError::UnknownSession => unknown_programmer(),
        SelectionReplaceError::RevisionConflict { expected, actual } => ActionError::new(
            ActionErrorKind::Conflict,
            format!("selection revision conflict: expected {expected}, actual {actual}"),
        )
        .at_revision(actual),
    }
}

pub(super) struct Snapshot {
    pub(super) command_line: CommandLineState,
    pub(super) selection_revision: u64,
    pub(super) capture_mode: ProgrammerCaptureMode,
    pub(super) values_generation: u64,
    pub(super) preload_values_generation: u64,
}

impl Snapshot {
    pub(super) fn read(
        programmers: &ProgrammerRegistry,
        _desk_id: Uuid,
        session: SessionId,
        user_id: UserId,
    ) -> Result<Self, ActionError> {
        let version = programmers
            .interaction_version(session)
            .ok_or_else(unknown_programmer)?;
        if programmers.user_id(session) != Some(user_id) {
            return Err(ActionError::new(
                ActionErrorKind::Forbidden,
                "the Programmer session does not belong to the authenticated user",
            ));
        }
        Ok(Self {
            command_line: version.command_line,
            selection_revision: version.selection_revision,
            capture_mode: version.capture_mode,
            values_generation: programmers
                .normal_values_generation(session)
                .ok_or_else(unknown_programmer)?,
            preload_values_generation: programmers
                .preload_values_generation(session)
                .ok_or_else(unknown_programmer)?,
        })
    }

    pub(super) fn result(
        self,
        context: crate::ActionContext,
        outcome: ProgrammingOutcome,
        after: Self,
        selection: Option<ProgrammerSelection>,
    ) -> ProgrammingResult {
        ProgrammingResult {
            context,
            outcome,
            command_line_before: self.command_line,
            command_line: after.command_line,
            selection_revision_before: self.selection_revision,
            selection_revision: after.selection_revision,
            selection,
            interaction_event_sequence: None,
            capture_mode_event_sequence: None,
            values_event_sequence: None,
            preload_values_event_sequence: None,
            replayed: false,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ReplayKey {
    desk_id: Uuid,
    session: SessionId,
    request_id: String,
}

struct ReplayEntry {
    command: ProgrammingCommand,
    result: ProgrammingResult,
}

#[derive(Default)]
pub(super) struct ReplayCache {
    entries: HashMap<ReplayKey, ReplayEntry>,
    order: VecDeque<ReplayKey>,
}

impl ReplayCache {
    pub(super) fn get(
        &self,
        desk_id: Uuid,
        session: SessionId,
        request_id: &str,
        command: &ProgrammingCommand,
    ) -> Result<Option<ProgrammingResult>, ActionError> {
        let key = ReplayKey {
            desk_id,
            session,
            request_id: request_id.to_owned(),
        };
        let Some(entry) = self.entries.get(&key) else {
            return Ok(None);
        };
        if entry.command != *command {
            return Err(ActionError::new(
                ActionErrorKind::Conflict,
                "request_id was already used for a different programming operation",
            ));
        }
        let mut replayed = entry.result.clone();
        replayed.replayed = true;
        Ok(Some(replayed))
    }

    pub(super) fn insert(
        &mut self,
        desk_id: Uuid,
        session: SessionId,
        request_id: String,
        command: ProgrammingCommand,
        result: ProgrammingResult,
    ) {
        let key = ReplayKey {
            desk_id,
            session,
            request_id,
        };
        if !self.entries.contains_key(&key) {
            self.order.push_back(key.clone());
        }
        self.entries.insert(key, ReplayEntry { command, result });
        while self.entries.len() > REQUEST_CACHE_LIMIT {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }
}
