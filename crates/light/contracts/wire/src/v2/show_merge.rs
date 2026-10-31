//! Merging an offline show copy back into the open show (TL-858).
//!
//! The copy is a library show, the operand of `/api/v2/show-merges/{copy_show_id}/…`; the merge
//! target is always the loaded show. A preview computes the three-way merge without writing; an
//! apply re-computes it with the operator's decisions and commits it as one undoable step.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ShowMergeChoice {
    /// Keep what the open show has.
    Mine,
    /// Take what the offline copy has.
    Theirs,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ShowMergeResolution {
    pub conflict_id: String,
    pub choice: ShowMergeChoice,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ShowMergePreviewRequest {
    #[serde(default)]
    pub resolutions: Vec<ShowMergeResolution>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ShowMergeApplyRequest {
    pub request_id: String,
    /// The show revision the operator previewed; a changed show must be previewed again.
    #[ts(type = "number")]
    pub expected_target_revision: u64,
    #[serde(default)]
    pub resolutions: Vec<ShowMergeResolution>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ShowMergeLineage {
    pub copy_show_id: Uuid,
    pub copy_name: String,
    pub source_show_id: Uuid,
    pub source_show_name: String,
    pub taken_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ShowMergeKindSummary {
    pub kind: String,
    pub added: u32,
    pub changed: u32,
    pub deleted: u32,
    pub conflicts: u32,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ShowMergeConflict {
    pub id: String,
    pub kind: String,
    pub object_id: String,
    /// The number the operator knows the object by (pool, fixture or Macro number), when it has one.
    pub object_number: Option<String>,
    /// The object's name in either version, when it has one.
    pub object_name: Option<String>,
    /// Where inside the object, outermost first; empty when the whole object conflicts.
    pub path: Vec<String>,
    #[ts(type = "unknown")]
    pub base: Option<Value>,
    #[ts(type = "unknown")]
    pub mine: Option<Value>,
    #[ts(type = "unknown")]
    pub theirs: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ShowMergeRenumber {
    pub kind: String,
    pub from: String,
    pub to: String,
    pub name: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ShowMergeMacroWarning {
    pub macro_id: String,
    pub name: String,
    pub mentions: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ShowMergePreview {
    pub lineage: ShowMergeLineage,
    #[ts(type = "number")]
    pub target_revision: u64,
    pub summary: Vec<ShowMergeKindSummary>,
    /// Objects both sides changed that merged without a conflict.
    pub auto_merged: u32,
    pub write_count: u32,
    pub conflicts: Vec<ShowMergeConflict>,
    pub renumbers: Vec<ShowMergeRenumber>,
    pub macro_warnings: Vec<ShowMergeMacroWarning>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ShowMergeApplyResult {
    pub changed: bool,
    #[ts(type = "number")]
    pub show_revision: u64,
    /// The named revision saved before the merge, so the show can be restored to it.
    #[ts(type = "number")]
    pub safety_revision: u64,
    pub written_objects: u32,
}
