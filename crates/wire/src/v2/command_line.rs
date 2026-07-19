//! Stable request, response, outcome, error, and event DTOs for the v2 command-line API.

#[path = "command_line/interaction.rs"]
mod interaction;
#[path = "command_line/selection.rs"]
mod selection;

pub use interaction::*;
pub use selection::*;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

/// Replaces the complete visible desk command line.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ReplaceCommandLineRequest {
    pub text: String,
}

/// Applies one logical key through the shared desk command-line state machine.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct CommandKeyRequest {
    pub key: CommandKey,
    pub phase: CommandKeyPhase,
    pub request_id: String,
}

/// Executes either the desk's current command line or an explicitly supplied complete line.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ExecuteCommandLineRequest {
    #[serde(default)]
    #[ts(optional = nullable)]
    pub command: Option<String>,
    pub request_id: String,
}

/// The desk-local default scope used when an operator starts a command.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommandTarget {
    Fixture,
    Group,
}

/// A logical key name, independent of keyboard layout and transport.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub enum CommandKey {
    #[serde(rename = "SET")]
    Set,
    #[serde(rename = "GRP")]
    Group,
    #[serde(rename = "CUE")]
    Cue,
    #[serde(rename = "UND")]
    Undo,
    #[serde(rename = "CLR")]
    Clear,
    #[serde(rename = "DEL")]
    Delete,
    #[serde(rename = "MOV")]
    Move,
    #[serde(rename = "CPY")]
    Copy,
    #[serde(rename = "TRU")]
    Thru,
    #[serde(rename = "DIV")]
    Divide,
    #[serde(rename = "BACKSPACE")]
    Backspace,
    #[serde(rename = "AT")]
    At,
    #[serde(rename = "ENT")]
    Enter,
    #[serde(rename = "PRE")]
    Preload,
    #[serde(rename = "REC")]
    Record,
    #[serde(rename = "ESC")]
    Escape,
    #[serde(rename = "SHIFT")]
    Shift,
    #[serde(rename = "TIME")]
    Time,
    #[serde(rename = "SELECT")]
    Select,
    #[serde(rename = "+")]
    Plus,
    #[serde(rename = "-")]
    Minus,
    #[serde(rename = ".")]
    Dot,
    #[serde(rename = "0")]
    Digit0,
    #[serde(rename = "1")]
    Digit1,
    #[serde(rename = "2")]
    Digit2,
    #[serde(rename = "3")]
    Digit3,
    #[serde(rename = "4")]
    Digit4,
    #[serde(rename = "5")]
    Digit5,
    #[serde(rename = "6")]
    Digit6,
    #[serde(rename = "7")]
    Digit7,
    #[serde(rename = "8")]
    Digit8,
    #[serde(rename = "9")]
    Digit9,
}

/// Physical key edge. Non-Shift release edges are accepted as idempotent no-ops.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CommandKeyPhase {
    Press,
    Release,
}

/// One authoritative command-line projection and its optimistic-concurrency revision.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct CommandLineResponse {
    pub text: String,
    pub target: CommandTarget,
    pub pristine: bool,
    #[ts(type = "number")]
    pub revision: u64,
    pub pending_choice: Option<CueMoveCopyChoice>,
}

/// Complete response to a mutating command-line operation.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct CommandOperationResponse {
    pub request_id: String,
    #[serde(flatten)]
    pub outcome: CommandOperationOutcome,
    pub command_line: CommandLineResponse,
}

/// Discriminated result of a command-line operation.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum CommandOperationOutcome {
    Accepted {
        action: CommandAcceptedAction,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional = nullable)]
        applied: Option<usize>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional = nullable)]
        warning: Option<String>,
    },
    ChoiceRequired {
        pending_choice: CueMoveCopyChoice,
    },
    Rejected {
        error: String,
    },
}

/// Accepted command-line state transition.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CommandAcceptedAction {
    Edited,
    Executed,
    ClearedCommandLine,
    ClearedPreload,
    ClearedSelection,
    ClearedValues,
    Undone,
    NoChange,
    PreloadEntered,
    PreloadCommitted,
    ShiftPressed,
    ShiftReleased,
    IgnoredRelease,
}

