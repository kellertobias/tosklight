//! The 3D model library: numbered slots a layer's `3D model` channel maps it onto.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A model every Media Server ships without an import.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "kebab-case")]
pub enum BuiltinModelId {
    Plane,
    Cube,
    Sphere,
    Cylinder,
    Pyramid,
}

impl From<media_domain::BuiltinModel> for BuiltinModelId {
    fn from(model: media_domain::BuiltinModel) -> Self {
        use media_domain::BuiltinModel as B;
        match model {
            B::Plane => Self::Plane,
            B::Cube => Self::Cube,
            B::Sphere => Self::Sphere,
            B::Cylinder => Self::Cylinder,
            B::Pyramid => Self::Pyramid,
        }
    }
}

impl From<BuiltinModelId> for media_domain::BuiltinModel {
    fn from(model: BuiltinModelId) -> Self {
        match model {
            BuiltinModelId::Plane => Self::Plane,
            BuiltinModelId::Cube => Self::Cube,
            BuiltinModelId::Sphere => Self::Sphere,
            BuiltinModelId::Cylinder => Self::Cylinder,
            BuiltinModelId::Pyramid => Self::Pyramid,
        }
    }
}

/// One assigned model slot. Slot zero is deliberately absent: it means "draw flat".
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct ModelSlotView {
    pub slot: u8,
    pub name: String,
    pub vertices: u32,
    pub triangles: u32,
    /// The built-in model this slot holds, or null for an imported `.glb`.
    pub builtin: Option<BuiltinModelId>,
    /// `ready`, or `unloadable` when the stored file could not be loaded; a layer selecting an
    /// unloadable model is mapped onto the Plane.
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
            builtin: entry.builtin.map(BuiltinModelId::from),
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

/// Renames a slot, clears it, or assigns a built-in model to it. Assigning an imported model is
/// an upload.
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
    /// Puts this built-in model in the slot, replacing whatever it held. The slot takes the
    /// model's name unless `name` is sent too.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub builtin: Option<BuiltinModelId>,
}

/// The answer to clearing a slot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct ClearedModelSlotView {
    pub slot: u8,
    pub assigned: bool,
}
