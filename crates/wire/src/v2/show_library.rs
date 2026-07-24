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
    Show { show: RuntimeShowEntry },
    Revision { revision: ShowLibraryRevision },
    MvrApply { result: MvrApplyOutcome },
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

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct MvrExportPreview {
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
