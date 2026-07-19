//! Stable filtered event-subscription and playback-repair DTOs.

use super::{
    command_line::ProgrammingInteractionChange,
    playback::{PlaybackDeskProjection, PlaybackRuntimeChange},
    programming::ProgrammingValuesChange,
};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct EventSubscriptionFilter {
    #[serde(default)]
    pub capabilities: Vec<EventCapability>,
    #[serde(default)]
    pub classes: Vec<EventClass>,
    #[serde(default)]
    pub objects: Vec<EventObject>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct EventRateLimit {
    pub capability: EventCapability,
    pub class: EventClass,
    pub object: Option<EventObject>,
    #[ts(type = "number")]
    pub min_interval_millis: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventClientMessage {
    Subscribe {
        #[serde(default)]
        filter: EventSubscriptionFilter,
        #[serde(default)]
        #[ts(as = "Option<f64>", optional = nullable)]
        after_sequence: Option<u64>,
        #[serde(default)]
        #[ts(optional = nullable)]
        capacity: Option<u16>,
        #[serde(default)]
        rate_limits: Vec<EventRateLimit>,
    },
    Repair {
        cursor: EventSnapshotCursor,
    },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventServerMessage {
    Ready { cursor: EventSnapshotCursor },
    Event { event: Box<EventEnvelope> },
    Gap { gap: SequenceGap },
    Repaired { cursor: EventSnapshotCursor },
    Error { error: String },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct EventSnapshotCursor {
    #[ts(type = "number")]
    pub sequence: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct SequenceGap {
    #[ts(type = "number")]
    pub after_sequence: u64,
    #[ts(type = "number")]
    pub oldest_available: u64,
    #[ts(type = "number")]
    pub latest_sequence: u64,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct EventEnvelope {
    #[ts(type = "number")]
    pub sequence: u64,
    pub occurred_at: String,
    pub desk_id: Option<Uuid>,
    pub class: EventClass,
    pub object: Option<EventObject>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub related_objects: Option<Vec<EventObject>>,
    pub source: EventSource,
    pub correlation_id: Option<Uuid>,
    pub delivery: EventDeliveryPolicy,
    pub payload: EventPayload,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum EventCapability {
    Programmer,
    Playback,
    Show,
    Desk,
    Output,
    System,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum EventClass {
    Transition,
    Projection,
    CommandOutcome,
    Error,
    Safety,
    Telemetry,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize, TS)]
pub struct EventObject {
    pub capability: EventCapability,
    pub id: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum EventDeliveryPolicy {
    Lossless,
    Replaceable,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EventSource {
    Runtime,
    Action { source: EventActionSource },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum EventActionSource {
    UserInterface,
    Keyboard,
    Osc,
    Http,
    Midi,
    Matter,
    Cue,
    Timecode,
    Scheduler,
    Macro,
    System,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventPayload {
    ProgrammingInteractionChanged {
        change: ProgrammingInteractionChange,
    },
    ProgrammingValuesChanged {
        change: ProgrammingValuesChange,
    },
    PlaybackRuntimeChanged {
        change: PlaybackRuntimeChange,
    },
    PlaybackViewChanged {
        projection: PlaybackDeskProjection,
    },
    OutputRuntimeChanged {
        change: OutputRuntimeChange,
    },
    ShowPatchChanged {
        delta: super::patch::PatchDelta,
    },
    OutputRouteChanged {
        change: OutputRouteChange,
    },
    ShowObjectsChanged {
        change: ShowObjectsChange,
    },
    SelectiveImportApplied {
        change: Box<SelectiveImportChange>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum OutputRuntimeIdentity {
    GlobalMaster,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct OutputRuntimeScope {
    pub show_id: Uuid,
    #[ts(type = "number")]
    pub show_revision: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct OutputRuntimeProjection {
    pub scope: OutputRuntimeScope,
    pub identity: OutputRuntimeIdentity,
    pub grand_master: f32,
    pub blackout: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct OutputRuntimeChange {
    pub projection: OutputRuntimeProjection,
}

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct OutputRuntimeSnapshot {
    pub cursor: EventSnapshotCursor,
    pub projection: OutputRuntimeProjection,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ShowObjectsChange {
    pub show_id: Uuid,
    #[ts(type = "number")]
    pub show_revision: u64,
    pub changes: Vec<ShowObjectChange>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ShowObjectChange {
    pub kind: ShowObjectKind,
    pub object_id: String,
    #[ts(type = "number")]
    pub object_revision: u64,
    #[ts(type = "unknown | null")]
    pub body: Option<serde_json::Value>,
    pub deleted: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ShowObjectKind {
    CueList,
    Group,
    Playback,
    PlaybackPage,
    Preset,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct SelectiveImportChange {
    pub show_id: Uuid,
    #[ts(type = "number")]
    pub show_revision: u64,
    pub objects: Vec<SelectiveImportObjectChange>,
    pub profile_revisions: Vec<FixtureProfileIdentity>,
    pub managed_assets: Vec<ManagedAssetReference>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct SelectiveImportObjectChange {
    pub kind: String,
    pub object_id: String,
    #[ts(type = "number")]
    pub object_revision: u64,
    #[ts(type = "unknown")]
    pub body: serde_json::Value,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct FixtureProfileIdentity {
    pub profile_id: Uuid,
    #[ts(type = "number")]
    pub revision: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct ManagedAssetReference {
    pub asset_id: Uuid,
    #[ts(type = "number")]
    pub revision: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct OutputRouteChange {
    pub show_id: Uuid,
    #[ts(type = "number")]
    pub show_revision: u64,
    pub route_id: String,
    #[ts(type = "number")]
    pub object_revision: u64,
    pub route: Option<OutputRoute>,
    pub deleted: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct OutputRoute {
    pub protocol: OutputProtocol,
    pub logical_universe: u16,
    pub destination_universe: u16,
    pub delivery_mode: OutputDeliveryMode,
    pub destination: Option<String>,
    pub enabled: bool,
    pub minimum_slots: u16,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum OutputProtocol {
    ArtNet,
    Sacn,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum OutputDeliveryMode {
    Broadcast,
    Multicast,
    Unicast,
}
