//! Typed snapshot and replay-safe update DTOs for show-owned Virtual Playback exclusion zones.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct VirtualPlaybackExclusionZone {
    pub id: String,
    pub name: String,
    /// Stable show-owned Virtual Playback numbers, independent of panes and desks.
    pub playback_numbers: Vec<u16>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct VirtualPlaybackExclusionSnapshot {
    pub show_id: Uuid,
    #[schemars(range(max = 9007199254740991_u64))]
    #[ts(type = "number")]
    pub revision: u64,
    pub zones: Vec<VirtualPlaybackExclusionZone>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct VirtualPlaybackExclusionUpdateRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    #[schemars(range(max = 9007199254740991_u64))]
    #[ts(type = "number")]
    pub expected_revision: u64,
    pub zones: Vec<VirtualPlaybackExclusionZone>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct VirtualPlaybackExclusionUpdateOutcome {
    pub request_id: String,
    pub show_id: Uuid,
    #[schemars(range(max = 9007199254740991_u64))]
    #[ts(type = "number")]
    pub revision: u64,
    pub zones: Vec<VirtualPlaybackExclusionZone>,
    pub replayed: bool,
    pub changed: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct VirtualPlaybackExclusionZonesChange {
    pub show_id: Uuid,
    #[schemars(range(max = 9007199254740991_u64))]
    #[ts(type = "number")]
    pub revision: u64,
}
