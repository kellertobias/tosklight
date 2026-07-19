use axum::{
    Json,
    http::{HeaderValue, header},
    response::{IntoResponse, Response},
};
use light_application::{
    CueMoveCopyChoice as ApplicationCueChoice, CueTransferOperation as ApplicationCueOperation,
    ProgrammingAction, ProgrammingChoiceOption,
    ProgrammingChoiceOptionId as ApplicationChoiceOptionId, ProgrammingOutcome, ProgrammingResult,
};
use light_programmer::CommandLineState;
use light_programmer::command_line::{CommandKey, CommandKeyPhase};
use light_wire::v2::command_line::{
    CommandAcceptedAction, CommandChoiceOption as WireChoiceOption,
    CommandChoiceOptionId as WireChoiceOptionId, CommandKey as WireCommandKey,
    CommandKeyPhase as WireCommandKeyPhase, CommandLineResponse, CommandOperationOutcome,
    CommandOperationResponse, CommandTarget as WireCommandTarget, CueMoveCopyChoice,
    CueMoveCopyChoiceType as WireChoiceType, CueTransferOperation as WireCueOperation,
};
use serde::Serialize;

use super::super::ApiError;

pub(super) fn operation_response(
    request_id: String,
    result: ProgrammingResult,
) -> Result<CommandOperationResponse, ApiError> {
    let outcome = match result.outcome {
        ProgrammingOutcome::Accepted {
            action,
            applied,
            warning,
        } => CommandOperationOutcome::Accepted {
            action: wire_action(action),
            applied,
            warning,
        },
        ProgrammingOutcome::ChoiceRequired { pending_choice } => {
            CommandOperationOutcome::ChoiceRequired {
                pending_choice: wire_choice(pending_choice),
            }
        }
        ProgrammingOutcome::Rejected { error } => CommandOperationOutcome::Rejected { error },
    };
    Ok(CommandOperationResponse {
        request_id,
        outcome,
        command_line: command_line_from_state(result.command_line),
    })
}

const fn wire_action(action: ProgrammingAction) -> CommandAcceptedAction {
    match action {
        ProgrammingAction::Edited => CommandAcceptedAction::Edited,
        ProgrammingAction::Executed => CommandAcceptedAction::Executed,
        ProgrammingAction::ClearedCommandLine => CommandAcceptedAction::ClearedCommandLine,
        ProgrammingAction::ClearedPreload => CommandAcceptedAction::ClearedPreload,
        ProgrammingAction::ClearedSelection => CommandAcceptedAction::ClearedSelection,
        ProgrammingAction::ClearedValues => CommandAcceptedAction::ClearedValues,
        ProgrammingAction::Undone => CommandAcceptedAction::Undone,
        ProgrammingAction::NoChange => CommandAcceptedAction::NoChange,
        ProgrammingAction::PreloadEntered => CommandAcceptedAction::PreloadEntered,
        ProgrammingAction::PreloadCommitted => CommandAcceptedAction::PreloadCommitted,
        ProgrammingAction::ShiftPressed => CommandAcceptedAction::ShiftPressed,
        ProgrammingAction::ShiftReleased => CommandAcceptedAction::ShiftReleased,
        ProgrammingAction::IgnoredRelease => CommandAcceptedAction::IgnoredRelease,
    }
}

pub(super) fn wire_choice(choice: ApplicationCueChoice) -> CueMoveCopyChoice {
    CueMoveCopyChoice {
        choice_type: WireChoiceType::CueMoveCopy,
        operation: match choice.operation {
            ApplicationCueOperation::Copy => WireCueOperation::Copy,
            ApplicationCueOperation::Move => WireCueOperation::Move,
        },
        command: choice.command,
        options: choice
            .options
            .into_iter()
            .map(|option| WireChoiceOption {
                id: match option.id {
                    ApplicationChoiceOptionId::Plain => WireChoiceOptionId::Plain,
                    ApplicationChoiceOptionId::Status => WireChoiceOptionId::Status,
                },
                label: option.label,
                command: option.command,
            })
            .collect(),
        cancel_label: choice.cancel_label,
    }
}

