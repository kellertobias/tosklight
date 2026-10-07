//! Physical identity shared by authored fixture profiles and recorded programming.
//! These are data contracts; they do not depend on a fixture adapter or renderer.
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhysicalDataQuality {
    #[default]
    Unknown,
    Estimated,
    Manufacturer,
    Measured,
}

/// Full opening angle measured at 50% (Beam) or 10% (Field) of peak intensity.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpeningConvention {
    Beam,
    Field,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct NativeColorBinding {
    pub channel_id: Uuid,
    pub function_id: Uuid,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NativeColorValue {
    pub channel_id: Uuid,
    pub function_id: Uuid,
    /// Premaster native value. u32 preserves every supported channel width exactly.
    pub raw: u32,
}

/// Saved appearance provenance is distinct from native replay compatibility.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeColorIdentity {
    pub profile_id: Uuid,
    pub profile_revision: u32,
    pub profile_digest: String,
    pub mode_id: Uuid,
    pub head_id: Uuid,
    pub path_id: Uuid,
    pub model_revision: u32,
    pub native_layout_signature: String,
}
