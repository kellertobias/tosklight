#![forbid(unsafe_code)]
//! Fixture definitions, portable fixture library, color calibration, patching, and DMX encoding.

mod profile;
pub use profile::*;

use light_core::{AttributeKey, AttributeValue, DmxAddress, FixtureId, Universe, Xyz};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    net::IpAddr,
    path::Path,
};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ByteOrder {
    MsbFirst,
    LsbFirst,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChannelComponent {
    pub offset: u16,
    pub byte_order: ByteOrder,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Capability {
    pub name: String,
    pub dmx_from: u8,
    pub dmx_to: u8,
    pub preset_family: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Parameter {
    pub attribute: AttributeKey,
    pub components: Vec<ChannelComponent>,
    /// Unspecified channel defaults are DMX zero for compatibility with minimal fixture profiles.
    #[serde(default)]
    pub default: f32,
    pub virtual_dimmer: bool,
    #[serde(default)]
    pub metadata: ParameterMetadata,
    #[serde(default)]
    pub capabilities: Vec<Capability>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ParameterMetadata {
    pub physical_min: f32,
    pub physical_max: f32,
    pub unit: Option<String>,
    pub invert: bool,
    pub wrap: bool,
    pub curve: DmxCurve,
}
impl Default for ParameterMetadata {
    fn default() -> Self {
        Self {
            physical_min: 0.0,
            physical_max: 1.0,
            unit: None,
            invert: false,
            wrap: false,
            curve: DmxCurve::Linear,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DmxCurve {
    #[default]
    Linear,
    Square,
    SquareRoot,
    SmoothStep,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LogicalHead {
    pub index: u16,
    pub name: String,
    #[serde(default)]
    pub shared: bool,
    pub parameters: Vec<Parameter>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmitterCalibration {
    pub name: String,
    pub xyz: Xyz,
    pub limit: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ColorCalibration {
    pub emitters: Vec<EmitterCalibration>,
    #[serde(default = "identity_matrix")]
    pub correction_matrix: [[f32; 3]; 3],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FixtureDefinition {
    pub schema_version: u16,
    pub id: FixtureId,
    pub revision: u32,
    pub manufacturer: String,
    /// Broad, operator-facing classification used to browse the desk library.
    #[serde(default)]
    pub device_type: String,
    /// Human-readable fixture name. `model` remains the manufacturer's model identifier.
    #[serde(default)]
    pub name: String,
    pub model: String,
    pub mode: String,
    pub footprint: u16,
    pub heads: Vec<LogicalHead>,
    pub color_calibration: Option<ColorCalibration>,
    #[serde(default)]
    pub physical: FixturePhysicalProperties,
    /// Optional stage-view assets. Values are portable asset identifiers or data URLs.
    #[serde(default)]
    pub model_asset: Option<String>,
    #[serde(default)]
    pub icon_asset: Option<String>,
    pub hazardous: bool,
    /// Direct-control transports explicitly supported by this fixture profile.
    #[serde(default)]
    pub direct_control_protocols: Vec<DirectControlProtocol>,
    #[serde(default)]
    pub signal_loss_policy: SignalLossPolicy,
    pub safe_values: BTreeMap<AttributeKey, AttributeValue>,
    /// Present for schema-v2 snapshots embedded in shows. The complete profile snapshot insulates
    /// an already-patched show from later desk-library revisions.
    #[serde(default)]
    pub profile_id: Option<FixtureId>,
    #[serde(default)]
    pub mode_id: Option<Uuid>,
    #[serde(default)]
    pub profile_snapshot: Option<Box<FixtureProfile>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FixturePhysicalProperties {
    pub pan_range_degrees: Option<f32>,
    pub tilt_range_degrees: Option<f32>,
    pub width_millimetres: Option<f32>,
    pub height_millimetres: Option<f32>,
    pub depth_millimetres: Option<f32>,
    pub weight_kilograms: Option<f32>,
    #[serde(default)]
    pub power_watts: Option<f32>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SignalLossPolicy {
    #[default]
    HoldLast,
    FadeToSafe {
        duration_millis: u64,
    },
    ImmediateSafe,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PatchedFixture {
    pub fixture_id: FixtureId,
    /// Operator-facing fixture number. This is distinct from the stable internal UUID.
    #[serde(default)]
    pub fixture_number: Option<u32>,
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

#[derive(Clone, Debug, Serialize, Deserialize)]
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

fn default_patch_layer() -> String {
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PatchedHead {
    pub head_index: u16,
    pub fixture_id: FixtureId,
}

/// Rebuild the persisted logical-head mapping from the active fixture definition.
/// Existing IDs are retained by definition head index so programming remains stable.
pub fn reconcile_logical_heads(fixture: &mut PatchedFixture) -> bool {
    let before = fixture
        .logical_heads
        .iter()
        .map(|head| (head.head_index, head.fixture_id))
        .collect::<Vec<_>>();
    let mut existing = fixture
        .logical_heads
        .drain(..)
        .map(|head| (head.head_index, head.fixture_id))
        .collect::<HashMap<_, _>>();
    fixture.logical_heads = fixture
        .definition
        .heads
        .iter()
        .filter(|head| !head.shared)
        .map(|head| PatchedHead {
            head_index: head.index,
            fixture_id: existing.remove(&head.index).unwrap_or_else(FixtureId::new),
        })
        .collect();
    before
        != fixture
            .logical_heads
            .iter()
            .map(|head| (head.head_index, head.fixture_id))
            .collect::<Vec<_>>()
}

#[derive(Debug, Error)]
pub enum FixtureError {
    #[error("invalid fixture: {0}")]
    Invalid(String),
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("fixture revision conflict: expected {expected}, current {current}")]
    RevisionConflict { expected: u32, current: u32 },
}

impl FixtureDefinition {
    pub fn split_footprints(&self) -> BTreeMap<u16, u16> {
        if self.schema_version == FIXTURE_PROFILE_SCHEMA_VERSION
            && let (Some(profile), Some(mode_id)) = (&self.profile_snapshot, self.mode_id)
            && let Some(mode) = profile.mode(mode_id)
        {
            return mode
                .splits
                .iter()
                .map(|split| (split.number, split.footprint))
                .collect();
        }
        BTreeMap::from([(1, self.footprint)])
    }

    pub fn effective_signal_loss_policy(&self) -> SignalLossPolicy {
        if self.hazardous && self.signal_loss_policy == SignalLossPolicy::HoldLast {
            SignalLossPolicy::ImmediateSafe
        } else {
            self.signal_loss_policy
        }
    }
    pub fn validate(&self) -> Result<(), FixtureError> {
        if self.schema_version == FIXTURE_PROFILE_SCHEMA_VERSION {
            let profile = self.profile_snapshot.as_deref().ok_or_else(|| {
                FixtureError::Invalid("schema-v2 fixture snapshot is missing its profile".into())
            })?;
            profile
                .validate()
                .map_err(|error| FixtureError::Invalid(error.to_string()))?;
            if self.profile_id != Some(profile.id)
                || self.id != profile.id
                || self.revision != profile.revision
                || self
                    .mode_id
                    .is_none_or(|mode_id| profile.mode(mode_id).is_none())
            {
                return Err(FixtureError::Invalid(
                    "schema-v2 fixture snapshot identity is inconsistent".into(),
                ));
            }
            return Ok(());
        }
        if self.schema_version != 1 {
            return Err(FixtureError::Invalid(
                "unsupported fixture schema version".into(),
            ));
        }
        if self.footprint == 0 || self.footprint > 512 {
            return Err(FixtureError::Invalid(
                "footprint must be within one DMX universe".into(),
            ));
        }
        if self.heads.is_empty() {
            return Err(FixtureError::Invalid(
                "fixture needs at least one logical head".into(),
            ));
        }
        if self.manufacturer.trim().is_empty() || self.display_name().is_empty() {
            return Err(FixtureError::Invalid(
                "manufacturer and fixture name are required".into(),
            ));
        }
        for parameter in self.heads.iter().flat_map(|head| &head.parameters) {
            let abstract_virtual_dimmer = parameter.virtual_dimmer
                && parameter.attribute.is_intensity()
                && parameter.components.is_empty();
            if (!abstract_virtual_dimmer && parameter.components.is_empty())
                || parameter.components.len() > 4
            {
                return Err(FixtureError::Invalid(format!(
                    "{} must have 1-4 DMX components",
                    parameter.attribute.0
                )));
            }
            if !(0.0..=1.0).contains(&parameter.default) {
                return Err(FixtureError::Invalid(format!(
                    "{} default is outside 0-1",
                    parameter.attribute.0
                )));
            }
            if !parameter.metadata.physical_min.is_finite()
                || !parameter.metadata.physical_max.is_finite()
                || parameter.metadata.physical_min >= parameter.metadata.physical_max
            {
                return Err(FixtureError::Invalid(format!(
                    "{} physical range is invalid",
                    parameter.attribute.0
                )));
            }
            for component in &parameter.components {
                if component.offset >= self.footprint {
                    return Err(FixtureError::Invalid(
                        "component is outside fixture footprint".into(),
                    ));
                }
            }
        }
        if let Some(calibration) = &self.color_calibration {
            if calibration.emitters.len() < 3 {
                return Err(FixtureError::Invalid(
                    "color calibration needs at least three emitters".into(),
                ));
            }
            if calibration.emitters.iter().any(|emitter| {
                !(0.0..=1.0).contains(&emitter.limit)
                    || emitter.xyz.x < 0.0
                    || emitter.xyz.y < 0.0
                    || emitter.xyz.z < 0.0
            }) {
                return Err(FixtureError::Invalid("invalid emitter calibration".into()));
            }
        }
        Ok(())
    }

    pub fn display_name(&self) -> &str {
        if self.name.trim().is_empty() {
            &self.model
        } else {
            &self.name
        }
    }

    pub fn generated_presets(&self) -> Vec<GeneratedPreset> {
        self.heads
            .iter()
            .flat_map(|head| {
                head.parameters.iter().flat_map(move |parameter| {
                    parameter.capabilities.iter().filter_map(move |capability| {
                        capability
                            .preset_family
                            .as_ref()
                            .map(|family| GeneratedPreset {
                                family: family.clone(),
                                name: capability.name.clone(),
                                head_index: head.index,
                                attribute: parameter.attribute.clone(),
                                value: AttributeValue::Normalized(
                                    (f32::from(capability.dmx_from) + f32::from(capability.dmx_to))
                                        / 510.0,
                                ),
                            })
                    })
                })
            })
            .collect()
    }
}

impl PatchedFixture {
    pub fn effective_split_patches(&self) -> Vec<SplitPatch> {
        if self.split_patches.is_empty() {
            vec![SplitPatch {
                split: 1,
                universe: self.universe,
                address: self.address,
            }]
        } else {
            self.split_patches.clone()
        }
    }
}

impl MultiPatchInstance {
    pub fn effective_split_patches(&self) -> Vec<SplitPatch> {
        if self.split_patches.is_empty() {
            vec![SplitPatch {
                split: 1,
                universe: self.universe,
                address: self.address,
            }]
        } else {
            self.split_patches.clone()
        }
    }
}

/// Normalize a persisted patched fixture into the schema-v2 portable snapshot and explicit split
/// assignment shape. This is intentionally an explicit reader/migration rather than relying on
/// serde defaults: once written, a show no longer needs either the desk fixture library or the
/// legacy universe/address fallback to understand its patch.
pub fn migrate_patched_fixture_to_v2(fixture: &mut PatchedFixture) -> Result<bool, FixtureError> {
    let original = serde_json::to_value(&*fixture)?;
    if fixture.definition.schema_version == 1 {
        let legacy = fixture.definition.clone();
        let mut profile = FixtureProfile::from_legacy_modes(std::slice::from_ref(&legacy))
            .map_err(|error| FixtureError::Invalid(error.to_string()))?;
        // An embedded snapshot retains the selected legacy revision as its portable identity. The
        // desk library may independently migrate the same source into its own revision sequence.
        profile.revision = legacy.revision.max(1);
        let mode_id = profile
            .modes
            .first()
            .map(|mode| mode.id)
            .ok_or_else(|| FixtureError::Invalid("migrated fixture has no mode".into()))?;
        let mut definition = profile
            .resolved_definition(mode_id)
            .map_err(|error| FixtureError::Invalid(error.to_string()))?;
        // These compatibility projections are still consumed by existing programmer and stage
        // surfaces. Keeping them verbatim avoids a behavior change while schema-v2 runtime paths
        // use the complete embedded profile snapshot.
        definition.heads = legacy.heads;
        definition.color_calibration = legacy.color_calibration;
        definition.physical.pan_range_degrees = legacy.physical.pan_range_degrees;
        definition.physical.tilt_range_degrees = legacy.physical.tilt_range_degrees;
        definition.safe_values = legacy.safe_values;
        fixture.definition = definition;
    }

    let splits = fixture.definition.split_footprints();
    if fixture.split_patches.is_empty() && splits.len() == 1 {
        fixture.split_patches = splits
            .keys()
            .enumerate()
            .map(|(index, split)| SplitPatch {
                split: *split,
                universe: (index == 0).then_some(fixture.universe).flatten(),
                address: (index == 0).then_some(fixture.address).flatten(),
            })
            .collect();
    }
    for instance in &mut fixture.multipatch {
        if instance.split_patches.is_empty() && splits.len() == 1 {
            instance.split_patches = splits
                .keys()
                .enumerate()
                .map(|(index, split)| SplitPatch {
                    split: *split,
                    universe: (index == 0).then_some(instance.universe).flatten(),
                    address: (index == 0).then_some(instance.address).flatten(),
                })
                .collect();
        }
    }
    reconcile_logical_heads(fixture);
    fixture.definition.validate()?;
    let normalized = serde_json::to_value(&*fixture)?;
    Ok(normalized != original)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GeneratedPreset {
    pub family: String,
    pub name: String,
    pub head_index: u16,
    pub attribute: AttributeKey,
    pub value: AttributeValue,
}

pub fn validate_patch(fixtures: &[PatchedFixture]) -> Result<(), FixtureError> {
    let mut used: HashMap<Universe, [bool; 512]> = HashMap::new();
    let mut fixture_numbers = HashMap::new();
    for fixture in fixtures {
        if let Some(number) = fixture.fixture_number {
            if number == 0 {
                return Err(FixtureError::Invalid("fixture IDs start at 1".into()));
            }
            if fixture_numbers.insert(number, fixture.fixture_id).is_some() {
                return Err(FixtureError::Invalid(format!(
                    "fixture ID {number} is already in use"
                )));
            }
        }
        fixture.definition.validate()?;
        if let Some(endpoint) = &fixture.direct_control {
            if endpoint.port == 0 {
                return Err(FixtureError::Invalid(format!(
                    "fixture {} has an invalid direct-control port",
                    fixture.fixture_id.0
                )));
            }
            if !fixture
                .definition
                .direct_control_protocols
                .contains(&endpoint.protocol)
            {
                return Err(FixtureError::Invalid(format!(
                    "fixture {} profile does not support {:?} direct control",
                    fixture.fixture_id.0, endpoint.protocol
                )));
            }
        }
        let footprints = fixture.definition.split_footprints();
        let mut instances = vec![(
            fixture.fixture_id.0.to_string(),
            fixture.split_patches.as_slice(),
            fixture.universe,
            fixture.address,
        )];
        instances.extend(fixture.multipatch.iter().map(|instance| {
            (
                instance.id.to_string(),
                instance.split_patches.as_slice(),
                instance.universe,
                instance.address,
            )
        }));
        for (instance, explicit_patches, legacy_universe, legacy_address) in instances {
            let patches = if explicit_patches.is_empty() {
                if footprints.len() > 1 {
                    return Err(FixtureError::Invalid(format!(
                        "fixture instance {instance} must assign every split, including unpatched splits"
                    )));
                }
                let split = *footprints.keys().next().ok_or_else(|| {
                    FixtureError::Invalid(format!(
                        "fixture instance {instance} has no defined splits"
                    ))
                })?;
                vec![SplitPatch {
                    split,
                    universe: legacy_universe,
                    address: legacy_address,
                }]
            } else {
                explicit_patches.to_vec()
            };
            let mut instance_splits = HashSet::new();
            for patch in &patches {
                if !instance_splits.insert(patch.split) {
                    return Err(FixtureError::Invalid(format!(
                        "fixture instance {instance} assigns split {} more than once",
                        patch.split
                    )));
                }
                if !footprints.contains_key(&patch.split) {
                    return Err(FixtureError::Invalid(format!(
                        "fixture instance {instance} references unknown split {}",
                        patch.split
                    )));
                }
            }
            if let Some(missing) = footprints
                .keys()
                .find(|split| !instance_splits.contains(split))
            {
                return Err(FixtureError::Invalid(format!(
                    "fixture instance {instance} is missing split {missing}; every split needs an optional assignment entry"
                )));
            }
            for patch in patches {
                let footprint = footprints[&patch.split];
                let universe = patch.universe;
                let address = patch.address;
                if universe.is_some() != address.is_some() {
                    return Err(FixtureError::Invalid(format!(
                        "fixture instance {instance} split {} must set both universe and address or neither",
                        patch.split
                    )));
                }
                let (Some(universe), Some(address)) = (universe, address) else {
                    continue;
                };
                if address == 0 || usize::from(address) + usize::from(footprint) - 1 > 512 {
                    return Err(FixtureError::Invalid(format!(
                        "fixture instance {instance} exceeds universe {universe}"
                    )));
                }
                let slots = used.entry(universe).or_insert([false; 512]);
                let start = usize::from(address - 1);
                for (offset, slot) in slots[start..start + usize::from(footprint)]
                    .iter_mut()
                    .enumerate()
                {
                    if *slot {
                        return Err(FixtureError::Invalid(format!(
                            "patch overlap at universe {} address {}",
                            universe,
                            start + offset + 1
                        )));
                    }
                    *slot = true;
                }
            }
        }
    }
    Ok(())
}

pub fn encode_parameter(
    frame: &mut [u8; 512],
    base: DmxAddress,
    parameter: &Parameter,
    value: f32,
) -> Result<(), FixtureError> {
    if base == 0 || base > 512 {
        return Err(FixtureError::Invalid(
            "DMX addresses are 1-based and must be within 1-512".into(),
        ));
    }
    let mut value = value.clamp(0.0, 1.0);
    if parameter.metadata.invert {
        value = 1.0 - value;
    }
    value = match parameter.metadata.curve {
        DmxCurve::Linear => value,
        DmxCurve::Square => value * value,
        DmxCurve::SquareRoot => value.sqrt(),
        DmxCurve::SmoothStep => value * value * (3.0 - 2.0 * value),
    };
    let bytes = parameter.components.len();
    if !(1..=4).contains(&bytes) {
        return Err(FixtureError::Invalid(
            "parameters require 1-4 channel components".into(),
        ));
    }
    let max = (1_u64 << (bytes * 8)) - 1;
    let encoded = (value * max as f32).round() as u64;
    for (index, component) in parameter.components.iter().enumerate() {
        let shift = match component.byte_order {
            ByteOrder::MsbFirst => 8 * (bytes - index - 1),
            ByteOrder::LsbFirst => 8 * index,
        };
        let slot = usize::from(base - 1) + usize::from(component.offset);
        if slot >= 512 {
            return Err(FixtureError::Invalid(
                "encoded parameter exceeds universe".into(),
            ));
        }
        frame[slot] = ((encoded >> shift) & 0xff) as u8;
    }
    Ok(())
}

pub fn apply_virtual_dimmer(channels: &mut [f32], emitter_indices: &[usize], intensity: f32) {
    let intensity = intensity.clamp(0.0, 1.0);
    for index in emitter_indices {
        if let Some(channel) = channels.get_mut(*index) {
            *channel = (*channel * intensity).clamp(0.0, 1.0);
        }
    }
}

pub fn srgb_to_xyz(red: f32, green: f32, blue: f32) -> Xyz {
    let linear = |value: f32| {
        let value = value.clamp(0.0, 1.0);
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    let r = linear(red);
    let g = linear(green);
    let b = linear(blue);
    Xyz {
        x: 0.412_456_4 * r + 0.357_576_1 * g + 0.180_437_5 * b,
        y: 0.212_672_9 * r + 0.715_152_2 * g + 0.072_175 * b,
        z: 0.019_333_9 * r + 0.119_192 * g + 0.950_304_1 * b,
    }
}

/// Finds bounded emitter levels using projected gradient descent. This supports arbitrary RGBW/A/UV
/// emitter sets without assuming that extra emitters are merely white-channel extraction.
pub fn mix_color(target: Xyz, calibration: &ColorCalibration) -> Result<Vec<f32>, FixtureError> {
    if calibration.emitters.is_empty() {
        return Err(FixtureError::Invalid(
            "color calibration has no emitters".into(),
        ));
    }
    let target = multiply_matrix(calibration.correction_matrix, target);
    let mut levels = vec![0.0_f32; calibration.emitters.len()];
    let norm = calibration
        .emitters
        .iter()
        .map(|emitter| emitter.xyz.x.powi(2) + emitter.xyz.y.powi(2) + emitter.xyz.z.powi(2))
        .sum::<f32>()
        .max(0.001);
    let rate = 0.8 / norm;
    for _ in 0..256 {
        let produced = calibration.emitters.iter().zip(&levels).fold(
            Xyz {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            |sum, (emitter, level)| Xyz {
                x: sum.x + emitter.xyz.x * level,
                y: sum.y + emitter.xyz.y * level,
                z: sum.z + emitter.xyz.z * level,
            },
        );
        let error = Xyz {
            x: produced.x - target.x,
            y: produced.y - target.y,
            z: produced.z - target.z,
        };
        for (level, emitter) in levels.iter_mut().zip(&calibration.emitters) {
            let gradient =
                2.0 * (error.x * emitter.xyz.x + error.y * emitter.xyz.y + error.z * emitter.xyz.z);
            *level = (*level - rate * gradient).clamp(0.0, emitter.limit);
        }
    }
    Ok(levels)
}

fn identity_matrix() -> [[f32; 3]; 3] {
    [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
}
fn multiply_matrix(matrix: [[f32; 3]; 3], value: Xyz) -> Xyz {
    Xyz {
        x: matrix[0][0] * value.x + matrix[0][1] * value.y + matrix[0][2] * value.z,
        y: matrix[1][0] * value.x + matrix[1][1] * value.y + matrix[1][2] * value.z,
        z: matrix[2][0] * value.x + matrix[2][1] * value.y + matrix[2][2] * value.z,
    }
}

pub struct FixtureLibrary {
    conn: Connection,
}

pub type LegacyFixtureProfileSource = (String, String, Option<Vec<u8>>);

/// Ownership marker for profiles generated from the desk's built-in Generic catalog. Catalog
/// upgrades use this marker and deterministic legacy IDs; manufacturer text is never an ownership
/// signal because operators may create their own fixtures whose manufacturer is also `Generic`.
pub const BUILTIN_GENERIC_RESERVED_SOURCE: &str = "builtin:generic-catalog";
const BUILTIN_GENERIC_CATALOG_VERSION: &str = "5";

impl FixtureLibrary {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, FixtureError> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA foreign_keys=ON;
             CREATE TABLE IF NOT EXISTS fixture_definitions(id TEXT NOT NULL,revision INTEGER NOT NULL,manufacturer TEXT NOT NULL,model TEXT NOT NULL,mode TEXT NOT NULL,definition_json TEXT NOT NULL,source_gdtf BLOB,PRIMARY KEY(id,revision));
             CREATE TABLE IF NOT EXISTS fixture_profiles(id TEXT NOT NULL,revision INTEGER NOT NULL,manufacturer TEXT NOT NULL,name TEXT NOT NULL,profile_json TEXT NOT NULL,reserved_source TEXT,PRIMARY KEY(id,revision));
             CREATE TABLE IF NOT EXISTS fixture_profile_sources(profile_id TEXT NOT NULL,profile_revision INTEGER NOT NULL,source_gdtf BLOB NOT NULL,PRIMARY KEY(profile_id,profile_revision));
             CREATE TABLE IF NOT EXISTS fixture_profile_legacy_sources(profile_id TEXT NOT NULL,profile_revision INTEGER NOT NULL,legacy_id TEXT NOT NULL,legacy_revision INTEGER NOT NULL,definition_json TEXT NOT NULL,source_gdtf BLOB,PRIMARY KEY(profile_id,profile_revision,legacy_id,legacy_revision));
             CREATE TABLE IF NOT EXISTS fixture_profile_legacy_map(legacy_id TEXT NOT NULL,legacy_revision INTEGER NOT NULL,profile_id TEXT NOT NULL,profile_revision INTEGER NOT NULL,PRIMARY KEY(legacy_id,legacy_revision));
             CREATE TABLE IF NOT EXISTS fixture_profile_migration_failures(legacy_id TEXT NOT NULL,legacy_revision INTEGER NOT NULL,error TEXT NOT NULL,PRIMARY KEY(legacy_id,legacy_revision));
             CREATE TABLE IF NOT EXISTS fixture_library_warnings(id INTEGER PRIMARY KEY AUTOINCREMENT,message TEXT NOT NULL UNIQUE);
             CREATE TABLE IF NOT EXISTS library_metadata(key TEXT PRIMARY KEY,value TEXT NOT NULL);",
        )?;
        if conn
            .prepare("SELECT source_gdtf FROM fixture_definitions LIMIT 0")
            .is_err()
        {
            conn.execute(
                "ALTER TABLE fixture_definitions ADD COLUMN source_gdtf BLOB",
                [],
            )?;
        }
        let library = Self { conn };
        library.migrate_legacy_profiles()?;
        Ok(library)
    }
    pub fn import_json(&self, json: &str) -> Result<FixtureDefinition, FixtureError> {
        self.import_json_with_source(json, None)
    }
    pub fn import_json_with_source(
        &self,
        json: &str,
        source_gdtf: Option<&[u8]>,
    ) -> Result<FixtureDefinition, FixtureError> {
        let fixture: FixtureDefinition = serde_json::from_str(json)?;
        fixture.validate()?;
        self.conn.execute("INSERT INTO fixture_definitions(id,revision,manufacturer,model,mode,definition_json,source_gdtf) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(id,revision) DO UPDATE SET manufacturer=excluded.manufacturer,model=excluded.model,mode=excluded.mode,definition_json=excluded.definition_json,source_gdtf=COALESCE(excluded.source_gdtf,fixture_definitions.source_gdtf)",params![fixture.id.0.to_string(),fixture.revision,fixture.manufacturer,fixture.model,fixture.mode,json,source_gdtf])?;
        self.migrate_legacy_profiles()?;
        Ok(fixture)
    }

    /// Latest complete profile revisions, one atomic record per manufacturer fixture family.
    pub fn profiles(&self) -> Result<Vec<FixtureProfile>, FixtureError> {
        let mut statement = self.conn.prepare(
            "SELECT p.profile_json FROM fixture_profiles p JOIN (SELECT id,MAX(revision) revision FROM fixture_profiles GROUP BY id) latest ON latest.id=p.id AND latest.revision=p.revision ORDER BY p.manufacturer COLLATE NOCASE,p.name COLLATE NOCASE",
        )?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        rows.map(|row| Ok(serde_json::from_str(&row?)?)).collect()
    }

    /// Resolves every ordered mode in each latest profile to the portable definition snapshot
    /// consumed by patching. Legacy rows whose migration failed remain patchable while the
    /// corresponding warning explains how to recover or repair them.
    pub fn patchable_definitions(&self) -> Result<Vec<FixtureDefinition>, FixtureError> {
        let mut definitions = Vec::new();
        for profile in self.profiles()? {
            for mode in &profile.modes {
                definitions.push(
                    profile
                        .resolved_definition(mode.id)
                        .map_err(|error| FixtureError::Invalid(error.to_string()))?,
                );
            }
        }
        let mut statement = self.conn.prepare(
            "SELECT f.definition_json FROM fixture_definitions f JOIN fixture_profile_migration_failures x ON x.legacy_id=f.id AND x.legacy_revision=f.revision ORDER BY f.manufacturer COLLATE NOCASE,f.model COLLATE NOCASE,f.mode COLLATE NOCASE",
        )?;
        let failures = statement.query_map([], |row| row.get::<_, String>(0))?;
        for json in failures {
            definitions.push(serde_json::from_str(&json?)?);
        }
        definitions.sort_by(|left, right| {
            (
                left.manufacturer.to_lowercase(),
                left.name.to_lowercase(),
                left.mode.to_lowercase(),
            )
                .cmp(&(
                    right.manufacturer.to_lowercase(),
                    right.name.to_lowercase(),
                    right.mode.to_lowercase(),
                ))
        });
        Ok(definitions)
    }

    pub fn profile(
        &self,
        id: FixtureId,
        revision: u32,
    ) -> Result<Option<FixtureProfile>, FixtureError> {
        self.conn
            .query_row(
                "SELECT profile_json FROM fixture_profiles WHERE id=?1 AND revision=?2",
                params![id.0.to_string(), revision],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(|json| serde_json::from_str(&json).map_err(FixtureError::from))
            .transpose()
    }

    pub fn profile_revisions(&self, id: FixtureId) -> Result<Vec<u32>, FixtureError> {
        let mut statement = self
            .conn
            .prepare("SELECT revision FROM fixture_profiles WHERE id=?1 ORDER BY revision")?;
        Ok(statement
            .query_map([id.0.to_string()], |row| row.get(0))?
            .collect::<Result<_, _>>()?)
    }

    /// Deletes one immutable fixture-profile revision. Patched shows remain unaffected because
    /// they carry their own profile/mode snapshot rather than consulting the live library.
    pub fn delete_profile(&self, id: FixtureId, revision: u32) -> Result<bool, FixtureError> {
        self.conn.execute(
            "DELETE FROM fixture_profile_sources WHERE profile_id=?1 AND profile_revision=?2",
            params![id.0.to_string(), revision],
        )?;
        Ok(self.conn.execute(
            "DELETE FROM fixture_profiles WHERE id=?1 AND revision=?2",
            params![id.0.to_string(), revision],
        )? == 1)
    }

    /// Stores a whole profile as one immutable revision. The server/library assigns the revision;
    /// clients can only state which current revision they edited.
    pub fn save_profile(
        &self,
        mut profile: FixtureProfile,
        expected_revision: u32,
    ) -> Result<FixtureProfile, FixtureError> {
        let current = self.conn.query_row(
            "SELECT COALESCE(MAX(revision),0) FROM fixture_profiles WHERE id=?1",
            [profile.id.0.to_string()],
            |row| row.get::<_, u32>(0),
        )?;
        if current != expected_revision {
            return Err(FixtureError::RevisionConflict {
                expected: expected_revision,
                current,
            });
        }
        profile.revision = current + 1;
        profile.schema_version = FIXTURE_PROFILE_SCHEMA_VERSION;
        profile
            .validate()
            .map_err(|error| FixtureError::Invalid(error.to_string()))?;
        let json = serde_json::to_string(&profile)?;
        self.conn.execute(
            "INSERT INTO fixture_profiles(id,revision,manufacturer,name,profile_json,reserved_source) VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                profile.id.0.to_string(),
                profile.revision,
                profile.manufacturer,
                profile.name,
                json,
                profile.reserved_source,
            ],
        )?;
        if current > 0 {
            self.conn.execute(
                "INSERT OR IGNORE INTO fixture_profile_sources(profile_id,profile_revision,source_gdtf) SELECT profile_id,?2,source_gdtf FROM fixture_profile_sources WHERE profile_id=?1 AND profile_revision=?3",
                params![profile.id.0.to_string(), profile.revision, current],
            )?;
        }
        Ok(profile)
    }

    /// Retain the original GDTF archive independently from the normalized editable profile.
    pub fn set_profile_source_gdtf(
        &self,
        id: FixtureId,
        revision: u32,
        source: &[u8],
    ) -> Result<bool, FixtureError> {
        let exists = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM fixture_profiles WHERE id=?1 AND revision=?2)",
            params![id.0.to_string(), revision],
            |row| row.get::<_, bool>(0),
        )?;
        if !exists {
            return Ok(false);
        }
        self.conn.execute(
            "INSERT INTO fixture_profile_sources(profile_id,profile_revision,source_gdtf) VALUES(?1,?2,?3) ON CONFLICT(profile_id,profile_revision) DO UPDATE SET source_gdtf=excluded.source_gdtf",
            params![id.0.to_string(), revision, source],
        )?;
        Ok(true)
    }

    pub fn profile_source_gdtf(
        &self,
        id: FixtureId,
        revision: u32,
    ) -> Result<Option<Vec<u8>>, FixtureError> {
        self.conn
            .query_row(
                "SELECT source_gdtf FROM fixture_profile_sources WHERE profile_id=?1 AND profile_revision=?2",
                params![id.0.to_string(), revision],
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn migration_warnings(&self) -> Result<Vec<String>, FixtureError> {
        let mut statement = self
            .conn
            .prepare("SELECT message FROM fixture_library_warnings ORDER BY id")?;
        Ok(statement
            .query_map([], |row| row.get(0))?
            .collect::<Result<_, _>>()?)
    }

    pub fn profile_legacy_sources(
        &self,
        id: FixtureId,
        revision: u32,
    ) -> Result<Vec<LegacyFixtureProfileSource>, FixtureError> {
        let mut statement = self.conn.prepare(
            "SELECT legacy_id,definition_json,source_gdtf FROM fixture_profile_legacy_sources WHERE profile_id=?1 AND profile_revision=?2 ORDER BY legacy_id,legacy_revision",
        )?;
        Ok(statement
            .query_map(params![id.0.to_string(), revision], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<Result<_, _>>()?)
    }

    fn migrate_legacy_profiles(&self) -> Result<usize, FixtureError> {
        #[derive(Clone)]
        struct LegacyRow {
            id: String,
            revision: u32,
            json: String,
            source: Option<Vec<u8>>,
            definition: FixtureDefinition,
        }
        let rows = {
            let mut statement = self.conn.prepare(
                "SELECT f.id,f.revision,f.definition_json,f.source_gdtf FROM fixture_definitions f JOIN (SELECT id,MAX(revision) revision FROM fixture_definitions GROUP BY id) latest ON latest.id=f.id AND latest.revision=f.revision LEFT JOIN fixture_profile_legacy_map m ON m.legacy_id=f.id AND m.legacy_revision=f.revision LEFT JOIN fixture_profile_migration_failures x ON x.legacy_id=f.id AND x.legacy_revision=f.revision WHERE m.legacy_id IS NULL AND x.legacy_id IS NULL ORDER BY f.manufacturer COLLATE NOCASE,f.model COLLATE NOCASE,f.mode COLLATE NOCASE",
            )?;
            let mapped = statement.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, u32>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<Vec<u8>>>(3)?,
                ))
            })?;
            mapped.collect::<Result<Vec<_>, _>>()?
        };
        let mut valid_rows = Vec::new();
        for (id, revision, json, source) in rows {
            match serde_json::from_str(&json) {
                Ok(definition) => valid_rows.push(LegacyRow {
                    id,
                    revision,
                    json,
                    source,
                    definition,
                }),
                Err(error) => {
                    let message = format!(
                        "Legacy fixture {id} revision {revision} could not be migrated: {error}. The original definition and GDTF source were retained."
                    );
                    self.conn.execute(
                        "INSERT OR REPLACE INTO fixture_profile_migration_failures(legacy_id,legacy_revision,error) VALUES(?1,?2,?3)",
                        params![id, revision, error.to_string()],
                    )?;
                    self.conn.execute(
                        "INSERT OR IGNORE INTO fixture_library_warnings(message) VALUES(?1)",
                        [message],
                    )?;
                }
            }
        }
        if valid_rows.is_empty() {
            return Ok(0);
        }
        let mut families = BTreeMap::<String, Vec<LegacyRow>>::new();
        for row in valid_rows {
            let definition = &row.definition;
            let metadata = serde_json::to_string(&serde_json::json!({
                "device_type": definition.device_type,
                "name": definition.name,
                "physical": definition.physical,
                "model_asset": definition.model_asset,
                "icon_asset": definition.icon_asset,
                "hazardous": definition.hazardous,
                "direct_control_protocols": definition.direct_control_protocols,
                "signal_loss_policy": definition.signal_loss_policy,
            }))?;
            let family = format!(
                "{}\0{}",
                definition.manufacturer.to_lowercase(),
                definition.model.to_lowercase()
            );
            families
                .entry(format!("{family}\0{metadata}"))
                .or_default()
                .push(row);
        }
        let family_counts =
            families
                .keys()
                .fold(HashMap::<String, usize>::new(), |mut counts, key| {
                    let family = key.split('\0').take(2).collect::<Vec<_>>().join("\0");
                    *counts.entry(family).or_default() += 1;
                    counts
                });
        let transaction = self.conn.unchecked_transaction()?;
        let mut migrated = 0;
        for (_key, rows) in families {
            let definitions = rows
                .iter()
                .map(|row| row.definition.clone())
                .collect::<Vec<_>>();
            let mut profile = match FixtureProfile::from_legacy_modes(&definitions) {
                Ok(profile) => profile,
                Err(error) => {
                    let message = format!(
                        "Legacy fixture family {} {} could not be migrated: {error}. Original rows and GDTF sources were retained.",
                        rows[0].definition.manufacturer, rows[0].definition.model
                    );
                    transaction.execute(
                        "INSERT OR IGNORE INTO fixture_library_warnings(message) VALUES(?1)",
                        [message],
                    )?;
                    for row in &rows {
                        transaction.execute(
                            "INSERT OR REPLACE INTO fixture_profile_migration_failures(legacy_id,legacy_revision,error) VALUES(?1,?2,?3)",
                            params![row.id, row.revision, error.to_string()],
                        )?;
                    }
                    continue;
                }
            };
            while transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM fixture_profiles WHERE id=?1 AND revision=1)",
                [profile.id.0.to_string()],
                |row| row.get::<_, bool>(0),
            )? {
                profile.id = FixtureId::new();
            }
            let profile_json = serde_json::to_string(&profile)?;
            transaction.execute(
                "INSERT INTO fixture_profiles(id,revision,manufacturer,name,profile_json,reserved_source) VALUES(?1,1,?2,?3,?4,NULL)",
                params![profile.id.0.to_string(), profile.manufacturer, profile.name, profile_json],
            )?;
            for row in &rows {
                transaction.execute(
                    "INSERT INTO fixture_profile_legacy_sources(profile_id,profile_revision,legacy_id,legacy_revision,definition_json,source_gdtf) VALUES(?1,1,?2,?3,?4,?5)",
                    params![profile.id.0.to_string(), row.id, row.revision, row.json, row.source],
                )?;
                transaction.execute(
                    "INSERT INTO fixture_profile_legacy_map(legacy_id,legacy_revision,profile_id,profile_revision) VALUES(?1,?2,?3,1)",
                    params![row.id, row.revision, profile.id.0.to_string()],
                )?;
            }
            let family = format!(
                "{}\0{}",
                rows[0].definition.manufacturer.to_lowercase(),
                rows[0].definition.model.to_lowercase()
            );
            if family_counts.get(&family).copied().unwrap_or(1) > 1 {
                transaction.execute(
                    "INSERT OR IGNORE INTO fixture_library_warnings(message) VALUES(?1)",
                    [format!(
                        "{} {} contained conflicting fixture-level metadata; its legacy modes were retained as separate profiles",
                        rows[0].definition.manufacturer, rows[0].definition.model
                    )],
                )?;
            }
            migrated += 1;
        }
        transaction.execute(
            "INSERT INTO library_metadata(key,value) VALUES('fixture_profile_schema','2') ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [],
        )?;
        transaction.commit()?;
        Ok(migrated)
    }
    pub fn source_gdtf(
        &self,
        id: FixtureId,
        revision: u32,
    ) -> Result<Option<Vec<u8>>, FixtureError> {
        if let Some(source) = self.profile_source_gdtf(id, revision)? {
            return Ok(Some(source));
        }
        self.conn
            .query_row(
                "SELECT source_gdtf FROM fixture_definitions WHERE id=?1 AND revision=?2",
                params![id.0.to_string(), revision],
                |row| row.get(0),
            )
            .optional()
            .map(|value| value.flatten())
            .map_err(Into::into)
    }
    pub fn export_json(
        &self,
        id: FixtureId,
        revision: u32,
    ) -> Result<Option<String>, FixtureError> {
        self.conn
            .query_row(
                "SELECT definition_json FROM fixture_definitions WHERE id=?1 AND revision=?2",
                params![id.0.to_string(), revision],
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
    }
    pub fn revisions(&self, id: FixtureId) -> Result<Vec<u32>, FixtureError> {
        let mut statement = self
            .conn
            .prepare("SELECT revision FROM fixture_definitions WHERE id=?1 ORDER BY revision")?;
        Ok(statement
            .query_map([id.0.to_string()], |row| row.get(0))?
            .collect::<Result<_, _>>()?)
    }
    pub fn definitions(&self) -> Result<Vec<FixtureDefinition>, FixtureError> {
        let mut statement = self.conn.prepare(
            "SELECT f.definition_json FROM fixture_definitions f JOIN (SELECT id,MAX(revision) revision FROM fixture_definitions GROUP BY id) latest ON latest.id=f.id AND latest.revision=f.revision ORDER BY f.manufacturer COLLATE NOCASE, f.model COLLATE NOCASE, f.mode COLLATE NOCASE",
        )?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        rows.map(|row| Ok(serde_json::from_str(&row?)?)).collect()
    }
    pub fn delete(&self, id: FixtureId, revision: u32) -> Result<bool, FixtureError> {
        Ok(self.conn.execute(
            "DELETE FROM fixture_definitions WHERE id=?1 AND revision=?2",
            params![id.0.to_string(), revision],
        )? == 1)
    }
    pub fn ensure_builtin_generics(&mut self) -> Result<usize, FixtureError> {
        self.conn.execute_batch("CREATE TABLE IF NOT EXISTS library_metadata(key TEXT PRIMARY KEY,value TEXT NOT NULL);")?;
        let installed_version = self
            .conn
            .query_row(
                "SELECT value FROM library_metadata WHERE key='generic_catalog_version'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let installed_profile_count = self
            .conn
            .query_row(
                "SELECT value FROM library_metadata WHERE key='generic_catalog_profile_count'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .and_then(|value| value.parse::<usize>().ok());
        let reserved_profiles = self.conn.query_row(
            "SELECT COUNT(DISTINCT id) FROM fixture_profiles WHERE reserved_source=?1",
            [BUILTIN_GENERIC_RESERVED_SOURCE],
            |row| row.get::<_, usize>(0),
        )?;
        if installed_version.as_deref() == Some(BUILTIN_GENERIC_CATALOG_VERSION)
            && installed_profile_count == Some(reserved_profiles)
            && reserved_profiles > 0
        {
            return Ok(0);
        }
        let definitions = generic_fixture_definitions();
        let mut families = BTreeMap::<String, Vec<FixtureDefinition>>::new();
        for definition in &definitions {
            families
                .entry(format!(
                    "{}\0{}\0{}",
                    definition.model, definition.device_type, definition.name
                ))
                .or_default()
                .push(definition.clone());
        }
        let mut profiles = Vec::with_capacity(families.len());
        for definitions in families.values_mut() {
            definitions.sort_by_key(|definition| definition.mode.to_lowercase());
            let mut profile = FixtureProfile::from_legacy_modes(definitions)
                .map_err(|error| FixtureError::Invalid(error.to_string()))?;
            profile.reserved_source = Some(BUILTIN_GENERIC_RESERVED_SOURCE.into());
            profiles.push((profile, definitions.clone()));
        }
        let transaction = self.conn.transaction()?;
        // Never delete by manufacturer/name: user-authored Generic fixtures are normal protected
        // library content. Built-ins have deterministic legacy IDs and a reserved-source marker,
        // so catalog upgrades replace only entries they own.
        let builtin_ids = definitions
            .iter()
            .map(|fixture| fixture.id.0.to_string())
            .collect::<HashSet<_>>();
        let mut previous_profile_ids = {
            let mut statement = transaction
                .prepare("SELECT DISTINCT id FROM fixture_profiles WHERE reserved_source=?1")?;
            statement
                .query_map([BUILTIN_GENERIC_RESERVED_SOURCE], |row| {
                    row.get::<_, String>(0)
                })?
                .collect::<Result<HashSet<_>, _>>()?
        };
        {
            let mut find_profile = transaction.prepare(
                "SELECT profile_id FROM fixture_profile_legacy_map WHERE legacy_id=?1 AND legacy_revision=1",
            )?;
            for id in &builtin_ids {
                if let Some(profile_id) = find_profile
                    .query_row([id], |row| row.get::<_, String>(0))
                    .optional()?
                {
                    previous_profile_ids.insert(profile_id);
                }
            }
        }
        for profile_id in previous_profile_ids {
            transaction.execute(
                "DELETE FROM fixture_profile_sources WHERE profile_id=?1",
                [&profile_id],
            )?;
            transaction.execute(
                "DELETE FROM fixture_profile_legacy_sources WHERE profile_id=?1",
                [&profile_id],
            )?;
            transaction.execute(
                "DELETE FROM fixture_profile_legacy_map WHERE profile_id=?1",
                [&profile_id],
            )?;
            transaction.execute("DELETE FROM fixture_profiles WHERE id=?1", [&profile_id])?;
        }
        for fixture in &definitions {
            let json = serde_json::to_string(fixture)?;
            transaction.execute("INSERT INTO fixture_definitions(id,revision,manufacturer,model,mode,definition_json) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(id,revision) DO UPDATE SET manufacturer=excluded.manufacturer,model=excluded.model,mode=excluded.mode,definition_json=excluded.definition_json", params![fixture.id.0.to_string(),fixture.revision,fixture.manufacturer,fixture.model,fixture.mode,json])?;
        }
        for (profile, legacy_definitions) in &profiles {
            let profile_json = serde_json::to_string(profile)?;
            transaction.execute(
                "INSERT INTO fixture_profiles(id,revision,manufacturer,name,profile_json,reserved_source) VALUES(?1,1,?2,?3,?4,?5)",
                params![profile.id.0.to_string(), profile.manufacturer, profile.name, profile_json, BUILTIN_GENERIC_RESERVED_SOURCE],
            )?;
            for definition in legacy_definitions {
                let definition_json = serde_json::to_string(definition)?;
                transaction.execute(
                    "INSERT OR REPLACE INTO fixture_profile_legacy_sources(profile_id,profile_revision,legacy_id,legacy_revision,definition_json,source_gdtf) VALUES(?1,1,?2,1,?3,NULL)",
                    params![profile.id.0.to_string(), definition.id.0.to_string(), definition_json],
                )?;
                transaction.execute(
                    "INSERT OR REPLACE INTO fixture_profile_legacy_map(legacy_id,legacy_revision,profile_id,profile_revision) VALUES(?1,1,?2,1)",
                    params![definition.id.0.to_string(), profile.id.0.to_string()],
                )?;
                transaction.execute(
                    "DELETE FROM fixture_profile_migration_failures WHERE legacy_id=?1 AND legacy_revision=1",
                    [definition.id.0.to_string()],
                )?;
            }
        }
        transaction.execute("INSERT INTO library_metadata(key,value) VALUES('generic_catalog_version',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [BUILTIN_GENERIC_CATALOG_VERSION])?;
        transaction.execute("INSERT INTO library_metadata(key,value) VALUES('generic_catalog_profile_count',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [profiles.len().to_string()])?;
        transaction.commit()?;
        Ok(profiles.len())
    }
}

fn permutations(values: &[(&str, &str)]) -> Vec<Vec<(String, String)>> {
    fn visit(
        remaining: Vec<(&str, &str)>,
        current: &mut Vec<(String, String)>,
        output: &mut Vec<Vec<(String, String)>>,
    ) {
        if remaining.is_empty() {
            output.push(current.clone());
            return;
        }
        for index in 0..remaining.len() {
            let mut next = remaining.clone();
            let value = next.remove(index);
            current.push((value.0.into(), value.1.into()));
            visit(next, current, output);
            current.pop();
        }
    }
    let mut output = Vec::new();
    visit(values.to_vec(), &mut Vec::new(), &mut output);
    output
}

fn generic_definition(
    name: &str,
    device_type: &str,
    mode: String,
    channels: &[(String, String)],
    virtual_dimmer: bool,
    resolution: usize,
) -> FixtureDefinition {
    let mut offset = 0_u16;
    let mut parameters = channels
        .iter()
        .map(|(_label, attribute)| {
            let bytes = resolution;
            let start = offset;
            offset += bytes as u16;
            Parameter {
                attribute: AttributeKey(attribute.clone()),
                components: (0..bytes)
                    .map(|component| ChannelComponent {
                        offset: start + component as u16,
                        byte_order: ByteOrder::MsbFirst,
                    })
                    .collect(),
                default: 0.0,
                virtual_dimmer: virtual_dimmer && attribute.starts_with("color."),
                metadata: ParameterMetadata::default(),
                capabilities: if attribute == "fog" {
                    vec![Capability {
                        name: "Off to full output".into(),
                        dmx_from: 0,
                        dmx_to: 255,
                        preset_family: Some("beam".into()),
                    }]
                } else if attribute == "switch" {
                    vec![
                        Capability {
                            name: "Off".into(),
                            dmx_from: 0,
                            dmx_to: 127,
                            preset_family: Some("control".into()),
                        },
                        Capability {
                            name: "On".into(),
                            dmx_from: 128,
                            dmx_to: 255,
                            preset_family: Some("control".into()),
                        },
                    ]
                } else {
                    Vec::new()
                },
            }
        })
        .collect::<Vec<_>>();
    if virtual_dimmer {
        parameters.insert(
            0,
            Parameter {
                attribute: AttributeKey::intensity(),
                components: Vec::new(),
                default: 1.0,
                virtual_dimmer: true,
                metadata: ParameterMetadata::default(),
                capabilities: Vec::new(),
            },
        );
    }
    let id = FixtureId(profile::stable_uuid(&format!(
        "builtin-generic\0{name}\0{mode}\0{resolution}\0{}",
        channels
            .iter()
            .map(|(_, attribute)| attribute.as_str())
            .collect::<Vec<_>>()
            .join(",")
    )));
    FixtureDefinition {
        schema_version: 1,
        id,
        revision: 1,
        manufacturer: "Generic".into(),
        device_type: device_type.into(),
        name: name.into(),
        model: name.into(),
        mode,
        footprint: offset,
        heads: vec![LogicalHead {
            index: 0,
            name: "Main".into(),
            shared: true,
            parameters,
        }],
        color_calibration: None,
        physical: FixturePhysicalProperties::default(),
        model_asset: None,
        icon_asset: None,
        hazardous: false,
        direct_control_protocols: Vec::new(),
        signal_loss_policy: SignalLossPolicy::HoldLast,
        safe_values: BTreeMap::new(),
        profile_id: None,
        mode_id: None,
        profile_snapshot: None,
    }
}

fn led_modes(name: &str, emitters: &[(&str, &str)]) -> Vec<FixtureDefinition> {
    let mut result = Vec::new();
    for order in permutations(emitters) {
        let label = order
            .iter()
            .map(|(label, _)| label.as_str())
            .collect::<String>();
        let mut dimmer_first = vec![("D".into(), "intensity".into())];
        dimmer_first.extend(order.clone());
        result.push(generic_definition(
            name,
            "wash",
            format!("D{label} 8-bit dimmer first"),
            &dimmer_first,
            false,
            1,
        ));
        let mut dimmer_last = order.clone();
        dimmer_last.push(("D".into(), "intensity".into()));
        result.push(generic_definition(
            name,
            "wash",
            format!("{label}D 8-bit dimmer last"),
            &dimmer_last,
            false,
            1,
        ));
        result.push(generic_definition(
            name,
            "wash",
            format!("{label} virtual dimmer"),
            &order,
            true,
            1,
        ));
    }
    result
}

fn scanner_definition() -> FixtureDefinition {
    let mut fixture = generic_definition(
        "Mirror Mover Scanner",
        "scanner",
        "Dimmer, Pan, Tilt".into(),
        &[
            ("D".into(), "intensity".into()),
            ("P".into(), "pan".into()),
            ("T".into(), "tilt".into()),
        ],
        false,
        1,
    );
    for parameter in fixture
        .heads
        .iter_mut()
        .flat_map(|head| &mut head.parameters)
    {
        if parameter.attribute.0 == "pan" || parameter.attribute.0 == "tilt" {
            parameter.default = 0.5;
        }
    }
    fixture.physical.pan_range_degrees = Some(180.0);
    fixture.physical.tilt_range_degrees = Some(120.0);
    fixture
}

/// Built-in profiles are normal library entries, grouped as named modes by manufacturer/model.
pub fn generic_fixture_definitions() -> Vec<FixtureDefinition> {
    let mut result = vec![
        generic_definition(
            "Dimmer",
            "dimmer",
            "8-bit".into(),
            &[("D".into(), "intensity".into())],
            false,
            1,
        ),
        generic_definition(
            "Dimmer",
            "dimmer",
            "16-bit".into(),
            &[("D".into(), "intensity".into())],
            false,
            2,
        ),
        generic_definition(
            "Fogger",
            "fogger",
            "Fog 8-bit".into(),
            &[("F".into(), "fog".into())],
            false,
            1,
        ),
        generic_definition(
            "Hazer",
            "fogger",
            "Fan, Fog".into(),
            &[("Fan".into(), "fan".into()), ("Fog".into(), "fog".into())],
            false,
            1,
        ),
        generic_definition(
            "Fan",
            "other",
            "Fan 8-bit".into(),
            &[("F".into(), "fan".into())],
            false,
            1,
        ),
        generic_definition(
            "Relay",
            "other",
            "Off / On".into(),
            &[("S".into(), "switch".into())],
            false,
            1,
        ),
        generic_definition(
            "Hazer",
            "fogger",
            "Fog, Fan".into(),
            &[("Fog".into(), "fog".into()), ("Fan".into(), "fan".into())],
            false,
            1,
        ),
        generic_definition(
            "Strobe",
            "strobe",
            "Dimmer, Strobe".into(),
            &[
                ("D".into(), "intensity".into()),
                ("S".into(), "strobe".into()),
            ],
            false,
            1,
        ),
        scanner_definition(),
        generic_definition(
            "Strobe",
            "strobe",
            "Strobe, Dimmer".into(),
            &[
                ("S".into(), "strobe".into()),
                ("D".into(), "intensity".into()),
            ],
            false,
            1,
        ),
        generic_definition(
            "Pan Tilt",
            "other",
            "Pan Tilt 8-bit".into(),
            &[("P".into(), "pan".into()), ("T".into(), "tilt".into())],
            false,
            1,
        ),
        generic_definition(
            "Pan Tilt",
            "other",
            "Pan Tilt 16-bit".into(),
            &[("P".into(), "pan".into()), ("T".into(), "tilt".into())],
            false,
            2,
        ),
    ];
    result.extend(led_modes(
        "RGB LED",
        &[
            ("R", "color.red"),
            ("G", "color.green"),
            ("B", "color.blue"),
        ],
    ));
    result.extend(led_modes(
        "RGBW LED",
        &[
            ("R", "color.red"),
            ("G", "color.green"),
            ("B", "color.blue"),
            ("W", "color.white"),
        ],
    ));
    result.extend(led_modes(
        "RGBCCT LED",
        &[
            ("R", "color.red"),
            ("G", "color.green"),
            ("B", "color.blue"),
            ("C", "color.cold_white"),
            ("W", "color.warm_white"),
        ],
    ));
    result.extend(led_modes(
        "RGBWA LED",
        &[
            ("R", "color.red"),
            ("G", "color.green"),
            ("B", "color.blue"),
            ("W", "color.white"),
            ("A", "color.amber"),
        ],
    ));
    result.extend(led_modes(
        "RGBWAUV LED",
        &[
            ("R", "color.red"),
            ("G", "color.green"),
            ("B", "color.blue"),
            ("W", "color.white"),
            ("A", "color.amber"),
            ("U", "color.uv"),
        ],
    ));
    result.extend(led_modes(
        "CCT LED",
        &[("C", "color.cold_white"), ("W", "color.warm_white")],
    ));
    result.extend(led_modes(
        "CMY LED",
        &[
            ("C", "color.cyan"),
            ("M", "color.magenta"),
            ("Y", "color.yellow"),
        ],
    ));
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_legacy_metre_locations_and_current_millimetre_locations() {
        let legacy: FixtureLocation =
            serde_json::from_str(r#"{"x":1.25,"y":-0.5,"z":0.00000001}"#).unwrap();
        assert_eq!(
            legacy,
            FixtureLocation {
                x: 1_250,
                y: -500,
                z: 0
            }
        );
        let current: FixtureLocation =
            serde_json::from_str(r#"{"x":1250,"y":-500,"z":0}"#).unwrap();
        assert_eq!(current, legacy);
        assert_eq!(
            serde_json::to_string(&current).unwrap(),
            r#"{"x":1250,"y":-500,"z":0}"#
        );
    }

    fn definition(footprint: u16) -> FixtureDefinition {
        FixtureDefinition {
            schema_version: 1,
            id: FixtureId::new(),
            revision: 1,
            manufacturer: "Test".into(),
            device_type: "other".into(),
            name: "Lamp".into(),
            model: "Lamp".into(),
            mode: "Mode".into(),
            footprint,
            heads: vec![LogicalHead {
                index: 0,
                name: "Main".into(),
                shared: true,
                parameters: vec![],
            }],
            color_calibration: None,
            physical: FixturePhysicalProperties::default(),
            model_asset: None,
            icon_asset: None,
            hazardous: false,
            direct_control_protocols: Vec::new(),
            signal_loss_policy: SignalLossPolicy::HoldLast,
            safe_values: BTreeMap::new(),
            profile_id: None,
            mode_id: None,
            profile_snapshot: None,
        }
    }

    #[test]
    fn encodes_16_bit_msb_first_at_one_based_address() {
        let p = Parameter {
            attribute: AttributeKey("pan".into()),
            components: vec![
                ChannelComponent {
                    offset: 0,
                    byte_order: ByteOrder::MsbFirst,
                },
                ChannelComponent {
                    offset: 1,
                    byte_order: ByteOrder::MsbFirst,
                },
            ],
            default: 0.0,
            virtual_dimmer: false,
            metadata: ParameterMetadata::default(),
            capabilities: vec![],
        };
        let mut frame = [0; 512];
        encode_parameter(&mut frame, 1, &p, 0.5).unwrap();
        assert_eq!(&frame[..2], &[128, 0]);
    }
    #[test]
    fn encoder_applies_fixture_inversion_and_transfer_curve() {
        let parameter = Parameter {
            attribute: AttributeKey::intensity(),
            components: vec![ChannelComponent {
                offset: 0,
                byte_order: ByteOrder::MsbFirst,
            }],
            default: 0.0,
            virtual_dimmer: false,
            metadata: ParameterMetadata {
                invert: true,
                curve: DmxCurve::Square,
                ..ParameterMetadata::default()
            },
            capabilities: vec![],
        };
        let mut frame = [0; 512];
        encode_parameter(&mut frame, 1, &parameter, 0.25).unwrap();
        assert_eq!(frame[0], 143);
    }
    #[test]
    fn rejects_patch_overlap_and_boundary_overflow() {
        let def = definition(10);
        let first = PatchedFixture {
            fixture_id: FixtureId::new(),
            fixture_number: None,
            name: "First".into(),
            definition: def.clone(),
            universe: Some(1),
            address: Some(1),
            split_patches: vec![],
            layer_id: default_patch_layer(),
            direct_control: None,
            location: Default::default(),
            rotation: Default::default(),
            logical_heads: vec![],
            multipatch: vec![],
            move_in_black_enabled: true,
            move_in_black_delay_millis: 0,
            highlight_overrides: BTreeMap::new(),
        };
        let overlap = PatchedFixture {
            fixture_id: FixtureId::new(),
            fixture_number: None,
            name: "Overlap".into(),
            definition: def.clone(),
            universe: Some(1),
            address: Some(10),
            split_patches: vec![],
            layer_id: default_patch_layer(),
            direct_control: None,
            location: Default::default(),
            rotation: Default::default(),
            logical_heads: vec![],
            multipatch: vec![],
            move_in_black_enabled: true,
            move_in_black_delay_millis: 0,
            highlight_overrides: BTreeMap::new(),
        };
        assert!(validate_patch(&[first.clone(), overlap]).is_err());
        let overflow = PatchedFixture {
            fixture_id: FixtureId::new(),
            fixture_number: None,
            name: "Overflow".into(),
            definition: def,
            universe: Some(1),
            address: Some(504),
            split_patches: vec![],
            layer_id: default_patch_layer(),
            direct_control: None,
            location: Default::default(),
            rotation: Default::default(),
            logical_heads: vec![],
            multipatch: vec![],
            move_in_black_enabled: true,
            move_in_black_delay_millis: 0,
            highlight_overrides: BTreeMap::new(),
        };
        assert!(validate_patch(&[overflow]).is_err());
        assert!(validate_patch(&[first]).is_ok());
    }
    #[test]
    fn multipatch_reserves_real_addresses_and_allows_visualizer_only_instances() {
        let mut fixture = PatchedFixture {
            fixture_id: FixtureId::new(),
            fixture_number: None,
            name: "Multi".into(),
            definition: definition(3),
            universe: Some(1),
            address: Some(1),
            split_patches: vec![],
            layer_id: default_patch_layer(),
            direct_control: None,
            location: Default::default(),
            rotation: Default::default(),
            logical_heads: vec![],
            multipatch: vec![
                MultiPatchInstance {
                    id: Uuid::new_v4(),
                    name: "Output".into(),
                    universe: Some(1),
                    address: Some(10),
                    split_patches: vec![],
                    location: Default::default(),
                    rotation: Default::default(),
                },
                MultiPatchInstance {
                    id: Uuid::new_v4(),
                    name: "Visual".into(),
                    universe: None,
                    address: None,
                    split_patches: vec![],
                    location: Default::default(),
                    rotation: Default::default(),
                },
            ],
            move_in_black_enabled: true,
            move_in_black_delay_millis: 0,
            highlight_overrides: BTreeMap::new(),
        };
        assert!(validate_patch(std::slice::from_ref(&fixture)).is_ok());
        fixture.multipatch[1].universe = Some(1);
        assert!(validate_patch(std::slice::from_ref(&fixture)).is_err());
        fixture.multipatch[1].address = Some(2);
        assert!(validate_patch(&[fixture]).is_err());
    }

    fn schema_v2_two_split_fixture() -> PatchedFixture {
        let mut profile = FixtureProfile::blank();
        profile.revision = 1;
        profile.manufacturer = "Test".into();
        profile.name = "Two split".into();
        let mode_id = profile.modes[0].id;
        profile.modes[0].splits.push(FixtureSplit {
            number: 2,
            footprint: 1,
        });
        profile.modes[0].heads.push(FixtureHead {
            id: Uuid::new_v4(),
            name: "Second".into(),
            master_shared: false,
            split: 2,
        });
        let definition = profile.resolved_definition(mode_id).unwrap();
        PatchedFixture {
            fixture_id: FixtureId::new(),
            fixture_number: Some(1),
            name: "Two split".into(),
            definition,
            universe: None,
            address: None,
            split_patches: vec![
                SplitPatch {
                    split: 1,
                    universe: Some(1),
                    address: Some(1),
                },
                SplitPatch {
                    split: 2,
                    universe: None,
                    address: None,
                },
            ],
            layer_id: default_patch_layer(),
            direct_control: None,
            location: Default::default(),
            rotation: Default::default(),
            logical_heads: vec![],
            multipatch: vec![MultiPatchInstance {
                id: Uuid::new_v4(),
                name: "Second body".into(),
                universe: None,
                address: None,
                split_patches: vec![
                    SplitPatch {
                        split: 1,
                        universe: Some(1),
                        address: Some(10),
                    },
                    SplitPatch {
                        split: 2,
                        universe: None,
                        address: None,
                    },
                ],
                location: Default::default(),
                rotation: Default::default(),
            }],
            move_in_black_enabled: true,
            move_in_black_delay_millis: 0,
            highlight_overrides: BTreeMap::new(),
        }
    }

    #[test]
    fn schema_v2_multi_split_requires_exact_optional_assignments_for_every_instance() {
        let fixture = schema_v2_two_split_fixture();
        validate_patch(std::slice::from_ref(&fixture)).unwrap();

        let mut missing_parent = fixture.clone();
        missing_parent.split_patches.pop();
        assert!(
            validate_patch(&[missing_parent])
                .unwrap_err()
                .to_string()
                .contains("missing split 2")
        );

        let mut duplicate = fixture.clone();
        duplicate.split_patches[1].split = 1;
        assert!(
            validate_patch(&[duplicate])
                .unwrap_err()
                .to_string()
                .contains("more than once")
        );

        let mut unknown = fixture.clone();
        unknown.split_patches[1].split = 99;
        assert!(
            validate_patch(&[unknown])
                .unwrap_err()
                .to_string()
                .contains("unknown split 99")
        );

        let mut partial = fixture.clone();
        partial.split_patches[1].universe = Some(2);
        assert!(
            validate_patch(&[partial])
                .unwrap_err()
                .to_string()
                .contains("both universe and address or neither")
        );

        let mut missing_multipatch = fixture;
        missing_multipatch.multipatch[0].split_patches.clear();
        assert!(
            validate_patch(&[missing_multipatch])
                .unwrap_err()
                .to_string()
                .contains("must assign every split")
        );
    }
    #[test]
    fn media_server_layers_inherit_parent_direct_control_endpoint() {
        let endpoint = DirectControlEndpoint {
            protocol: DirectControlProtocol::Citp,
            ip_address: "192.0.2.20".parse().unwrap(),
            port: 4811,
        };
        let mut media_definition = definition(1);
        media_definition.direct_control_protocols = vec![DirectControlProtocol::Citp];
        let parent = PatchedFixture {
            fixture_id: FixtureId::new(),
            fixture_number: None,
            name: "Media".into(),
            definition: media_definition,
            universe: Some(1),
            address: Some(1),
            split_patches: vec![],
            layer_id: default_patch_layer(),
            direct_control: Some(endpoint.clone()),
            location: Default::default(),
            rotation: Default::default(),
            logical_heads: vec![PatchedHead {
                head_index: 1,
                fixture_id: FixtureId::new(),
            }],
            multipatch: vec![],
            move_in_black_enabled: true,
            move_in_black_delay_millis: 0,
            highlight_overrides: BTreeMap::new(),
        };
        validate_patch(std::slice::from_ref(&parent)).unwrap();
        assert_eq!(parent.direct_control, Some(endpoint));
        assert_eq!(parent.logical_heads.len(), 1);
        let mut unsupported = parent.clone();
        unsupported.definition.direct_control_protocols.clear();
        assert!(
            validate_patch(&[unsupported])
                .unwrap_err()
                .to_string()
                .contains("does not support")
        );
    }
    #[test]
    fn logical_head_reconciliation_preserves_matching_ids_and_repairs_shape() {
        let kept = FixtureId::new();
        let stale = FixtureId::new();
        let mut fixture = PatchedFixture {
            fixture_id: FixtureId::new(),
            fixture_number: Some(100),
            name: "Multi".into(),
            definition: definition(2),
            universe: Some(1),
            address: Some(1),
            split_patches: vec![],
            layer_id: default_patch_layer(),
            direct_control: None,
            location: Default::default(),
            rotation: Default::default(),
            logical_heads: vec![
                PatchedHead {
                    head_index: 0,
                    fixture_id: kept,
                },
                PatchedHead {
                    head_index: 99,
                    fixture_id: stale,
                },
            ],
            multipatch: vec![],
            move_in_black_enabled: true,
            move_in_black_delay_millis: 0,
            highlight_overrides: BTreeMap::new(),
        };
        fixture.definition.heads = vec![
            LogicalHead {
                index: 10,
                name: "Master".into(),
                shared: true,
                parameters: vec![],
            },
            LogicalHead {
                index: 0,
                name: "Cell 1".into(),
                shared: false,
                parameters: vec![],
            },
            LogicalHead {
                index: 4,
                name: "Cell 2".into(),
                shared: false,
                parameters: vec![],
            },
        ];
        assert!(reconcile_logical_heads(&mut fixture));
        assert_eq!(fixture.logical_heads.len(), 2);
        assert_eq!(fixture.logical_heads[0].head_index, 0);
        assert_eq!(fixture.logical_heads[0].fixture_id, kept);
        assert_eq!(fixture.logical_heads[1].head_index, 4);
        assert_ne!(fixture.logical_heads[1].fixture_id, stale);
        assert!(!reconcile_logical_heads(&mut fixture));
    }
    #[test]
    fn virtual_dimmer_preserves_color_ratios() {
        let mut channels = [0.8, 0.4, 0.2, 1.0];
        apply_virtual_dimmer(&mut channels, &[0, 1, 2], 0.5);
        assert_eq!(channels, [0.4, 0.2, 0.1, 1.0]);
    }
    #[test]
    fn calibrated_rgb_reconstructs_target_xyz() {
        let calibration = ColorCalibration {
            emitters: vec![
                EmitterCalibration {
                    name: "R".into(),
                    xyz: Xyz {
                        x: 0.412_456_4,
                        y: 0.212_672_9,
                        z: 0.019_333_9,
                    },
                    limit: 1.0,
                },
                EmitterCalibration {
                    name: "G".into(),
                    xyz: Xyz {
                        x: 0.357_576_1,
                        y: 0.715_152_2,
                        z: 0.119_192,
                    },
                    limit: 1.0,
                },
                EmitterCalibration {
                    name: "B".into(),
                    xyz: Xyz {
                        x: 0.180_437_5,
                        y: 0.072_175,
                        z: 0.950_304_1,
                    },
                    limit: 1.0,
                },
            ],
            correction_matrix: identity_matrix(),
        };
        let levels = mix_color(srgb_to_xyz(1.0, 0.0, 0.0), &calibration).unwrap();
        assert!(levels[0] > 0.98);
        assert!(levels[1] < 0.02);
        assert!(levels[2] < 0.02);
    }
    #[test]
    fn fixture_json_round_trips_through_library() {
        let path =
            std::env::temp_dir().join(format!("fixture-library-{}.sqlite", uuid::Uuid::new_v4()));
        let library = FixtureLibrary::open(&path).unwrap();
        let fixture = definition(1);
        let json = serde_json::to_string(&fixture).unwrap();
        library.import_json(&json).unwrap();
        assert_eq!(library.export_json(fixture.id, 1).unwrap().unwrap(), json);
        let profiles = library.profiles().unwrap();
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].modes[0].name, "Mode");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn profile_revisions_are_atomic_and_server_assigned() {
        let path = std::env::temp_dir().join(format!("fixture-profiles-{}.sqlite", Uuid::new_v4()));
        let library = FixtureLibrary::open(&path).unwrap();
        let mut draft = FixtureProfile::blank();
        draft.manufacturer = "Acme".into();
        draft.name = "Orbit".into();
        draft.short_name = "Orbit".into();
        let first = library.save_profile(draft, 0).unwrap();
        assert_eq!(first.revision, 1);
        assert!(
            library
                .set_profile_source_gdtf(first.id, 1, b"original-gdtf-archive")
                .unwrap()
        );
        assert_eq!(
            library.profile_source_gdtf(first.id, 1).unwrap().as_deref(),
            Some(b"original-gdtf-archive".as_slice())
        );
        let mut edit = first.clone();
        edit.notes = "Second revision".into();
        let second = library.save_profile(edit, 1).unwrap();
        assert_eq!(second.revision, 2);
        assert_eq!(
            library.profile_source_gdtf(first.id, 2).unwrap().as_deref(),
            Some(b"original-gdtf-archive".as_slice()),
            "new immutable revisions retain the original import archive"
        );
        assert_eq!(library.profile(first.id, 1).unwrap().unwrap().notes, "");
        assert_eq!(
            library.profile(first.id, 2).unwrap().unwrap().notes,
            "Second revision"
        );
        assert!(matches!(
            library.save_profile(second, 1),
            Err(FixtureError::RevisionConflict {
                expected: 1,
                current: 2
            })
        ));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn embedded_legacy_patch_migrates_to_portable_profile_and_explicit_split_assignments() {
        let mut legacy = definition(4);
        legacy.revision = 7;
        let intensity = Parameter {
            attribute: AttributeKey::intensity(),
            components: vec![ChannelComponent {
                offset: 0,
                byte_order: ByteOrder::MsbFirst,
            }],
            default: 0.0,
            virtual_dimmer: false,
            metadata: ParameterMetadata::default(),
            capabilities: vec![Capability {
                name: "Open".into(),
                dmx_from: 1,
                dmx_to: 255,
                preset_family: Some("beam".into()),
            }],
        };
        let emitter_parameter = |name: &str, offset| Parameter {
            attribute: AttributeKey(format!("color.emitter.{name}")),
            components: vec![ChannelComponent {
                offset,
                byte_order: ByteOrder::MsbFirst,
            }],
            default: 0.0,
            virtual_dimmer: false,
            metadata: ParameterMetadata::default(),
            capabilities: vec![],
        };
        legacy.heads[0].parameters = vec![
            intensity,
            emitter_parameter("red", 1),
            emitter_parameter("green", 2),
            emitter_parameter("blue", 3),
        ];
        legacy.color_calibration = Some(ColorCalibration {
            emitters: vec![
                EmitterCalibration {
                    name: "red".into(),
                    xyz: Xyz {
                        x: 1.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    limit: 0.8,
                },
                EmitterCalibration {
                    name: "green".into(),
                    xyz: Xyz {
                        x: 0.0,
                        y: 1.0,
                        z: 0.0,
                    },
                    limit: 0.9,
                },
                EmitterCalibration {
                    name: "blue".into(),
                    xyz: Xyz {
                        x: 0.0,
                        y: 0.0,
                        z: 1.0,
                    },
                    limit: 1.0,
                },
            ],
            correction_matrix: [[0.9, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.1]],
        });
        let instance_id = Uuid::new_v4();
        let mut fixture = PatchedFixture {
            fixture_id: FixtureId::new(),
            fixture_number: Some(1),
            name: "Legacy".into(),
            definition: legacy,
            universe: Some(2),
            address: Some(101),
            split_patches: vec![],
            layer_id: default_patch_layer(),
            direct_control: None,
            location: Default::default(),
            rotation: Default::default(),
            logical_heads: vec![],
            multipatch: vec![MultiPatchInstance {
                id: instance_id,
                name: "Balcony".into(),
                universe: Some(3),
                address: Some(201),
                split_patches: vec![],
                location: Default::default(),
                rotation: Default::default(),
            }],
            move_in_black_enabled: true,
            move_in_black_delay_millis: 0,
            highlight_overrides: BTreeMap::new(),
        };

        assert!(migrate_patched_fixture_to_v2(&mut fixture).unwrap());
        assert_eq!(
            fixture.definition.schema_version,
            FIXTURE_PROFILE_SCHEMA_VERSION
        );
        assert_eq!(fixture.definition.revision, 7);
        assert!(fixture.definition.profile_snapshot.is_some());
        let migrated_mode = &fixture.definition.profile_snapshot.as_ref().unwrap().modes[0];
        let ColorSystem::Additive { emitters } = &migrated_mode.color_systems[0].system else {
            panic!("legacy additive calibration was not converted")
        };
        assert_eq!(emitters.len(), 3);
        assert_eq!(emitters[0].maximum_level, 0.8);
        assert_eq!(emitters[0].response_curve, 1.0);
        assert!(emitters.iter().all(|emitter| emitter.visible));
        assert_eq!(migrated_mode.color_systems[0].correction_matrix[0][0], 0.9);
        assert_eq!(
            fixture.definition.heads[0].parameters[0].capabilities[0].name,
            "Open"
        );
        assert_eq!(
            fixture.split_patches,
            vec![SplitPatch {
                split: 1,
                universe: Some(2),
                address: Some(101),
            }]
        );
        assert_eq!(
            fixture.multipatch[0].split_patches,
            vec![SplitPatch {
                split: 1,
                universe: Some(3),
                address: Some(201),
            }]
        );
        assert!(!migrate_patched_fixture_to_v2(&mut fixture).unwrap());
    }

    #[test]
    fn legacy_library_migration_combines_compatible_modes_and_retains_sources() {
        let path = std::env::temp_dir().join(format!(
            "fixture-profile-migration-{}.sqlite",
            Uuid::new_v4()
        ));
        let connection = Connection::open(&path).unwrap();
        connection.execute_batch("CREATE TABLE fixture_definitions(id TEXT NOT NULL,revision INTEGER NOT NULL,manufacturer TEXT NOT NULL,model TEXT NOT NULL,mode TEXT NOT NULL,definition_json TEXT NOT NULL,source_gdtf BLOB,PRIMARY KEY(id,revision));").unwrap();
        let mut coarse = definition(1);
        coarse.manufacturer = "Acme".into();
        coarse.model = "Orbit".into();
        coarse.name = "Orbit".into();
        coarse.mode = "Coarse".into();
        let mut fine = coarse.clone();
        fine.id = FixtureId::new();
        fine.mode = "Fine".into();
        for fixture in [&coarse, &fine] {
            connection.execute(
                "INSERT INTO fixture_definitions(id,revision,manufacturer,model,mode,definition_json,source_gdtf) VALUES(?1,1,?2,?3,?4,?5,?6)",
                params![fixture.id.0.to_string(), fixture.manufacturer, fixture.model, fixture.mode, serde_json::to_string(fixture).unwrap(), b"retained-gdtf".as_slice()],
            ).unwrap();
        }
        drop(connection);
        let library = FixtureLibrary::open(&path).unwrap();
        let profiles = library.profiles().unwrap();
        assert_eq!(profiles.len(), 1);
        assert_eq!(
            profiles[0]
                .modes
                .iter()
                .map(|mode| mode.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Coarse", "Fine"]
        );
        let sources = library.profile_legacy_sources(profiles[0].id, 1).unwrap();
        assert_eq!(sources.len(), 2);
        assert!(
            sources
                .iter()
                .all(|(_, json, source)| json.contains("Orbit")
                    && source.as_deref() == Some(b"retained-gdtf".as_slice()))
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn failed_or_conflicting_legacy_migration_keeps_startup_available() {
        let path = std::env::temp_dir().join(format!(
            "fixture-profile-recovery-{}.sqlite",
            Uuid::new_v4()
        ));
        let connection = Connection::open(&path).unwrap();
        connection.execute_batch("CREATE TABLE fixture_definitions(id TEXT NOT NULL,revision INTEGER NOT NULL,manufacturer TEXT NOT NULL,model TEXT NOT NULL,mode TEXT NOT NULL,definition_json TEXT NOT NULL,source_gdtf BLOB,PRIMARY KEY(id,revision));").unwrap();
        let mut first = definition(1);
        first.manufacturer = "Acme".into();
        first.model = "Conflict".into();
        first.name = "Conflict".into();
        let mut second = first.clone();
        second.id = FixtureId::new();
        second.mode = "Different metadata".into();
        second.physical.width_millimetres = Some(500.0);
        for fixture in [&first, &second] {
            connection.execute(
                "INSERT INTO fixture_definitions(id,revision,manufacturer,model,mode,definition_json) VALUES(?1,1,?2,?3,?4,?5)",
                params![fixture.id.0.to_string(), fixture.manufacturer, fixture.model, fixture.mode, serde_json::to_string(fixture).unwrap()],
            ).unwrap();
        }
        let invalid_id = FixtureId::new();
        connection.execute(
            "INSERT INTO fixture_definitions(id,revision,manufacturer,model,mode,definition_json,source_gdtf) VALUES(?1,1,'Broken','Broken','Broken','{',?2)",
            params![invalid_id.0.to_string(), b"original-source".as_slice()],
        ).unwrap();
        drop(connection);
        let library = FixtureLibrary::open(&path).unwrap();
        assert_eq!(library.profiles().unwrap().len(), 2);
        let warnings = library.migration_warnings().unwrap();
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("conflicting fixture-level metadata"))
        );
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("could not be migrated"))
        );
        assert_eq!(
            library.export_json(invalid_id, 1).unwrap().as_deref(),
            Some("{")
        );
        assert_eq!(
            library.source_gdtf(invalid_id, 1).unwrap().as_deref(),
            Some(b"original-source".as_slice())
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn generic_catalog_groups_named_modes_and_covers_led_permutations() {
        let catalog = generic_fixture_definitions();
        assert_eq!(catalog.len(), 3_006);
        assert!(
            catalog
                .iter()
                .any(|fixture| fixture.name == "Mirror Mover Scanner"
                    && fixture.device_type == "scanner")
        );
        assert!(catalog.iter().all(|fixture| fixture.validate().is_ok()));
        let dimmers = catalog
            .iter()
            .filter(|fixture| fixture.name == "Dimmer")
            .collect::<Vec<_>>();
        assert_eq!(dimmers.len(), 2);
        assert!(
            dimmers
                .iter()
                .any(|fixture| fixture.mode == "8-bit" && fixture.footprint == 1)
        );
        assert!(dimmers.iter().any(|fixture| fixture.mode == "16-bit"
            && fixture.footprint == 2
            && fixture.heads[0].parameters[0].components.len() == 2));
        let rgb = catalog
            .iter()
            .filter(|fixture| fixture.name == "RGB LED")
            .collect::<Vec<_>>();
        assert_eq!(rgb.len(), 18);
        assert!(
            rgb.iter()
                .any(|fixture| fixture.mode == "DRGB 8-bit dimmer first")
        );
        assert!(
            rgb.iter()
                .any(|fixture| fixture.mode == "RBGD 8-bit dimmer last")
        );
        assert!(rgb.iter().any(|fixture| {
            fixture.mode == "BGR virtual dimmer"
                && fixture.footprint == 3
                && fixture.heads[0].parameters.iter().any(|parameter| {
                    parameter.attribute.is_intensity()
                        && parameter.components.is_empty()
                        && parameter.virtual_dimmer
                })
                && fixture.heads[0]
                    .parameters
                    .iter()
                    .filter(|parameter| parameter.attribute.0.starts_with("color."))
                    .all(|parameter| parameter.virtual_dimmer)
        }));
        assert_eq!(
            catalog
                .iter()
                .filter(|fixture| fixture.name == "Hazer")
                .count(),
            2
        );
        assert!(catalog.iter().any(|fixture| fixture.name == "Relay"
            && fixture.heads[0].parameters[0].capabilities.len() == 2));
        assert_eq!(
            catalog
                .iter()
                .filter(|fixture| fixture.name == "CCT LED")
                .count(),
            6
        );
        assert_eq!(
            catalog
                .iter()
                .filter(|fixture| fixture.name == "CMY LED")
                .count(),
            18
        );
        assert!(catalog.iter().any(
            |fixture| fixture.name == "RGBWAUV LED" && fixture.mode == "UAWGBR virtual dimmer"
        ));
    }

    #[test]
    fn generic_catalog_upgrade_owns_only_reserved_profiles() {
        let path = std::env::temp_dir().join(format!(
            "fixture-profile-generic-upgrade-{}.sqlite",
            Uuid::new_v4()
        ));
        let mut library = FixtureLibrary::open(&path).unwrap();

        // Reproduce an older installation where deterministic built-in definitions had already
        // been migrated into ordinary, unmarked profiles.
        let old_dimmers = generic_fixture_definitions()
            .into_iter()
            .filter(|definition| definition.name == "Dimmer")
            .collect::<Vec<_>>();
        for definition in &old_dimmers {
            let json = serde_json::to_string(definition).unwrap();
            library.conn.execute(
                "INSERT INTO fixture_definitions(id,revision,manufacturer,model,mode,definition_json) VALUES(?1,1,?2,?3,?4,?5)",
                params![definition.id.0.to_string(), definition.manufacturer, definition.model, definition.mode, json],
            ).unwrap();
        }
        library.migrate_legacy_profiles().unwrap();
        let old_profile = library
            .profiles()
            .unwrap()
            .into_iter()
            .find(|profile| profile.name == "Dimmer")
            .unwrap();
        assert_eq!(old_profile.reserved_source, None);
        library.conn.execute(
            "INSERT INTO library_metadata(key,value) VALUES('generic_catalog_version','4') ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [],
        ).unwrap();

        // Manufacturer text is deliberately not enough to make a profile catalog-owned.
        let mut user_profile = FixtureProfile::blank();
        user_profile.manufacturer = "Generic".into();
        user_profile.name = "Operator relay".into();
        user_profile.short_name = "Relay".into();
        let user_profile = library.save_profile(user_profile, 0).unwrap();

        let expected_profile_count = generic_fixture_definitions()
            .into_iter()
            .map(|definition| (definition.model, definition.device_type, definition.name))
            .collect::<HashSet<_>>()
            .len();
        assert_eq!(
            library.ensure_builtin_generics().unwrap(),
            expected_profile_count
        );
        let profiles = library.profiles().unwrap();
        assert_eq!(
            profiles
                .iter()
                .filter(|profile| profile.reserved_source.as_deref()
                    == Some(BUILTIN_GENERIC_RESERVED_SOURCE))
                .count(),
            expected_profile_count
        );
        assert_eq!(
            library
                .profile(user_profile.id, user_profile.revision)
                .unwrap()
                .unwrap()
                .reserved_source,
            None
        );
        assert_eq!(library.ensure_builtin_generics().unwrap(), 0);

        drop(library);
        let mut reopened = FixtureLibrary::open(&path).unwrap();
        assert_eq!(reopened.ensure_builtin_generics().unwrap(), 0);
        assert!(
            reopened
                .profiles()
                .unwrap()
                .iter()
                .any(|profile| profile.id == user_profile.id
                    && profile.manufacturer == "Generic"
                    && profile.reserved_source.is_none())
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn legacy_patch_defaults_move_in_black_on_and_round_trips_explicit_settings() {
        let legacy = serde_json::json!({
            "fixture_id": FixtureId::new(),
            "definition": definition(2)
        });
        let mut fixture: PatchedFixture = serde_json::from_value(legacy).unwrap();
        assert!(fixture.move_in_black_enabled);
        assert_eq!(fixture.move_in_black_delay_millis, 0);

        fixture.move_in_black_enabled = false;
        fixture.move_in_black_delay_millis = 1_250;
        let restored: PatchedFixture =
            serde_json::from_value(serde_json::to_value(fixture).unwrap()).unwrap();
        assert!(!restored.move_in_black_enabled);
        assert_eq!(restored.move_in_black_delay_millis, 1_250);
    }

    #[test]
    fn legacy_patch_inherits_highlight_look_and_round_trips_instance_overrides() {
        let legacy = serde_json::json!({
            "fixture_id": FixtureId::new(),
            "definition": definition(2)
        });
        let mut fixture: PatchedFixture = serde_json::from_value(legacy).unwrap();
        assert!(fixture.highlight_overrides.is_empty());

        let channel_id = Uuid::new_v4();
        fixture.highlight_overrides.insert(channel_id, 173);
        let restored: PatchedFixture =
            serde_json::from_value(serde_json::to_value(fixture).unwrap()).unwrap();
        assert_eq!(restored.highlight_overrides.get(&channel_id), Some(&173));
    }
}
