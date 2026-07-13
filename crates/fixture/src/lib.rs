#![forbid(unsafe_code)]
//! Fixture definitions, portable fixture library, color calibration, patching, and DMX encoding.

use light_core::{AttributeKey, AttributeValue, DmxAddress, FixtureId, Universe, Xyz};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
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
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FixturePhysicalProperties {
    pub pan_range_degrees: Option<f32>,
    pub tilt_range_degrees: Option<f32>,
    pub width_millimetres: Option<f32>,
    pub height_millimetres: Option<f32>,
    pub depth_millimetres: Option<f32>,
    pub weight_kilograms: Option<f32>,
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
    pub location: FixtureLocation,
    #[serde(default)]
    pub rotation: FixtureVector,
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
        Ok(Self { x: stored.x, y: stored.y, z: stored.z })
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
            formatter.write_str("an integer millimetre coordinate or a legacy floating-point metre coordinate")
        }
        fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<i32, E> {
            i32::try_from(value).map_err(E::custom)
        }
        fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<i32, E> {
            i32::try_from(value).map_err(E::custom)
        }
        fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<i32, E> {
            let millimetres = value * 1_000.0;
            if !millimetres.is_finite() || millimetres < f64::from(i32::MIN) || millimetres > f64::from(i32::MAX) {
                return Err(E::custom("legacy fixture location is outside the supported range"));
            }
            Ok(millimetres.round() as i32)
        }
    }
    deserializer.deserialize_any(CoordinateVisitor)
}

fn default_patch_layer() -> String { "default".into() }

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

