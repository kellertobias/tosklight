use crate::FixtureDefinition;
use light_core::{DmxAddress, FixtureId, Universe};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::net::IpAddr;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PatchedFixture {
    pub fixture_id: FixtureId,
    /// Operator-facing fixture number. This is distinct from the stable internal UUID.
    #[serde(default)]
    pub fixture_number: Option<u32>,
    /// Operator-facing number in the reserved visual-only `0.x` namespace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub virtual_fixture_number: Option<u32>,
    /// Show-local operator name. Definition names remain immutable library metadata.
    #[serde(default)]
    pub name: String,
    pub definition: FixtureDefinition,
    #[serde(default)]
    pub universe: Option<Universe>,
    /// User-facing DMX address, always 1 through 512.
    #[serde(default)]
    pub address: Option<DmxAddress>,
    /// Schema-v2 independently patchable split assignments. Legacy universe/address remain the
    /// canonical split-1 representation and are migrated into this shape on the next save.
    #[serde(default)]
    pub split_patches: Vec<SplitPatch>,
    #[serde(default = "default_patch_layer")]
    pub layer_id: String,
    /// Optional direct-control endpoint attached to the physical parent fixture.
    /// Logical heads inherit this endpoint and cannot override it.
    #[serde(default)]
    pub direct_control: Option<DirectControlEndpoint>,
    #[serde(default)]
    pub location: FixtureLocation,
    #[serde(default)]
    pub rotation: FixtureVector,
    #[serde(default)]
    pub logical_heads: Vec<PatchedHead>,
    /// Additional physical instances controlled and selected as this fixture.
    /// An instance without a universe/address exists in the visualizer only.
    #[serde(default)]
    pub multipatch: Vec<MultiPatchInstance>,
    /// Preposition Position-family attributes for the next lit Cue while dark.
    #[serde(default = "default_true")]
    pub move_in_black_enabled: bool,
    /// Safety delay measured from the resolved-dark boundary.
    #[serde(default)]
    pub move_in_black_delay_millis: u64,
    /// Optional per-instance raw Highlight overrides keyed by stable channel ID.
    #[serde(default)]
    pub highlight_overrides: BTreeMap<Uuid, u32>,
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MultiPatchInstance {
    pub id: Uuid,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub universe: Option<Universe>,
    #[serde(default)]
    pub address: Option<DmxAddress>,
    #[serde(default)]
    pub split_patches: Vec<SplitPatch>,
    #[serde(default)]
    pub location: FixtureLocation,
    #[serde(default)]
    pub rotation: FixtureVector,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SplitPatch {
    pub split: u16,
    pub universe: Option<Universe>,
    pub address: Option<DmxAddress>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FixtureVector {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct FixtureLocation {
    /// Integer millimetres avoid accumulating floating-point positioning error.
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl<'de> Deserialize<'de> for FixtureLocation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct StoredLocation {
            #[serde(deserialize_with = "deserialize_location_coordinate")]
            x: i32,
            #[serde(deserialize_with = "deserialize_location_coordinate")]
            y: i32,
            #[serde(deserialize_with = "deserialize_location_coordinate")]
            z: i32,
        }
        let stored = StoredLocation::deserialize(deserializer)?;
        Ok(Self {
            x: stored.x,
            y: stored.y,
            z: stored.z,
        })
    }
}

fn deserialize_location_coordinate<'de, D>(deserializer: D) -> Result<i32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct CoordinateVisitor;
    impl<'de> serde::de::Visitor<'de> for CoordinateVisitor {
        type Value = i32;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str(
                "an integer millimetre coordinate or a legacy floating-point metre coordinate",
            )
        }
        fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<i32, E> {
            i32::try_from(value).map_err(E::custom)
        }
        fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<i32, E> {
            i32::try_from(value).map_err(E::custom)
        }
        fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<i32, E> {
            let millimetres = value * 1_000.0;
            if !millimetres.is_finite()
                || millimetres < f64::from(i32::MIN)
                || millimetres > f64::from(i32::MAX)
            {
                return Err(E::custom(
                    "legacy fixture location is outside the supported range",
                ));
            }
            Ok(millimetres.round() as i32)
        }
    }
    deserializer.deserialize_any(CoordinateVisitor)
}

pub(crate) fn default_patch_layer() -> String {
    "default".into()
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DirectControlEndpoint {
    pub protocol: DirectControlProtocol,
    pub ip_address: IpAddr,
    #[serde(default = "default_citp_port")]
    pub port: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DirectControlProtocol {
    Citp,
}

const fn default_citp_port() -> u16 {
    4811
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PatchedHead {
    /// Stable identity of the selected immutable profile head.
    ///
    /// Legacy patch records do not have this value. The next profile-aware patch mutation fills it
    /// after matching that legacy head by `head_index` once.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_head_id: Option<Uuid>,
    pub head_index: u16,
    pub fixture_id: FixtureId,
}
