//! Stable v2 contracts for dependency-aware, atomic selective show import.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;
use uuid::Uuid;

#[derive(
    Clone, Debug, Deserialize, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize, TS,
)]
#[serde(deny_unknown_fields)]
pub struct SelectiveImportObjectKey {
    pub kind: String,
    pub id: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SelectiveImportConflictResolution {
    KeepDestination,
    ReplaceDestination,
    Duplicate,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SelectiveImportConflictChoice {
    pub key: SelectiveImportObjectKey,
    pub resolution: SelectiveImportConflictResolution,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct SelectiveImportProfileKey {
    pub profile_id: Uuid,
    #[ts(type = "number")]
    pub revision: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SelectiveImportProfileConflictResolution {
    KeepDestination,
    Duplicate,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SelectiveImportProfileConflictChoice {
    pub key: SelectiveImportProfileKey,
    pub resolution: SelectiveImportProfileConflictResolution,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SelectiveImportSelection {
    pub selected_objects: Vec<SelectiveImportObjectKey>,
    #[serde(default)]
    pub conflict_resolutions: Vec<SelectiveImportConflictChoice>,
    #[serde(default)]
    pub profile_conflict_resolutions: Vec<SelectiveImportProfileConflictChoice>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SelectiveImportApplyRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    #[ts(type = "number")]
    pub expected_source_revision: u64,
    #[ts(type = "number")]
    pub expected_target_revision: u64,
    #[serde(flatten)]
    pub selection: SelectiveImportSelection,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct SelectiveImportCatalog {
    pub source_show_id: Uuid,
    pub source_show_name: String,
    #[ts(type = "number")]
    pub source_revision: u64,
    pub objects: Vec<SelectiveImportCatalogObject>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct SelectiveImportCatalogObject {
    pub key: SelectiveImportObjectKey,
    #[ts(type = "number")]
    pub object_revision: u64,
    pub display_name: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SelectiveImportObjectAction {
    ImportPreservingId,
    SkipIdentical,
    KeepDestination,
    ReplaceDestination,
    Duplicate {
        destination: SelectiveImportObjectKey,
    },
    BlockedConflict,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct SelectiveImportObjectPreview {
    pub source: SelectiveImportObjectKey,
    pub destination: SelectiveImportObjectKey,
    pub action: SelectiveImportObjectAction,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SelectiveImportDependencyDisposition {
    Selected,
    Included,
    BoundToDestination,
    Missing,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct SelectiveImportDependency {
    pub owner: SelectiveImportObjectKey,
    pub dependency: SelectiveImportObjectKey,
    pub disposition: SelectiveImportDependencyDisposition,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct SelectiveImportConflict {
    pub key: SelectiveImportObjectKey,
    pub resolution: Option<SelectiveImportConflictResolution>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SelectiveImportProfileAction {
    Copy,
    SkipIdentical,
    KeepDestination,
    Duplicate {
        destination: SelectiveImportProfileKey,
    },
    BlockedConflict,
    Missing,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct SelectiveImportProfilePreview {
    pub source: SelectiveImportProfileKey,
    pub destination: SelectiveImportProfileKey,
    pub action: SelectiveImportProfileAction,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SelectiveImportManagedAssetAction {
    Copy,
    SkipIdentical,
    Missing,
    BlockedConflict,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct SelectiveImportAssetReference {
    pub asset_id: Uuid,
    #[ts(type = "number")]
    pub revision: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct SelectiveImportManagedAssetPreview {
    pub asset: SelectiveImportAssetReference,
    pub action: SelectiveImportManagedAssetAction,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SelectiveImportBlocker {
    EmptySelection,
    SameShow,
    UnsupportedObject {
        key: SelectiveImportObjectKey,
    },
    MissingObject {
        key: SelectiveImportObjectKey,
        required_by: Option<SelectiveImportObjectKey>,
    },
    ObjectConflict {
        key: SelectiveImportObjectKey,
    },
    InvalidResolution {
        key: SelectiveImportObjectKey,
        message: String,
    },
    InvalidProfileResolution {
        key: SelectiveImportProfileKey,
        message: String,
    },
    InvalidDescriptor {
        key: SelectiveImportObjectKey,
        message: String,
    },
    MissingProfile {
        key: SelectiveImportProfileKey,
        required_by: SelectiveImportObjectKey,
    },
    ProfileConflict {
        key: SelectiveImportProfileKey,
    },
    MissingManagedAsset {
        asset: SelectiveImportAssetReference,
    },
    ManagedAssetConflict {
        asset: SelectiveImportAssetReference,
    },
    ReferenceRewrite {
        owner: SelectiveImportObjectKey,
        message: String,
    },
    CandidateInvalid {
        message: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct SelectiveImportPreview {
    pub source_show_id: Uuid,
    pub target_show_id: Uuid,
    #[ts(type = "number")]
    pub source_revision: u64,
    #[ts(type = "number")]
    pub target_revision: u64,
    pub objects: Vec<SelectiveImportObjectPreview>,
    pub dependencies: Vec<SelectiveImportDependency>,
    pub conflicts: Vec<SelectiveImportConflict>,
    pub profiles: Vec<SelectiveImportProfilePreview>,
    pub managed_assets: Vec<SelectiveImportManagedAssetPreview>,
    pub blockers: Vec<SelectiveImportBlocker>,
    pub can_apply: bool,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct SelectiveImportOutcomeObjectChange {
    pub key: SelectiveImportObjectKey,
    #[ts(type = "number")]
    pub object_revision: u64,
    #[ts(type = "unknown")]
    pub body: Value,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct SelectiveImportProfileChange {
    pub source: SelectiveImportProfileKey,
    pub destination: SelectiveImportProfileKey,
    pub digest: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct SelectiveImportOutcome {
    pub request_id: String,
    pub correlation_id: Uuid,
    pub changed: bool,
    pub show_id: Uuid,
    #[ts(type = "number")]
    pub show_revision: u64,
    #[ts(as = "Option<f64>", optional = nullable)]
    pub event_sequence: Option<u64>,
    pub outcomes: Vec<SelectiveImportObjectPreview>,
    pub objects: Vec<SelectiveImportOutcomeObjectChange>,
    pub profiles: Vec<SelectiveImportProfileChange>,
    pub managed_assets: Vec<SelectiveImportAssetReference>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct SelectiveImportErrorResponse {
    pub error: String,
    #[ts(as = "Option<f64>", optional = nullable)]
    pub current_revision: Option<u64>,
    pub retryable: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_rejects_unknown_fields_and_preserves_explicit_choices() {
        let request: SelectiveImportApplyRequest = serde_json::from_value(serde_json::json!({
            "request_id":"select-groups",
            "expected_source_revision":4,
            "expected_target_revision":9,
            "selected_objects":[{"kind":"group","id":"front"}],
            "conflict_resolutions":[{
                "key":{"kind":"group","id":"front"},
                "resolution":"duplicate"
            }]
        }))
        .unwrap();
        assert_eq!(request.selection.selected_objects[0].kind, "group");
        assert_eq!(
            request.selection.conflict_resolutions[0].resolution,
            SelectiveImportConflictResolution::Duplicate
        );
        assert!(
            serde_json::from_value::<SelectiveImportApplyRequest>(serde_json::json!({
                "request_id":"bad",
                "expected_source_revision":1,
                "expected_target_revision":1,
                "selected_objects":[],
                "unknown":true
            }))
            .is_err()
        );
    }
}
