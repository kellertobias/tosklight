//! Exact-user scoped queued Preload playback projection.

use super::events::EventSnapshotCursor;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

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
    DynamicPause,
    DynamicRestart,
    DynamicDoubleSpeed,
    DynamicHalfSpeed,
    DynamicLearnSpeed,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingPreloadPlaybackQueueItem {
    pub address: super::playback::PlaybackAddressValue,
    pub action: ProgrammingPreloadPlaybackAction,
    pub control_source: super::playback::PlaybackControlSource,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ProgrammingPreloadPlaybackQueueProjection {
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
    fn captured_queue_requires_exact_address_and_retains_order() {
        let value = serde_json::json!({"address":{"page":99,"position":1000},"action":"go","control_source":{"surface_id":"main","control_id":1}});
        let item: ProgrammingPreloadPlaybackQueueItem =
            serde_json::from_value(value.clone()).unwrap();
        let projection = ProgrammingPreloadPlaybackQueueProjection {
            revision: 2,
            actions: vec![item.clone(), item],
        };
        let result = serde_json::to_value(projection).unwrap();
        assert_eq!(result["actions"], serde_json::json!([value.clone(), value]));
        assert!(
            serde_json::from_value::<ProgrammingPreloadPlaybackQueueItem>(
                serde_json::json!({"playback_number":7,"action":"go","surface":"virtual"})
            )
            .is_err()
        );
    }
}
