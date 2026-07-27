use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CueTransferOperation {
    Copy,
    Move,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgrammingChoiceOptionId {
    Plain,
    Status,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProgrammingChoiceOption {
    pub id: ProgrammingChoiceOptionId,
    pub label: String,
    pub command: String,
}

/// One explicit choice retained with the desk-local command interaction after execution.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CueMoveCopyChoice {
    pub choice_id: Uuid,
    pub show_id: Uuid,
    pub show_revision: u64,
    pub operation: CueTransferOperation,
    pub command: String,
    pub options: Vec<ProgrammingChoiceOption>,
    pub cancel_label: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DynamicInstanceChoiceOption {
    pub controller_id: Uuid,
    pub label: String,
    pub command: String,
}

/// Exact running-instance choice retained when a targetless Dynamic command is ambiguous.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DynamicInstanceChoice {
    pub choice_id: Uuid,
    pub show_id: Uuid,
    pub show_revision: u64,
    pub dynamic_id: Uuid,
    pub pool_number: u16,
    pub command: String,
    pub options: Vec<DynamicInstanceChoiceOption>,
    pub cancel_label: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PendingCommandChoice {
    CueMoveCopy(CueMoveCopyChoice),
    DynamicInstance(DynamicInstanceChoice),
}

impl PendingCommandChoice {
    pub const fn cue_move_copy(&self) -> Option<&CueMoveCopyChoice> {
        match self {
            Self::CueMoveCopy(choice) => Some(choice),
            Self::DynamicInstance(_) => None,
        }
    }
}
