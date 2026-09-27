//! Discovered network show sources; imports name service instances rather than arbitrary hosts.
use super::{discovery::DiscoveredRole, show_library::ShowLibraryRevision};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct NetworkShowCatalog {
    pub browsing: bool,
    pub peers: Vec<NetworkShowPeer>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct NetworkShowPeer {
    pub instance: String,
    pub name: String,
    pub address: String,
    pub role: DiscoveredRole,
    pub shows: Vec<NetworkShow>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct NetworkShow {
    /// Editors advertise one open document, without a desk-library identity.
    pub id: Option<Uuid>,
    pub name: String,
    pub updated_at: Option<String>,
    pub revisions: Vec<ShowLibraryRevision>,
}

/// The remote desk's configured save destinations. Paths remain root-relative.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct NetworkSaveFolders {
    pub roots: Vec<NetworkSaveRoot>,
    pub root_id: Option<String>,
    pub path: String,
    pub entries: Vec<NetworkSaveEntry>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct NetworkSaveRoot {
    pub id: String,
    pub label: String,
    pub icon: String,
    pub removable: bool,
    pub writable: bool,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct NetworkSaveEntry {
    pub name: String,
    pub path: String,
    pub kind: NetworkSaveEntryKind,
    #[ts(type = "number")]
    pub size: u64,
    #[ts(type = "number | null")]
    pub modified_millis: Option<u64>,
    #[ts(type = "number | null")]
    pub created_millis: Option<u64>,
    pub hidden: bool,
    pub writable: bool,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum NetworkSaveEntryKind {
    Folder,
    File,
}
