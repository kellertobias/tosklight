//! Stable command, outcome, repair-snapshot, and event DTOs for Playback v2.

mod projection;

pub use projection::*;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use super::events::EventSnapshotCursor;

/// One authenticated, idempotent Playback mutation.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PlaybackActionRequest {
    /// Idempotency is retained for the 4096 most-recent IDs in the live server process. After
    /// eviction or restart, clients repair from a narrow runtime snapshot before retrying.
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    pub address: PlaybackAddress,
    pub action: PlaybackAction,
    /// Selects virtual or physical preload-capture semantics. The HTTP source itself is always
    /// recorded by the server and cannot be supplied by a caller.
    pub surface: PlaybackSurface,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlaybackAddress {
    CueList { cue_list_id: Uuid },
    Playback { playback_number: u16 },
    CurrentPage { slot: u8 },
    ExplicitPage { page: u8, slot: u8 },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResolvedPlaybackAddress {
    CueList {
        cue_list_id: Uuid,
    },
    Playback {
        playback_number: u16,
        page: Option<u8>,
        slot: Option<u8>,
    },
}

/// Deliberate interaction semantics accepted at the HTTP boundary.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackSurface {
    Virtual,
    Physical,
}

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlaybackAction {
    Go { pressed: bool },
    Back { pressed: bool },
    Pause { pressed: bool },
    Release,
    On { pressed: bool },
    Off { pressed: bool },
    Toggle { pressed: bool },
    FastForward { pressed: bool },
    FastRewind { pressed: bool },
    Flash { pressed: bool },
    Temp { pressed: bool },
    Swap { pressed: bool },
    Select { pressed: bool },
    SelectContents { pressed: bool },
    SelectDereferenced { pressed: bool },
    Learn { pressed: bool },
    Double { pressed: bool },
    Half { pressed: bool },
    Blackout { pressed: bool },
    PauseDynamics { pressed: bool },
    None { pressed: bool },
    Master { value: f32 },
    GoTo { cue_number: f64 },
    Load { cue_number: f64 },
    Crossfade { enabled: bool },
    Temporary { enabled: bool, pressed: bool },
    ConfiguredButton { number: u8, pressed: bool },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct PlaybackActionOutcome {
    pub request_id: String,
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
    pub replayed: bool,
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
#[serde(deny_unknown_fields)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_surface_cannot_claim_an_external_action_source() {
        for forged in ["osc", "matter"] {
            let value = serde_json::json!({
                "request_id": "request-1",
                "address": {"kind": "playback", "playback_number": 1},
                "action": {"type": "go", "pressed": true},
                "surface": forged,
            });
            assert!(serde_json::from_value::<PlaybackActionRequest>(value).is_err());
        }
    }

    #[test]
    fn action_is_a_readable_discriminated_contract() {
        let request: PlaybackActionRequest = serde_json::from_value(serde_json::json!({
            "request_id": "request-2",
            "address": {"kind": "explicit_page", "page": 2, "slot": 4},
            "action": {"type": "master", "value": 0.75},
            "surface": "physical",
        }))
        .expect("decode playback action");
        assert_eq!(
            request.address,
            PlaybackAddress::ExplicitPage { page: 2, slot: 4 }
        );
        assert_eq!(request.surface, PlaybackSurface::Physical);
    }

    #[test]
    fn snapshot_allows_a_desk_only_or_bounded_identity_list_in_schema() {
        let schema = schemars::schema_for!(PlaybackRuntimeSnapshotRequest);
        let json = serde_json::to_value(schema).expect("serialize schema");
        assert!(json["properties"]["identities"].get("minItems").is_none());
        assert_eq!(json["properties"]["identities"]["maxItems"], 256);
    }
}
