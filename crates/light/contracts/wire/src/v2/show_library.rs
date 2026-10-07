//! Typed contracts for the cross-show library and lifecycle service.
//!
//! Show-library writes are intent-shaped and carry a client request identity. The server
//! accepts and logs unknown fields through its tolerant JSON extractor.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use super::runtime::RuntimeShowEntry;

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ShowLibrarySnapshot {
    pub shows: Vec<ShowLibraryEntry>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ShowLibraryEntry {
    #[serde(flatten)]
    pub show: RuntimeShowEntry,
    #[serde(default)]
    pub description: String,
    pub revisions: Vec<ShowLibraryRevision>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ShowLibraryRevision {
    pub show_id: Uuid,
    #[ts(type = "number")]
    pub revision: u64,
    pub name: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ShowLibraryActionRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    pub action: ShowLibraryAction,
}

/// Loading from a visualizer is an operator asking for that rig now, so it opens unless the
/// caller says otherwise.
fn opens_by_default() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ShowLibraryAction {
    Create {
        name: String,
        #[serde(default)]
        data_base64: Option<String>,
        #[serde(default)]
        overwrite: bool,
    },
    SaveCopy {
        #[serde(default)]
        source_show_id: Option<Uuid>,
        #[serde(default)]
        data_base64: Option<String>,
        name: String,
        root_id: String,
        path: String,
        #[serde(default)]
        is_base_show: bool,
    },
    ExportMvrFile {
        #[serde(default)]
        show_id: Option<Uuid>,
        #[serde(default)]
        data_base64: Option<String>,
        name: String,
        root_id: String,
        path: String,
    },
    SaveCopyToPeer {
        instance: String,
        source_show_id: Uuid,
        name: String,
        root_id: String,
        path: String,
        is_base_show: bool,
    },
    ExportMvrToPeer {
        instance: String,
        show_id: Uuid,
        name: String,
        root_id: String,
        path: String,
    },
    SetBaseShow {
        show_id: Uuid,
        is_base_show: bool,
    },
    SetDescription {
        show_id: Uuid,
        description: String,
    },
    CreateFromBase {
        show_id: Uuid,
        name: String,
    },
    PrepareRevision {
        show_id: Uuid,
        #[ts(type = "number")]
        revision: u64,
    },
    ImportFromDesk {
        instance: String,
        show_id: Uuid,
        #[serde(default)]
        #[ts(type = "number | null")]
        revision: Option<u64>,
        #[serde(default = "crate::v2::show_library::opens_by_default")]
        open: bool,
    },
    Open {
        show_id: Uuid,
        #[serde(default)]
        transition: ShowOpenTransition,
        #[serde(default)]
        transition_millis: Option<u64>,
    },
    OpenDefault {
        #[serde(default)]
        transition: ShowOpenTransition,
        #[serde(default)]
        transition_millis: Option<u64>,
    },
    Rollback {
        #[serde(default)]
        transition: ShowOpenTransition,
        #[serde(default)]
        transition_millis: Option<u64>,
    },
    Rename {
        show_id: Uuid,
        name: String,
    },
    Overwrite {
        source_show_id: Uuid,
        destination_show_id: Uuid,
    },
    /// Save an edited portable document back to its originating library entry.
    UpdateDocument {
        destination_show_id: Uuid,
        #[ts(type = "number")]
        expected_revision: u64,
        data_base64: String,
    },
    SaveRevision {
        show_id: Uuid,
        name: String,
    },
    OpenRevision {
        show_id: Uuid,
        #[ts(type = "number")]
        revision: u64,
        #[serde(default)]
        transition: ShowOpenTransition,
        #[serde(default)]
        transition_millis: Option<u64>,
    },
    /// Load the document a Viz editor on the network currently has open.
    ///
    /// The desk fetches it, imports it as an ordinary show, and opens it. It is a copy: the
    /// editor keeps its own document, and patching either side afterwards does not reach the
    /// other. The instance is the one discovery reported; a peer that has gone is a clear 404
    /// rather than a silently different show.
    ImportFromVisualizer {
        instance: String,
        #[serde(default = "crate::v2::show_library::opens_by_default")]
        open: bool,
    },
    ApplyMvr {
        token: Uuid,
        destination: MvrImportDestination,
        #[serde(default)]
        resolutions: Vec<MvrImportResolution>,
    },
}

#[derive(Clone, Copy, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ShowOpenTransition {
    HoldCurrent,
    TimedFade,
    #[default]
    SafeBlackout,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MvrImportDestination {
    NewShow {
        name: String,
        #[serde(default)]
        open_after_import: bool,
    },
    ExistingShow {
        show_id: Uuid,
    },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct MvrImportResolution {
    pub fixture_id: Uuid,
    pub action: MvrImportResolutionAction,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MvrImportResolutionAction {
    Import,
    Skip,
    ImportUnpatched,
    Replace,
    Address { universe: u16, address: u16 },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ShowLibraryActionOutcome {
    pub request_id: String,
    pub replayed: bool,
    pub result: ShowLibraryActionResult,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ShowLibraryActionResult {
    FileSaved {
        root_id: String,
        path: String,
    },
    /// The active show was exported as an MVR archive; `summary` describes what was written.
    MvrExported {
        root_id: String,
        path: String,
        summary: MvrExportSummary,
    },
    Show {
        show: RuntimeShowEntry,
    },
    DocumentUpdated {
        show: RuntimeShowEntry,
        #[ts(type = "number")]
        document_revision: u64,
    },
    Revision {
        revision: ShowLibraryRevision,
    },
    MvrApply {
        result: MvrApplyOutcome,
    },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct MvrApplyOutcome {
    pub show: RuntimeShowEntry,
    pub imported_fixtures: usize,
    pub unresolved_fixtures: usize,
    pub imported_scenery: usize,
    pub opened: bool,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct MvrImportPreview {
    pub token: Uuid,
    pub fixtures: Vec<MvrPreviewFixture>,
    pub scenery: usize,
    pub missing_profiles: Vec<String>,
    pub warnings: Vec<String>,
    pub address_conflicts: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct MvrPreviewFixture {
    pub uuid: Uuid,
    pub name: String,
    pub gdtf_spec: String,
    pub gdtf_mode: String,
    pub universe: Option<u16>,
    pub address: Option<u16>,
    pub matched: bool,
}

/// What one MVR export actually wrote, and everything it could not carry.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct MvrExportSummary {
    pub fixtures: usize,
    pub scenery: usize,
    pub embedded_profiles: usize,
    pub missing_profiles: Vec<String>,
    pub omitted: Vec<String>,
    pub warnings: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_requests_are_tolerant_and_known_fields_stay_typed() {
        let request: ShowLibraryActionRequest = serde_json::from_value(serde_json::json!({
            "request_id": "show-open-1",
            "action": {
                "type": "open",
                "show_id": Uuid::nil(),
                "transition": "hold_current",
                "future_hint": true
            },
            "future_root": true
        }))
        .unwrap();
        assert!(matches!(request.action, ShowLibraryAction::Open { .. }));

        let error = serde_json::from_value::<ShowLibraryActionRequest>(serde_json::json!({
            "request_id": "show-open-2",
            "action": {"type": "open", "show_id": 7}
        }))
        .unwrap_err();
        assert!(error.to_string().contains("UUID"));
    }
}