/// Typed body returned when the HTTP request itself cannot be processed.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct CommandErrorResponse {
    pub error: String,
}

/// Choice presented when Cue copy or move semantics would otherwise be ambiguous.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct CueMoveCopyChoice {
    #[serde(rename = "type")]
    pub choice_type: CueMoveCopyChoiceType,
    pub operation: CueTransferOperation,
    pub command: String,
    pub options: Vec<CommandChoiceOption>,
    pub cancel_label: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub enum CueMoveCopyChoiceType {
    #[serde(rename = "cue_move_copy")]
    CueMoveCopy,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CueTransferOperation {
    Copy,
    Move,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct CommandChoiceOption {
    pub id: CommandChoiceOptionId,
    pub label: String,
    pub command: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CommandChoiceOptionId {
    Plain,
    Status,
}

/// Source names currently emitted by the v2 HTTP command-line adapter.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CommandHttpSource {
    Http,
    HttpKey,
}

/// Typed payload for the existing command-line change compatibility event.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct CommandLineChangedEvent {
    pub desk_id: Uuid,
    pub session_id: Uuid,
    pub user_id: Uuid,
    pub text: String,
    pub target: CommandTarget,
    pub pristine: bool,
    #[ts(type = "number")]
    pub revision: u64,
    pub source: CommandHttpSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub redacted: bool,
}

const fn is_false(value: &bool) -> bool {
    !*value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operation_outcome_uses_the_public_discriminator_and_flattened_envelope() {
        let response = CommandOperationResponse {
            request_id: "request-1".into(),
            outcome: CommandOperationOutcome::Accepted {
                action: CommandAcceptedAction::Executed,
                applied: Some(2),
                warning: None,
            },
            command_line: CommandLineResponse {
                text: "FIXTURE".into(),
                target: CommandTarget::Fixture,
                pristine: true,
                revision: 4,
                pending_choice: None,
            },
        };

        let value = serde_json::to_value(response).expect("serialize command response");
        assert_eq!(value["outcome"], "accepted");
        assert_eq!(value["action"], "executed");
        assert_eq!(value["applied"], 2);
        assert!(value.get("warning").is_none());
        assert_eq!(value["command_line"]["revision"], 4);
    }

    #[test]
    fn logical_command_keys_preserve_operator_facing_names() {
        let request = CommandKeyRequest {
            key: CommandKey::Plus,
            phase: CommandKeyPhase::Press,
            request_id: "request-2".into(),
        };
        assert_eq!(
            serde_json::to_value(request).expect("serialize key request"),
            serde_json::json!({
                "key": "+",
                "phase": "press",
                "request_id": "request-2"
            })
        );
    }

    #[test]
    fn cue_transfer_choice_decodes_the_existing_v2_response_shape() {
        let response: CommandOperationResponse = serde_json::from_value(serde_json::json!({
            "request_id": "request-3",
            "outcome": "choice_required",
            "pending_choice": {
                "type": "cue_move_copy",
                "operation": "copy",
                "command": "COPY SET 1 CUE 1 AT SET 2 CUE 2",
                "options": [
                    {
                        "id": "plain",
                        "label": "Plain Copy",
                        "command": "COPY PLAIN SET 1 CUE 1 AT SET 2 CUE 2"
                    },
                    {
                        "id": "status",
                        "label": "Status Copy",
                        "command": "COPY STATUS SET 1 CUE 1 AT SET 2 CUE 2"
                    }
                ],
                "cancel_label": "Cancel"
            },
            "command_line": {
                "text": "COPY SET 1 CUE 1 AT SET 2 CUE 2",
                "target": "FIXTURE",
                "pristine": false,
                "revision": 8,
                "pending_choice": null
            }
        }))
        .expect("decode current choice response");

        let CommandOperationOutcome::ChoiceRequired { pending_choice } = response.outcome else {
            panic!("expected choice-required outcome");
        };
        assert_eq!(pending_choice.operation, CueTransferOperation::Copy);
        assert_eq!(pending_choice.options.len(), 2);
    }
}
