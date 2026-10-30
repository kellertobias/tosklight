use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(
    Clone, Copy, Debug, Deserialize, Eq, JsonSchema, Ord, PartialEq, PartialOrd, Serialize, TS,
)]
#[serde(rename_all = "snake_case")]
pub enum ShowHealthSeverity {
    Error,
    Warning,
    Info,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ShowHealthFinding {
    pub id: String,
    pub severity: ShowHealthSeverity,
    pub message: String,
    pub object_kind: String,
    pub object_id: String,
    #[ts(type = "number")]
    pub object_revision: u64,
    pub fixture_id: Option<Uuid>,
    pub multipatch_instance_id: Option<Uuid>,
    pub remove_missing_group_fixtures: bool,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ShowHealthSnapshot {
    pub show_id: Uuid,
    #[ts(type = "number")]
    pub show_revision: u64,
    pub checked_at: String,
    pub checks: Vec<String>,
    pub findings: Vec<ShowHealthFinding>,
}
