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
