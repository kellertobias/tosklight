//! Typed control-desk settings and explicit page-assignment intents.

use super::runtime::{RuntimeControlDesk, RuntimePlaybackSurfaceLayout};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ControlDeskConfigurationActionRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    pub action: ControlDeskConfigurationAction,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ControlDeskConfigurationAction {
    Update {
        patch: ControlDeskConfigurationPatch,
    },
    SetPage {
        #[schemars(range(min = 1, max = 127))]
        page: u8,
        #[serde(default)]
        existing_only: bool,
    },
    RemoveClient {
        client_id: Uuid,
    },
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ControlDeskConfigurationPatch {
    pub name: Option<String>,
    pub columns: Option<u8>,
    pub rows: Option<u8>,
    pub buttons: Option<u8>,
    pub playback_layout: Option<RuntimePlaybackSurfaceLayout>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ControlDeskConfigurationActionOutcome {
    pub request_id: String,
    pub replayed: bool,
    pub desk: RuntimeControlDesk,
    pub removed: bool,
    pub page: Option<u8>,
    #[schemars(range(max = 9007199254740991_u64))]
    #[ts(type = "number | null")]
    pub event_sequence: Option<u64>,
    #[schemars(range(max = 9007199254740991_u64))]
    #[ts(type = "number | null")]
    pub page_creation_event_sequence: Option<u64>,
}
