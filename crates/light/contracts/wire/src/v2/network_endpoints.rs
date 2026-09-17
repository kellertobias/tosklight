//! The DMX screen's Nodes tab: every Art-Net and sACN endpoint the desk sends to or hears from.

use crate::v2::events::{OutputDeliveryMode, OutputProtocol};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum NetworkEndpointDirection {
    Send,
    Receive,
}

/// Whether the endpoint is the show's or desk's own configuration, or something heard on the
/// network.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum NetworkEndpointOrigin {
    Configured,
    Observed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum NetworkEndpointStatus {
    /// Data is flowing now.
    Active,
    /// Open and waiting; nothing has arrived recently.
    Listening,
    /// Configured but nothing flowed recently.
    Idle,
    /// Switched off by the operator.
    Disabled,
    /// Another source sends a universe the desk also sends.
    Conflict,
    /// Sending or opening fails.
    Error,
    /// The transport behind it could not start.
    Unavailable,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct NetworkEndpoint {
    /// Stable within one snapshot and between snapshots while the endpoint exists.
    pub id: String,
    pub protocol: OutputProtocol,
    pub direction: NetworkEndpointDirection,
    pub origin: NetworkEndpointOrigin,
    /// What the endpoint is for, e.g. "DMX output" or "Universe discovery".
    pub role: String,
    /// The address and port packets go to or arrive at.
    pub endpoint: String,
    /// The peer's name, when it announces one.
    pub name: Option<String>,
    pub delivery_mode: Option<OutputDeliveryMode>,
    /// The show's logical universe a send route carries.
    pub logical_universe: Option<u16>,
    /// Protocol universes on the wire, ascending.
    pub universes: Vec<u16>,
    pub status: NetworkEndpointStatus,
    /// What the status means and what to do about it.
    pub detail: String,
    #[ts(type = "number")]
    pub errors: u64,
    #[ts(type = "number | null")]
    pub last_activity_millis_ago: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct NetworkEndpointsSnapshot {
    pub output_bind_ip: String,
    /// False when the network output could not start; every send endpoint is then unavailable.
    pub network_output_available: bool,
    pub endpoints: Vec<NetworkEndpoint>,
}
