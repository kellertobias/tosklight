//! The 3D model library: numbered slots a layer's `3D model` channel maps it onto.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One assigned model slot. Slot zero is deliberately absent: it means "draw flat".
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct ModelSlotView {
    pub slot: u8,
    pub name: String,
    pub vertices: u32,
    pub triangles: u32,
    /// `ready`, or `unloadable` when the stored file could not be loaded; a layer selecting an
    /// unloadable model draws flat.
    pub status: String,
    /// Why the model is unloadable.
    pub detail: Option<String>,
}

impl ModelSlotView {
    pub(crate) fn of(entry: &media_domain::ModelEntry, failure: Option<&str>) -> Self {
        Self {
            slot: entry.slot,
            name: entry.name.clone(),
            vertices: entry.vertices,
            triangles: entry.triangles,
            status: if failure.is_some() {
                "unloadable"
            } else {
                "ready"
            }
            .to_owned(),
            detail: failure.map(str::to_owned),
        }
    }
}

/// Renames or clears a slot. Assigning a model is an upload.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct UpdateModelSlot {
    pub request_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub clear: Option<bool>,
}

/// The answer to clearing a slot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct ClearedModelSlotView {
    pub slot: u8,
    pub assigned: bool,
}
