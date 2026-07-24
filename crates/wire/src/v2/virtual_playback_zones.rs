//! Typed snapshot and replay-safe update DTOs for virtual-playback exclusion zones.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct VirtualPlaybackExclusionZone {
    pub id: String,
    pub name: String,
    pub slots: Vec<u8>,
}

pub type VirtualPlaybackExclusionSurfaces = HashMap<String, Vec<VirtualPlaybackExclusionZone>>;
pub type VirtualPlaybackExclusionDesks = HashMap<String, VirtualPlaybackExclusionSurfaces>;

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct VirtualPlaybackExclusionSnapshot {
    pub show_id: Uuid,
    pub desks: VirtualPlaybackExclusionDesks,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct VirtualPlaybackExclusionUpdateRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    pub zones: Vec<VirtualPlaybackExclusionZone>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct VirtualPlaybackExclusionUpdateOutcome {
    pub request_id: String,
    pub show_id: Uuid,
    pub desk_id: Uuid,
    pub surface_id: String,
    pub zones: Vec<VirtualPlaybackExclusionZone>,
    pub replayed: bool,
    pub changed: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct VirtualPlaybackExclusionZonesChange {
    pub show_id: Uuid,
    pub desk_id: Uuid,
    pub surface_id: String,
}
