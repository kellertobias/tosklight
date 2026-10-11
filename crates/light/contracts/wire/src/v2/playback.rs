//! Stable command, outcome, repair-snapshot, and event DTOs for Playback v2.

mod projection;

pub use projection::*;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use super::events::EventSnapshotCursor;

/// One live control action. Repeated actions execute independently and are never replayed.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackActionRequest {
    pub address: PlaybackAddress,
    pub action: PlaybackAction,
    pub control_source: PlaybackControlSource,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackAddressValue {
    #[schemars(range(min = 1, max = 99))]
    pub page: u8,
    #[schemars(range(min = 1, max = 1000))]
    pub position: u16,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackControlSource {
    #[schemars(length(min = 1, max = 256))]
    pub surface_id: String,
    pub control_id: u16,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlaybackAddress {
    CueList {
        cue_list_id: Uuid,
    },
    Group {
        #[schemars(length(min = 1, max = 256))]
        group_id: String,
    },
    Playback {
        #[schemars(range(min = 1, max = 99))]
        page: u8,
        #[schemars(range(min = 1, max = 1000))]
        position: u16,
    },
    CurrentPage {
        #[schemars(range(min = 1, max = 1000))]
        position: u16,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResolvedPlaybackAddress {
    CueList {
        cue_list_id: Uuid,
    },
    Group {
        #[schemars(length(min = 1, max = 256))]
        group_id: String,
    },
    Playback {
        #[schemars(range(min = 1, max = 99))]
        page: u8,
        #[schemars(range(min = 1, max = 1000))]
        position: u16,
    },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlaybackAction {
    Go {
        pressed: bool,
    },
    Back {
        pressed: bool,
    },
    Pause {
        pressed: bool,
    },
    Release,
    RuntimeStopDynamic {
        dynamic_id: Uuid,
        instance_id: Uuid,
        controller_id: Uuid,
    },
    On {
        pressed: bool,
    },
    Off {
        pressed: bool,
    },
    Toggle {
        pressed: bool,
    },
    FastForward {
        pressed: bool,
    },
    FastRewind {
        pressed: bool,
    },
    Flash {
        pressed: bool,
    },
    Temp {
        pressed: bool,
    },
    Swap {
        pressed: bool,
    },
    Select {
        pressed: bool,
    },
    SelectContents {
        pressed: bool,
    },
    SelectDereferenced {
        pressed: bool,
    },
    Learn {
        pressed: bool,
    },
    Double {
        pressed: bool,
    },
    Half {
        pressed: bool,
    },
    Blackout {
        pressed: bool,
    },
    PauseDynamics {
        pressed: bool,
    },
    DynamicRestart {
        pressed: bool,
    },
    DynamicDoubleSpeed {
        pressed: bool,
    },
    DynamicHalfSpeed {
        pressed: bool,
    },
    DynamicLearnSpeed {
        pressed: bool,
    },
    None {
        pressed: bool,
    },
    Master {
        value: f32,
    },
    MasterAbsolute {
        value: f32,
    },
    GoTo {
        cue_number: String,
    },
    Load {
        cue_number: String,
    },
    Crossfade {
        enabled: bool,
    },
    Temporary {
        enabled: bool,
        pressed: bool,
    },
    ConfiguredButton {
        number: u8,
        pressed: bool,
    },
    ConfiguredFader {
        number: u8,
        level: f32,
    },
    ConfiguredFaderAbsolute {
        number: u8,
        level: f32,
    },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackActionOutcome {
    pub correlation_id: Uuid,
    pub requested: PlaybackAddress,
    pub resolved: ResolvedPlaybackAddress,
    pub outcome: PlaybackOutcome,
    pub durability: PlaybackDurability,
    pub projection: PlaybackRuntimeProjection,
    /// Additional runtime identities changed atomically by the same action, in event order.
    pub related: Vec<PlaybackRelatedOutcome>,
    pub desk: Option<PlaybackDeskProjection>,
    /// Highest sequence emitted by the action; absent for no-change or captured actions.
    #[ts(as = "Option<f64>", optional = nullable)]
    pub event_sequence: Option<u64>,
    /// Sequence of a separate desk-local view event when selection changed.
    #[ts(as = "Option<f64>", optional = nullable)]
    pub desk_event_sequence: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackRelatedOutcome {
    pub projection: PlaybackRuntimeProjection,
    #[ts(as = "f64")]
    pub event_sequence: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PlaybackOutcome {
    Applied,
    NoChange,
    Captured { pending: PendingPlaybackAction },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackDurability {
    Durable,
    PersistencePending,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PendingPlaybackAction {
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
pub struct PlaybackErrorResponse {
    pub kind: PlaybackErrorKind,
    pub error: String,
    pub retryable: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackErrorKind {
    Invalid,
    Unauthorized,
    Forbidden,
    NotFound,
    Conflict,
    Unavailable,
    Internal,
}

/// Requested identities for an authoritative, deliberately narrow repair snapshot.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackRuntimeSnapshotRequest {
    #[schemars(length(max = 256))]
    pub identities: Vec<PlaybackRuntimeIdentity>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackRuntimeSnapshot {
    pub cursor: EventSnapshotCursor,
    pub desk: PlaybackDeskProjection,
    pub projections: Vec<PlaybackRuntimeProjection>,
}

/// Authenticated desk snapshot for topology and runtime inspection.
///
/// Portable Cuelist, Playback, and Page bodies remain opaque here because their exact
/// extension-preserving schema is owned by the Show-object boundary. The top-level scope and
/// desk-owned fields are stable and typed.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackOverview {
    #[ts(type = "unknown[]")]
    pub cue_lists: Vec<serde_json::Value>,
    #[ts(type = "unknown[]")]
    pub pages: Vec<serde_json::Value>,
    #[ts(type = "unknown[]")]
    pub active: Vec<serde_json::Value>,
    pub desk: super::runtime::RuntimeControlDesk,
    pub surfaces: super::playback_surfaces::PlaybackSurfacesSnapshot,
    pub selected_playback: Option<PlaybackAddressValue>,
    #[ts(type = "unknown")]
    pub authoritative_controls: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unified_action_retains_address_and_control_source() {
        let request: PlaybackActionRequest = serde_json::from_value(serde_json::json!({
            "address":{"kind":"playback","page":99,"position":1000},
            "action":{"type":"flash","pressed":true},
            "control_source":{"surface_id":"pane:abc","control_id":2},
            "future_transport_hint":true
        }))
        .unwrap();
        assert_eq!(
            request.address,
            PlaybackAddress::Playback {
                page: 99,
                position: 1000
            }
        );
        assert_eq!(request.control_source.surface_id, "pane:abc");
        assert_eq!(request.action, PlaybackAction::Flash { pressed: true });
    }
    #[test]
    fn old_split_addresses_are_rejected() {
        for address in [
            serde_json::json!({"kind":"virtual","page":4,"playback_number":1001}),
            serde_json::json!({"kind":"playback","playback_number":1}),
            serde_json::json!({"kind":"explicit_page","page":2,"slot":4}),
        ] {
            assert!(serde_json::from_value::<PlaybackAddress>(address).is_err());
        }
    }
    #[test]
    fn address_schema_declares_operator_limits() {
        let json = serde_json::to_value(schemars::schema_for!(PlaybackAddressValue)).unwrap();
        assert_eq!(json["properties"]["page"]["maximum"], 99);
        assert_eq!(json["properties"]["position"]["maximum"], 1000);
    }
    #[test]
    fn live_request_has_no_replay_identity_or_surface_switch() {
        let json = serde_json::to_value(schemars::schema_for!(PlaybackActionRequest)).unwrap();
        assert!(json["properties"].get("request_id").is_none());
        assert!(json["properties"].get("surface").is_none());
    }
}

/// Show-owned capture eligibility. No page restriction means every page.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackPositionRange {
    #[schemars(range(min = 1, max = 1000))]
    pub start: u16,
    #[schemars(range(min = 1, max = 1000))]
    pub end: u16,
}
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackPreloadScope {
    pub position_ranges: Vec<PlaybackPositionRange>,
    pub pages: Option<Vec<u8>>,
}
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackPreloadScopeSnapshot {
    pub show_id: Uuid,
    #[ts(type = "number")]
    pub revision: u64,
    pub scope: PlaybackPreloadScope,
}
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackPreloadScopeUpdateRequest {
    pub request_id: String,
    #[ts(type = "number")]
    pub expected_revision: u64,
    pub scope: PlaybackPreloadScope,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackPreloadScopeUpdateOutcome {
    pub request_id: String,
    pub show_id: Uuid,
    #[ts(type = "number")]
    pub revision: u64,
    pub scope: PlaybackPreloadScope,
    pub replayed: bool,
    pub changed: bool,
}
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackPreloadScopeChange {
    pub show_id: Uuid,
    #[ts(type = "number")]
    pub revision: u64,
}
