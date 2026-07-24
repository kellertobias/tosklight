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
    },
    AttachGdtf {
        profile_id: Uuid,
        revision: u32,
        source_base64: String,
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