pub(super) fn application_choice(value: serde_json::Value) -> Result<ApplicationCueChoice, String> {
    let choice: CueMoveCopyChoice =
        serde_json::from_value(value).map_err(|error| error.to_string())?;
    Ok(ApplicationCueChoice {
        operation: match choice.operation {
            WireCueOperation::Copy => ApplicationCueOperation::Copy,
            WireCueOperation::Move => ApplicationCueOperation::Move,
        },
        command: choice.command,
        options: choice
            .options
            .into_iter()
            .map(|option| ProgrammingChoiceOption {
                id: match option.id {
                    WireChoiceOptionId::Plain => ApplicationChoiceOptionId::Plain,
                    WireChoiceOptionId::Status => ApplicationChoiceOptionId::Status,
                },
                label: option.label,
                command: option.command,
            })
            .collect(),
        cancel_label: choice.cancel_label,
    })
}

pub(super) fn command_line_from_state(state: CommandLineState) -> CommandLineResponse {
    let text = state.visible_text().to_owned();
    let pending_choice = super::super::pending_cue_transfer_choice(&text).map(|choice| {
        serde_json::from_value::<CueMoveCopyChoice>(choice)
            .expect("the server's Cue transfer choice must satisfy the v2 wire contract")
    });
    CommandLineResponse {
        text,
        target: match state.target {
            light_programmer::CommandTarget::Fixture => WireCommandTarget::Fixture,
            light_programmer::CommandTarget::Group => WireCommandTarget::Group,
        },
        pristine: state.pristine,
        revision: state.revision,
        pending_choice,
    }
}

pub(super) const fn command_key_phase(phase: WireCommandKeyPhase) -> CommandKeyPhase {
    match phase {
        WireCommandKeyPhase::Press => CommandKeyPhase::Press,
        WireCommandKeyPhase::Release => CommandKeyPhase::Release,
    }
}

pub(super) const fn command_key(key: WireCommandKey) -> CommandKey {
    match key {
        WireCommandKey::Set => CommandKey::Set,
        WireCommandKey::Group => CommandKey::Group,
        WireCommandKey::Cue => CommandKey::Cue,
        WireCommandKey::Undo => CommandKey::Undo,
        WireCommandKey::Clear => CommandKey::Clear,
        WireCommandKey::Delete => CommandKey::Delete,
        WireCommandKey::Move => CommandKey::Move,
        WireCommandKey::Copy => CommandKey::Copy,
        WireCommandKey::Thru => CommandKey::Thru,
        WireCommandKey::Divide => CommandKey::Divide,
        WireCommandKey::Backspace => CommandKey::Backspace,
        WireCommandKey::At => CommandKey::At,
        WireCommandKey::Enter => CommandKey::Enter,
        WireCommandKey::Preload => CommandKey::Preload,
        WireCommandKey::Record => CommandKey::Record,
        WireCommandKey::Escape => CommandKey::Escape,
        WireCommandKey::Shift => CommandKey::Shift,
        WireCommandKey::Time => CommandKey::Time,
        WireCommandKey::Select => CommandKey::Select,
        WireCommandKey::Plus => CommandKey::Plus,
        WireCommandKey::Minus => CommandKey::Minus,
        WireCommandKey::Dot => CommandKey::Dot,
        WireCommandKey::Digit0 => CommandKey::Digit(0),
        WireCommandKey::Digit1 => CommandKey::Digit(1),
        WireCommandKey::Digit2 => CommandKey::Digit(2),
        WireCommandKey::Digit3 => CommandKey::Digit(3),
        WireCommandKey::Digit4 => CommandKey::Digit(4),
        WireCommandKey::Digit5 => CommandKey::Digit(5),
        WireCommandKey::Digit6 => CommandKey::Digit(6),
        WireCommandKey::Digit7 => CommandKey::Digit(7),
        WireCommandKey::Digit8 => CommandKey::Digit(8),
        WireCommandKey::Digit9 => CommandKey::Digit(9),
    }
}

pub(super) fn with_etag<T>(value: T) -> Response
where
    T: Serialize + HasCommandRevision,
{
    let revision = value.command_revision();
    let mut response = Json(value).into_response();
    response.headers_mut().insert(
        header::ETAG,
        HeaderValue::from_str(&format!("\"{revision}\""))
            .expect("a numeric command revision always forms a valid ETag"),
    );
    response
}

pub(super) trait HasCommandRevision {
    fn command_revision(&self) -> u64;
}

impl HasCommandRevision for CommandLineResponse {
    fn command_revision(&self) -> u64 {
        self.revision
    }
}

impl HasCommandRevision for CommandOperationResponse {
    fn command_revision(&self) -> u64 {
        self.command_line.revision
    }
}
