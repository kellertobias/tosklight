use super::CommandLineResponse;
use crate::v2::events::EventSnapshotCursor;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

/// Authoritative interaction context shared by every control surface attached to one desk.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingInteractionProjection {
    pub desk_id: Uuid,
    pub command_line: CommandLineResponse,
    pub selection: ProgrammerSelectionProjection,
}

/// Sparse authoritative components changed by one semantic Programmer interaction.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
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
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammerSelectionProjection {
    pub selected: Vec<Uuid>,
    pub expression: Option<ProgrammerSelectionExpression>,
    #[ts(type = "number")]
    pub revision: u64,
    pub gesture_open: bool,
    /// Portable method plus independent traversal cursors. Older servers and repaired snapshots
    /// default to the documented 2D Stage grid.
    #[serde(default)]
    pub grid: ProgrammerSelectionGrid,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammerSelectionGrid {
    pub configuration: ProgrammerSelectionGridConfiguration,
    pub rows_first: ProgrammerRowsFirstTraversal,
    pub columns_first: ProgrammerColumnsFirstTraversal,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammerSelectionGridConfiguration {
    pub method: ProgrammerSelectionGridMethod,
    #[serde(default)]
    pub axis_origin: ProgrammerSelectionGridAxisOrigin,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammerSelectionGridAxisOrigin {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProgrammerSelectionGridMethod {
    #[default]
    Stage2d,
    TopToBottom,
    BottomToTop,
    FrontToBack,
    BackToFront,
    LeftToRight,
    RightToLeft,
    HorizontalAxisX,
    VerticalAxisZ,
    RoomDepthAxisY,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProgrammerRowsFirstTraversal {
    #[default]
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProgrammerColumnsFirstTraversal {
    #[default]
    TopLeft,
    BottomLeft,
    TopRight,
    BottomRight,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProgrammerSelectionGridTraversalAxis {
    Rows,
    Columns,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProgrammerSelectionExpression {
    Static,
    LiveGroup {
        group_id: String,
        rule: ProgrammerSelectionRule,
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
        #[schemars(range(min = 1, max = 9007199254740991_u64))]
        #[ts(type = "number")]
        n: u64,
        #[schemars(range(max = 9007199254740991_u64))]
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
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
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
            gesture_open: true,
            grid: ProgrammerSelectionGrid::default(),
        }
    }
}
