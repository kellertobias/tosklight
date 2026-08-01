//! What the desk has found of the other ToskLights on the network.
//!
//! Discovery is a read: the desk publishes the peers it can currently see, and the UI decides
//! whether that is worth offering the operator a button for. Nothing here is stored and nothing
//! here is show content — a peer that stops answering simply stops being listed.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Which ToskLight a peer is. A desk runs a show; an editor holds a planning document.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveredRole {
    Desk,
    Editor,
}

/// One ToskLight seen on the network.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
pub struct DiscoveredPeer {
    pub role: DiscoveredRole,
    /// What the peer calls itself, for a menu entry an operator recognises.
    pub name: String,
    /// The show or document it is holding. `null` means it has nothing to offer, which is worth
    /// listing and not worth offering to load from.
    pub show: Option<String>,
    /// `host:port` of the peer's API, resolved and ready to use.
    pub address: String,
    /// The service instance, which is what tells two peers with the same name apart and what a
    /// load action names.
    pub instance: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct DiscoverySnapshot {
    /// Whether this desk is looking at all. A desk on a network with no mDNS, or one that could
    /// not open the responder, answers an empty list — and says which of the two it is, so the UI
    /// does not present "nothing found" as if it had looked.
    pub browsing: bool,
    pub peers: Vec<DiscoveredPeer>,
}
