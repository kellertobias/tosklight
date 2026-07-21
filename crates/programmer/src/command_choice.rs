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
