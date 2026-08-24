//! Show-owned command Macro object, validation, and one-shot runtime contracts.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct MacroPresentation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub icon: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct MacroDefinition {
    pub id: Uuid,
    pub number: u16,
    pub name: String,
    pub source: String,
    #[serde(default)]
    pub presentation: MacroPresentation,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum MacroLineStatus {
    Valid,
    Invalid,
    InteractionRequired,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct MacroToken {
    pub start: u32,
    pub end: u32,
    pub kind: MacroTokenKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub expansion: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum MacroTokenKind {
    Keyword,
    Target,
    Operator,
    Address,
    Number,
    Timing,
    Comment,
    Text,
    Definition,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct MacroLineDiagnostic {
    pub line: u32,
    pub status: MacroLineStatus,
    pub message: String,
    pub tokens: Vec<MacroToken>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct MacroValidationRequest {
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub cursor: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct MacroSuggestion {
    pub label: String,
    pub insert_text: String,
    pub detail: String,
    pub replace_start: u32,
    pub replace_end: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct MacroValidation {
    pub valid: bool,
    pub diagnostics: Vec<MacroLineDiagnostic>,
    #[serde(default)]
    pub suggestions: Vec<MacroSuggestion>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct MacroObjectActionRequest {
    pub request_id: String,
    pub action: MacroObjectAction,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MacroObjectAction {
    Create {
        definition: MacroDefinition,
    },
    Update {
        macro_id: Uuid,
        #[ts(type = "number")]
        expected_revision: u64,
        patch: MacroPatch,
    },
    Copy {
        source_macro_id: Uuid,
        #[ts(type = "number")]
        expected_revision: u64,
        pool_number: u16,
    },
    Delete {
        macro_id: Uuid,
        #[ts(type = "number")]
        expected_revision: u64,
    },
}

#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct MacroPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub number: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub presentation: Option<MacroPresentation>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct MacroObjectActionOutcome {
    pub request_id: String,
    pub replayed: bool,
    pub show_id: Uuid,
    #[ts(type = "number")]
    pub show_revision: u64,
    pub macro_id: Uuid,
    #[ts(type = "number")]
    pub object_revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub definition: Option<MacroDefinition>,
    pub deleted: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct MacroRunActionRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(as = "Option<f64>", optional = nullable)]
    pub source_revision: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub trigger: Option<MacroTrigger>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct MacroRunLineActionRequest {
    #[ts(type = "number")]
    pub source_revision: u64,
    pub line: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MacroLiveAction {
    Run {
        macro_id: Uuid,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(as = "Option<f64>", optional = nullable)]
        source_revision: Option<u64>,
        trigger: MacroTrigger,
    },
    RunLine {
        macro_id: Uuid,
        #[ts(type = "number")]
        source_revision: u64,
        line: u32,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct MacroCancelActionRequest {
    pub execution_id: Uuid,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct MacroRunLineUndoOutcome {
    pub execution_id: Uuid,
    pub changed: bool,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MacroTrigger {
    Pool,
    Editor,
    Playback { playback_number: u16 },
    CommandLine,
    Http,
    WebSocket,
    Osc,
    Hardware,
    Schedule,
    Timecode,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum MacroExecutionState {
    Queued,
    Validating,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct MacroExecutionSnapshot {
    pub execution_id: Uuid,
    pub macro_id: Uuid,
    pub macro_number: u16,
    pub macro_name: String,
    #[ts(type = "number")]
    pub source_revision: u64,
    pub desk_id: Uuid,
    pub session_id: Uuid,
    pub state: MacroExecutionState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub line: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub statement: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub message: Option<String>,
    pub trigger: MacroTrigger,
    pub started_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub finished_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct MacroRuntimeSnapshot {
    pub desk_id: Uuid,
    pub active: Vec<MacroExecutionSnapshot>,
    pub recent: Vec<MacroExecutionSnapshot>,
}
