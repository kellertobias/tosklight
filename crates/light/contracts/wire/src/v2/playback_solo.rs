//! Typed snapshot and replay-safe update DTOs for show-owned Playback Solo regions.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackSoloRegion {
    pub id: Uuid,
    pub name: String,
    /// Explicit stable addresses, independent of panes and desks.
    pub members: Vec<super::playback::PlaybackAddressValue>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackSoloSnapshot {
    pub show_id: Uuid,
    #[schemars(range(max = 9007199254740991_u64))]
    #[ts(type = "number")]
    pub revision: u64,
    pub regions: Vec<PlaybackSoloRegion>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackSoloUpdateRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    #[schemars(range(max = 9007199254740991_u64))]
    #[ts(type = "number")]
    pub expected_revision: u64,
    pub regions: Vec<PlaybackSoloRegion>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackSoloUpdateOutcome {
    pub request_id: String,
    pub show_id: Uuid,
    #[schemars(range(max = 9007199254740991_u64))]
    #[ts(type = "number")]
    pub revision: u64,
    pub regions: Vec<PlaybackSoloRegion>,
    pub replayed: bool,
    pub changed: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackSoloRegionsChange {
    pub show_id: Uuid,
    #[schemars(range(max = 9007199254740991_u64))]
    #[ts(type = "number")]
    pub revision: u64,
}
