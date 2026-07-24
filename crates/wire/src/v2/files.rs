//! Typed transport contracts for File Manager mutations.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum FileInputAction {
    Rename,
    Copy,
    Move,
    Delete,
}

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum FileInputOrigin {
    Pending,
    Toolbar,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct FileInputClaimRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    pub instance_id: String,
    pub action: FileInputAction,
    pub origin: FileInputOrigin,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct FileInputReleaseRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    pub instance_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct NativeNoteUpdateRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    pub path: String,
    pub note: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct TextDocumentUpdateRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    pub path: String,
    pub text: String,
    pub revision: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum FileOperationKind {
    CreateFile,
    CreateFolder,
    Rename,
    Copy,
    Move,
    Trash,
    Delete,
}

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum FileConflictChoice {
    Replace,
    KeepBoth,
    Skip,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct FileOperationRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    pub operation: FileOperationKind,
    pub sources: Vec<String>,
    pub destination: Option<String>,
    pub destination_root_id: Option<String>,
    pub name: Option<String>,
    #[serde(default)]
    pub replace: bool,
    pub conflict: Option<FileConflictChoice>,
    #[serde(default)]
    pub apply_to_all: bool,
}
