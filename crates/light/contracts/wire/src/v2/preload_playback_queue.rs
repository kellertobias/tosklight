//! Exact-user scoped queued Preload playback projection.

use super::events::EventSnapshotCursor;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProgrammingPreloadPlaybackAction {
    Toggle,
    Go,
    Back,
    Off,
    On,
    TemporaryOn,
    TemporaryOff,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProgrammingPreloadPlaybackSurface {
    Physical,
    Virtual,
    Osc,
    Matter,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingPreloadPlaybackQueueItem {
    pub playback_number: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub page: Option<u8>,
    pub action: ProgrammingPreloadPlaybackAction,
    pub surface: ProgrammingPreloadPlaybackSurface,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingPreloadPlaybackQueueProjection {
    pub user_id: Uuid,
    #[ts(type = "number")]
    pub revision: u64,
    pub actions: Vec<ProgrammingPreloadPlaybackQueueItem>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingPreloadPlaybackQueueChange {
    pub projection: ProgrammingPreloadPlaybackQueueProjection,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingPreloadPlaybackQueueSnapshot {
    pub cursor: EventSnapshotCursor,
    pub projection: ProgrammingPreloadPlaybackQueueProjection,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_retains_order_duplicates_and_closed_names() {
        let action = ProgrammingPreloadPlaybackQueueItem {
            playback_number: 7,
            page: Some(3),
            action: ProgrammingPreloadPlaybackAction::TemporaryOn,
            surface: ProgrammingPreloadPlaybackSurface::Osc,
        };
        let projection = ProgrammingPreloadPlaybackQueueProjection {
            user_id: Uuid::from_u128(1),
            revision: 2,
            actions: vec![action, action],
        };

        let value = serde_json::to_value(projection).unwrap();
        assert_eq!(value["actions"][0]["action"], "temporary_on");
        assert_eq!(value["actions"][0]["surface"], "osc");
        assert_eq!(value["actions"][0]["page"], 3);
        assert_eq!(value["actions"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn queue_item_accepts_and_preserves_a_missing_page() {
        let legacy = serde_json::json!({
            "playback_number": 7,
            "action": "go",
            "surface": "virtual",
        });
        let item: ProgrammingPreloadPlaybackQueueItem =
            serde_json::from_value(legacy.clone()).unwrap();

        assert_eq!(item.page, None);
        assert_eq!(serde_json::to_value(item).unwrap(), legacy);
    }
}
