//! Typed contracts for the desk-wide fixture library.
//!
//! Fixture profile and legacy-definition bodies stay opaque at this lowest-level wire crate:
//! their domain schemas are owned by `light-fixture`, while this module owns the versioned
//! transport envelope, intent discriminants, request identity, and outcomes.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;
use uuid::Uuid;

pub use super::attribute_configuration::AttributeValueType;

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct FixtureDefinitionsSnapshot {
    #[ts(type = "unknown[]")]
    pub definitions: Vec<Value>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct FixtureProfilesSnapshot {
    #[ts(type = "unknown[]")]
    pub profiles: Vec<Value>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct FixtureLibraryWarningsSnapshot {
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct FixtureProfileRevisionsSnapshot {
    #[ts(type = "unknown[]")]
    pub profiles: Vec<Value>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct GelCatalogsSnapshot {
    pub catalogs: Vec<GelCatalog>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct GelCatalog {
    pub id: Uuid,
    pub revision: u32,
    pub name: String,
    pub entries: Vec<GelCatalogEntry>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct GelCatalogEntry {
    pub id: Uuid,
    pub number: String,
    pub name: String,
    pub display_srgb: String,
    pub visualizer_srgb: String,
}

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GelCatalogImportTarget {
    Create {
        catalog_id: Uuid,
    },
    Update {
        catalog_id: Uuid,
        expected_revision: u32,
    },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct GelCatalogImportPreviewRequest {
    pub target: GelCatalogImportTarget,
    pub catalog_name: String,
    pub csv_base64: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct GelCatalogImportConfirmRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    pub target: GelCatalogImportTarget,
    pub catalog_name: String,
    pub csv_base64: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct GelCatalogImportConfirmOutcome {
    pub request_id: String,
    pub replayed: bool,
    pub catalog: GelCatalog,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct GelCatalogImportPreview {
    pub catalog_id: Uuid,
    pub catalog_name: String,
    pub catalog_name_changed: bool,
    pub additions: Vec<GelCatalogImportAddition>,
    pub replacements: Vec<GelCatalogImportReplacement>,
    pub unchanged: Vec<GelCatalogEntry>,
    pub conflicts: Vec<GelCatalogImportConflict>,
    pub invalid_rows: Vec<GelCatalogCsvError>,
    pub confirmable: bool,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct GelCatalogImportAddition {
    pub entry: GelCatalogEntry,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct GelCatalogImportReplacement {
    pub previous: GelCatalogEntry,
    pub replacement: GelCatalogEntry,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GelCatalogImportConflict {
    CatalogIdentityAlreadyExists {
        catalog_id: Uuid,
    },
    CatalogMissing {
        catalog_id: Uuid,
    },
    RevisionMismatch {
        catalog_id: Uuid,
        expected: u32,
        current: u32,
    },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct GelCatalogCsvError {
    pub row: usize,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct FixtureLibraryActionRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    pub action: FixtureLibraryAction,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FixtureLibraryAction {
    SaveProfile {
        #[ts(type = "unknown")]
        profile: Value,
        #[ts(type = "number")]
        expected_revision: u64,
    },
    DeleteProfileRevision {
        profile_id: Uuid,
        revision: u32,
    },
    ImportPackage {
        package_base64: String,
        #[serde(default)]
        attribute_mappings: Vec<FixtureAttributeMapping>,
    },
    AttachGdtf {
        profile_id: Uuid,
        revision: u32,
        source_base64: String,
    },
    RememberSourceMapping {
        source_format: String,
        source_attribute: String,
        target_attribute: Option<String>,
    },
    SaveDefinition {
        #[ts(type = "unknown")]
        definition: Value,
    },
    DeleteDefinitionRevision {
        definition_id: Uuid,
        revision: u32,
    },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct FixtureLibraryActionOutcome {
    pub request_id: String,
    pub replayed: bool,
    pub result: FixtureLibraryActionResult,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct FixtureAttributeMapping {
    pub source_attribute: String,
    pub target_attribute: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct FixtureSourceMapping {
    pub source_format: String,
    pub source_attribute: String,
    pub target_attribute: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct FixtureSourceMappingsSnapshot {
    pub mappings: Vec<FixtureSourceMapping>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct FixtureImportRequirement {
    pub attribute: String,
    pub value_type: AttributeValueType,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FixtureLibraryActionResult {
    Profile {
        profile_id: Uuid,
        revision: u32,
    },
    Definition {
        definition_id: Uuid,
        revision: u32,
    },
    Deleted {
        resource: FixtureLibraryResource,
        id: Uuid,
        revision: u32,
    },
    GdtfAttached {
        profile_id: Uuid,
        revision: u32,
    },
    SourceMapping {
        mapping: Option<FixtureSourceMapping>,
    },
    ImportRequired {
        unknown_attributes: Vec<FixtureImportRequirement>,
    },
    GelCatalogImported {
        catalog: GelCatalog,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum FixtureLibraryResource {
    Profile,
    Definition,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_envelopes_are_tolerant_while_known_fields_stay_typed() {
        let request: FixtureLibraryActionRequest = serde_json::from_value(serde_json::json!({
            "request_id": "fixture-save-1",
            "action": {
                "type": "save_profile",
                "profile": {"id": "opaque-domain-body"},
                "expected_revision": 0,
                "future_hint": true
            },
            "future_root": true
        }))
        .unwrap();
        assert!(matches!(
            request.action,
            FixtureLibraryAction::SaveProfile {
                expected_revision: 0,
                ..
            }
        ));

        let error = serde_json::from_value::<FixtureLibraryActionRequest>(serde_json::json!({
            "request_id": "fixture-delete-1",
            "action": {
                "type": "delete_profile_revision",
                "profile_id": 7,
                "revision": 1
            }
        }))
        .unwrap_err();
        assert!(error.to_string().contains("UUID"));
    }
}
