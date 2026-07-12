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
    pub model: String,
    pub mode: String,
    pub footprint: u16,
    pub heads: Vec<LogicalHead>,
    pub color_calibration: Option<ColorCalibration>,
    pub hazardous: bool,
    /// Direct-control transports explicitly supported by this fixture profile.
    #[serde(default)]
    pub direct_control_protocols: Vec<DirectControlProtocol>,
    #[serde(default)]
    pub signal_loss_policy: SignalLossPolicy,
    pub safe_values: BTreeMap<AttributeKey, AttributeValue>,
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
    pub definition: FixtureDefinition,
    pub universe: Universe,
    /// User-facing DMX address, always 1 through 512.
    pub address: DmxAddress,
    /// Optional direct-control endpoint attached to the physical parent fixture.
    /// Logical heads inherit this endpoint and cannot override it.
    #[serde(default)]
    pub direct_control: Option<DirectControlEndpoint>,
    #[serde(default)]
    pub logical_heads: Vec<PatchedHead>,
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
        for parameter in self.heads.iter().flat_map(|head| &head.parameters) {
            if parameter.components.is_empty() || parameter.components.len() > 4 {
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
    for fixture in fixtures {
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
        if fixture.address == 0
            || usize::from(fixture.address) + usize::from(fixture.definition.footprint) - 1 > 512
        {
            return Err(FixtureError::Invalid(format!(
                "fixture {} exceeds universe {}",
                fixture.fixture_id.0, fixture.universe
            )));
        }
        let slots = used.entry(fixture.universe).or_insert([false; 512]);
        let start = usize::from(fixture.address - 1);
        for (offset, slot) in slots[start..start + usize::from(fixture.definition.footprint)]
            .iter_mut()
            .enumerate()
        {
            if *slot {
                return Err(FixtureError::Invalid(format!(
                    "patch overlap at universe {} address {}",
                    fixture.universe,
                    start + offset + 1
                )));
            }
            *slot = true;
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
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; CREATE TABLE IF NOT EXISTS fixture_definitions(id TEXT NOT NULL,revision INTEGER NOT NULL,manufacturer TEXT NOT NULL,model TEXT NOT NULL,mode TEXT NOT NULL,definition_json TEXT NOT NULL,PRIMARY KEY(id,revision));")?;
        Ok(Self { conn })
    }
    pub fn import_json(&self, json: &str) -> Result<FixtureDefinition, FixtureError> {
        let fixture: FixtureDefinition = serde_json::from_str(json)?;
        fixture.validate()?;
        self.conn.execute("INSERT INTO fixture_definitions(id,revision,manufacturer,model,mode,definition_json) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(id,revision) DO UPDATE SET manufacturer=excluded.manufacturer,model=excluded.model,mode=excluded.mode,definition_json=excluded.definition_json",params![fixture.id.0.to_string(),fixture.revision,fixture.manufacturer,fixture.model,fixture.mode,json])?;
        Ok(fixture)
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn definition(footprint: u16) -> FixtureDefinition {
        FixtureDefinition {
            schema_version: 1,
            id: FixtureId::new(),
            revision: 1,
            manufacturer: "Test".into(),
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
            definition: def.clone(),
            universe: 1,
            address: 1,
            direct_control: None,
            logical_heads: vec![],
        };
        let overlap = PatchedFixture {
            fixture_id: FixtureId::new(),
            definition: def.clone(),
            universe: 1,
            address: 10,
            direct_control: None,
            logical_heads: vec![],
        };
        assert!(validate_patch(&[first.clone(), overlap]).is_err());
        let overflow = PatchedFixture {
            fixture_id: FixtureId::new(),
            definition: def,
            universe: 1,
            address: 504,
            direct_control: None,
            logical_heads: vec![],
        };
        assert!(validate_patch(&[overflow]).is_err());
        assert!(validate_patch(&[first]).is_ok());
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
            definition: media_definition,
            universe: 1,
            address: 1,
            direct_control: Some(endpoint.clone()),
            logical_heads: vec![PatchedHead {
                head_index: 1,
                fixture_id: FixtureId::new(),
            }],
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
}
