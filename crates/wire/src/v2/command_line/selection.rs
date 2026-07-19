use super::{ProgrammerSelectionProjection, ProgrammerSelectionRule};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingSelectionActionRequest {
    pub request_id: String,
    #[serde(flatten)]
    pub action: ProgrammingSelectionAction,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "action", rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub enum ProgrammingSelectionAction {
    Replace {
        fixtures: Vec<Uuid>,
        #[ts(type = "number")]
        expected_revision: u64,
    },
    Gesture {
        source: ProgrammingSelectionGestureSource,
        remove: bool,
    },
    SelectGroup {
        group_id: String,
        frozen: bool,
        rule: ProgrammerSelectionRule,
        #[ts(type = "number")]
        expected_revision: u64,
    },
    ApplyRule {
        rule: ProgrammerSelectionRule,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub enum ProgrammingSelectionGestureSource {
    Fixture { fixture_id: Uuid },
    LiveGroup { group_id: String },
    DereferencedGroup { group_id: String },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingSelectionActionOutcome {
    pub request_id: String,
    pub correlation_id: Uuid,
    pub action: ProgrammingSelectionAcceptedAction,
    #[ts(type = "number")]
    pub applied: u64,
    pub selection: ProgrammerSelectionProjection,
    #[ts(type = "number")]
    pub event_sequence: u64,
    pub replayed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub warning: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProgrammingSelectionAcceptedAction {
    Replaced,
    GestureApplied,
    GroupSelected,
    RuleApplied,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_request_is_a_readable_discriminated_contract() {
        let request = ProgrammingSelectionActionRequest {
            request_id: "selection-1".into(),
            action: ProgrammingSelectionAction::Gesture {
                source: ProgrammingSelectionGestureSource::LiveGroup {
                    group_id: "7".into(),
                },
                remove: true,
            },
        };

        assert_eq!(
            serde_json::to_value(request).unwrap(),
            serde_json::json!({
                "request_id": "selection-1",
                "action": "gesture",
                "source": { "type": "live_group", "group_id": "7" },
                "remove": true,
            })
        );
    }

    #[test]
    fn relative_actions_do_not_require_an_absolute_revision() {
        let gesture = serde_json::json!({
            "request_id": "selection-2",
            "action": "gesture",
            "source": { "type": "fixture", "fixture_id": Uuid::from_u128(1) },
            "remove": false,
        });
        assert!(matches!(
            serde_json::from_value::<ProgrammingSelectionActionRequest>(gesture)
                .unwrap()
                .action,
            ProgrammingSelectionAction::Gesture { .. }
        ));
    }

    #[test]
    fn gesture_requests_reject_missing_or_misspelled_intent_fields() {
        for gesture in [
            serde_json::json!({
                "request_id": "selection-3",
                "action": "gesture",
                "source": { "type": "fixture", "fixture_id": Uuid::from_u128(1) },
            }),
            serde_json::json!({
                "request_id": "selection-4",
                "action": "gesture",
                "source": { "type": "fixture", "fixture_id": Uuid::from_u128(1) },
                "removee": true,
            }),
        ] {
            assert!(serde_json::from_value::<ProgrammingSelectionActionRequest>(gesture).is_err());
        }
    }
}
