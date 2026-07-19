use super::CommandLineResponse;
use crate::v2::events::EventSnapshotCursor;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

/// Authoritative interaction context shared by every control surface attached to one desk.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingInteractionProjection {
    pub desk_id: Uuid,
    pub command_line: CommandLineResponse,
    pub selection: ProgrammerSelectionProjection,
}

/// Sparse authoritative components changed by one semantic Programmer interaction.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(untagged)]
pub enum ProgrammingInteractionChange {
    Both {
        desk_id: Uuid,
        command_line: CommandLineResponse,
        selection: ProgrammerSelectionProjection,
    },
    CommandLine {
        desk_id: Uuid,
        command_line: CommandLineResponse,
    },
    Selection {
        desk_id: Uuid,
        selection: ProgrammerSelectionProjection,
    },
}

/// Ordered desk-local selection and the operation revision which produced it.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammerSelectionProjection {
    pub selected: Vec<Uuid>,
    pub expression: Option<ProgrammerSelectionExpression>,
    #[ts(type = "number")]
    pub revision: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProgrammerSelectionExpression {
    Static,
    LiveGroup {
        group_id: String,
        rule: ProgrammerSelectionRule,
    },
    FrozenGroup {
        group_id: String,
        #[ts(type = "number")]
        source_revision: u64,
    },
    PlaybackContents {
        items: Vec<ProgrammerSelectionReference>,
    },
    Sources {
        items: Vec<ProgrammerSelectionReference>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProgrammerSelectionRule {
    All,
    Odd,
    Even,
    EveryNth {
        #[ts(type = "number")]
        n: u64,
        #[ts(type = "number")]
        offset: u64,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProgrammerSelectionReference {
    Fixture { fixture_id: Uuid },
    LiveGroup { group_id: String },
    RemoveFixture { fixture_id: Uuid },
    RemoveLiveGroup { group_id: String },
}

/// Narrow repair snapshot for one desk's interaction stream.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingInteractionSnapshot {
    pub cursor: EventSnapshotCursor,
    pub projection: ProgrammingInteractionProjection,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v2::command_line::CommandTarget;
    use serde_json::json;

    #[test]
    fn sparse_changes_serialize_only_the_changed_components() {
        let desk_id = Uuid::from_u128(1);
        let command = ProgrammingInteractionChange::CommandLine {
            desk_id,
            command_line: command_line(),
        };
        let command_json = serde_json::to_value(command).unwrap();
        assert!(command_json.get("command_line").is_some());
        assert!(command_json.get("selection").is_none());

        let selection = ProgrammingInteractionChange::Selection {
            desk_id,
            selection: selection(),
        };
        let selection_json = serde_json::to_value(selection).unwrap();
        assert!(selection_json.get("command_line").is_none());
        assert!(selection_json.get("selection").is_some());
    }

    #[test]
    fn sparse_change_rejects_empty_or_null_components() {
        let desk_id = Uuid::from_u128(1);
        assert!(
            serde_json::from_value::<ProgrammingInteractionChange>(json!({
                "desk_id": desk_id,
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<ProgrammingInteractionChange>(json!({
                "desk_id": desk_id,
                "command_line": null,
            }))
            .is_err()
        );
    }

    #[test]
    fn combined_change_round_trips_without_losing_a_component() {
        let change = ProgrammingInteractionChange::Both {
            desk_id: Uuid::from_u128(1),
            command_line: command_line(),
            selection: selection(),
        };
        let json = serde_json::to_value(&change).unwrap();
        assert!(json.get("command_line").is_some());
        assert!(json.get("selection").is_some());
        assert_eq!(
            serde_json::from_value::<ProgrammingInteractionChange>(json).unwrap(),
            change
        );
    }

    fn command_line() -> CommandLineResponse {
        CommandLineResponse {
            text: "F1".into(),
            target: CommandTarget::Fixture,
            pristine: false,
            revision: 2,
            pending_choice: None,
        }
    }

    fn selection() -> ProgrammerSelectionProjection {
        ProgrammerSelectionProjection {
            selected: vec![Uuid::from_u128(2)],
            expression: Some(ProgrammerSelectionExpression::Static),
            revision: 3,
        }
    }
}