#[derive(Debug, Error)]
pub enum FixtureError {
    #[error("invalid fixture: {0}")]
    Invalid(String),
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl FixtureDefinition {
    pub fn effective_signal_loss_policy(&self) -> SignalLossPolicy {
        if self.hazardous && self.signal_loss_policy == SignalLossPolicy::HoldLast {
            SignalLossPolicy::ImmediateSafe
        } else {
            self.signal_loss_policy
        }
    }
    pub fn validate(&self) -> Result<(), FixtureError> {
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
        let mut patches = vec![(fixture.universe, fixture.address, fixture.fixture_id.0.to_string())];
        patches.extend(fixture.multipatch.iter().map(|instance| (instance.universe, instance.address, instance.id.to_string())));
        for (universe, address, instance) in patches {
            if universe.is_some() != address.is_some() {
                return Err(FixtureError::Invalid(format!("multipatch instance {instance} must set both universe and address or neither")));
            }
            let (Some(universe), Some(address)) = (universe, address) else { continue };
            if address == 0 || usize::from(address) + usize::from(fixture.definition.footprint) - 1 > 512 {
                return Err(FixtureError::Invalid(format!("fixture instance {instance} exceeds universe {universe}")));
            }
            let slots = used.entry(universe).or_insert([false; 512]);
            let start = usize::from(address - 1);
            for (offset, slot) in slots[start..start + usize::from(fixture.definition.footprint)].iter_mut().enumerate() {
                if *slot { return Err(FixtureError::Invalid(format!("patch overlap at universe {} address {}", universe, start + offset + 1))); }
                *slot = true;
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
impl FixtureLibrary {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, FixtureError> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; CREATE TABLE IF NOT EXISTS fixture_definitions(id TEXT NOT NULL,revision INTEGER NOT NULL,manufacturer TEXT NOT NULL,model TEXT NOT NULL,mode TEXT NOT NULL,definition_json TEXT NOT NULL,source_gdtf BLOB,PRIMARY KEY(id,revision));")?;
        if !conn.prepare("SELECT source_gdtf FROM fixture_definitions LIMIT 0").is_ok() { conn.execute("ALTER TABLE fixture_definitions ADD COLUMN source_gdtf BLOB",[])?; }
        Ok(Self { conn })
    }
    pub fn import_json(&self, json: &str) -> Result<FixtureDefinition, FixtureError> {
        self.import_json_with_source(json, None)
    }
    pub fn import_json_with_source(&self, json: &str, source_gdtf: Option<&[u8]>) -> Result<FixtureDefinition, FixtureError> {
        let fixture: FixtureDefinition = serde_json::from_str(json)?;
        fixture.validate()?;
        self.conn.execute("INSERT INTO fixture_definitions(id,revision,manufacturer,model,mode,definition_json,source_gdtf) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(id,revision) DO UPDATE SET manufacturer=excluded.manufacturer,model=excluded.model,mode=excluded.mode,definition_json=excluded.definition_json,source_gdtf=COALESCE(excluded.source_gdtf,fixture_definitions.source_gdtf)",params![fixture.id.0.to_string(),fixture.revision,fixture.manufacturer,fixture.model,fixture.mode,json,source_gdtf])?;
        Ok(fixture)
    }
    pub fn source_gdtf(&self,id:FixtureId,revision:u32)->Result<Option<Vec<u8>>,FixtureError>{self.conn.query_row("SELECT source_gdtf FROM fixture_definitions WHERE id=?1 AND revision=?2",params![id.0.to_string(),revision],|row|row.get(0)).optional().map(|value|value.flatten()).map_err(Into::into)}
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
        if self
            .conn
            .query_row(
                "SELECT value FROM library_metadata WHERE key='generic_catalog_version'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .as_deref()
            == Some("4")
        {
            return Ok(0);
        }
        let definitions = generic_fixture_definitions();
        let transaction = self.conn.transaction()?;
        transaction.execute("DELETE FROM fixture_definitions WHERE manufacturer='Generic' AND model IN ('Dimmer','Fogger','Hazer','Fan','Relay','Pan Tilt','Mirror Mover Scanner','RGB LED','RGBW LED','RGBCCT LED','RGBWA LED','RGBWAUV LED','CCT LED','CMY LED','Strobe')", [])?;
        for fixture in &definitions {
            let json = serde_json::to_string(fixture)?;
            transaction.execute("INSERT INTO fixture_definitions(id,revision,manufacturer,model,mode,definition_json) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(id,revision) DO NOTHING", params![fixture.id.0.to_string(),fixture.revision,fixture.manufacturer,fixture.model,fixture.mode,json])?;
        }
        transaction.execute("INSERT INTO library_metadata(key,value) VALUES('generic_catalog_version','4') ON CONFLICT(key) DO UPDATE SET value=excluded.value", [])?;
        transaction.commit()?;
        Ok(definitions.len())
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
    FixtureDefinition {
        schema_version: 1,
        id: FixtureId::new(),
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
        &[("D".into(), "intensity".into()), ("P".into(), "pan".into()), ("T".into(), "tilt".into())],
        false,
        1,
    );
    for parameter in fixture.heads.iter_mut().flat_map(|head| &mut head.parameters) {
        if parameter.attribute.0 == "pan" || parameter.attribute.0 == "tilt" { parameter.default = 0.5; }
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
        let legacy: FixtureLocation = serde_json::from_str(r#"{"x":1.25,"y":-0.5,"z":0.00000001}"#).unwrap();
        assert_eq!(legacy, FixtureLocation { x: 1_250, y: -500, z: 0 });
        let current: FixtureLocation = serde_json::from_str(r#"{"x":1250,"y":-500,"z":0}"#).unwrap();
        assert_eq!(current, legacy);
        assert_eq!(serde_json::to_string(&current).unwrap(), r#"{"x":1250,"y":-500,"z":0}"#);
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
            layer_id: default_patch_layer(),
            direct_control: None,
            location: Default::default(),
            rotation: Default::default(),
            logical_heads: vec![],
            multipatch: vec![],
        };
        let overlap = PatchedFixture {
            fixture_id: FixtureId::new(),
            fixture_number: None,
            name: "Overlap".into(),
            definition: def.clone(),
            universe: Some(1),
            address: Some(10),
            layer_id: default_patch_layer(),
            direct_control: None,
            location: Default::default(),
            rotation: Default::default(),
            logical_heads: vec![],
            multipatch: vec![],
        };
        assert!(validate_patch(&[first.clone(), overlap]).is_err());
        let overflow = PatchedFixture {
            fixture_id: FixtureId::new(),
            fixture_number: None,
            name: "Overflow".into(),
            definition: def,
            universe: Some(1),
            address: Some(504),
            layer_id: default_patch_layer(),
            direct_control: None,
            location: Default::default(),
            rotation: Default::default(),
            logical_heads: vec![],
            multipatch: vec![],
        };
        assert!(validate_patch(&[overflow]).is_err());
        assert!(validate_patch(&[first]).is_ok());
    }
    #[test]
    fn multipatch_reserves_real_addresses_and_allows_visualizer_only_instances() {
        let mut fixture = PatchedFixture {
            fixture_id: FixtureId::new(), fixture_number: None, name: "Multi".into(), definition: definition(3), universe: Some(1), address: Some(1),
            layer_id: default_patch_layer(), direct_control: None, location: Default::default(), rotation: Default::default(), logical_heads: vec![],
            multipatch: vec![
                MultiPatchInstance { id: Uuid::new_v4(), name: "Output".into(), universe: Some(1), address: Some(10), location: Default::default(), rotation: Default::default() },
                MultiPatchInstance { id: Uuid::new_v4(), name: "Visual".into(), universe: None, address: None, location: Default::default(), rotation: Default::default() },
            ],
        };
        assert!(validate_patch(std::slice::from_ref(&fixture)).is_ok());
        fixture.multipatch[1].universe = Some(1);
        assert!(validate_patch(std::slice::from_ref(&fixture)).is_err());
        fixture.multipatch[1].address = Some(2);
        assert!(validate_patch(&[fixture]).is_err());
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
            layer_id: default_patch_layer(),
            direct_control: Some(endpoint.clone()),
            location: Default::default(),
            rotation: Default::default(),
            logical_heads: vec![PatchedHead {
                head_index: 1,
                fixture_id: FixtureId::new(),
            }],
            multipatch: vec![],
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
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn generic_catalog_groups_named_modes_and_covers_led_permutations() {
        let catalog = generic_fixture_definitions();
        assert_eq!(catalog.len(), 3_006);
        assert!(catalog.iter().any(|fixture| fixture.name == "Mirror Mover Scanner" && fixture.device_type == "scanner"));
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
}
