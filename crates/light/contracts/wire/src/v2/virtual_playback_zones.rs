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
    /// One-based logical cell positions on the owning Virtual Playback surface.
    pub slots: Vec<u16>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VirtualPlaybackSurfacePageMode {
    FollowMain,
    Pinned {
        #[schemars(range(min = 1, max = 127))]
        page: u8,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct VirtualPlaybackExclusionSurface {
    #[schemars(range(max = 9007199254740991_u64))]
    #[ts(type = "number")]
    pub revision: u64,
    pub page_mode: VirtualPlaybackSurfacePageMode,
    pub zones: Vec<VirtualPlaybackExclusionZone>,
}

pub type VirtualPlaybackExclusionSurfaces =
    HashMap<String, VirtualPlaybackExclusionSurface>;
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
    #[schemars(range(max = 9007199254740991_u64))]
    #[ts(type = "number")]
    pub expected_revision: u64,
    pub page_mode: VirtualPlaybackSurfacePageMode,
    pub zones: Vec<VirtualPlaybackExclusionZone>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct VirtualPlaybackExclusionUpdateOutcome {
    pub request_id: String,
    pub show_id: Uuid,
    pub desk_id: Uuid,
    pub surface_id: String,
    pub surface: VirtualPlaybackExclusionSurface,
    pub replayed: bool,
    pub changed: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct VirtualPlaybackExclusionZonesChange {
    pub show_id: Uuid,
    pub desk_id: Uuid,
    pub surface_id: String,
}
