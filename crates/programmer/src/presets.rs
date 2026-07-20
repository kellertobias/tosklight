use light_core::{AttributeKey, AttributeValue, FixtureId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PresetStoreMode {
    Merge,
    Overwrite,
    AddMissingFixtures,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum PresetFamily {
    #[default]
    #[serde(alias = "All", alias = "all")]
    Mixed,
    Intensity,
    Color,
    Position,
    Beam,
}

impl PresetFamily {
    pub const fn type_number(self) -> u8 {
        match self {
            Self::Mixed => 0,
            Self::Intensity => 1,
            Self::Color => 2,
            Self::Position => 3,
            Self::Beam => 4,
        }
    }

    pub fn from_type_number(value: u8) -> Result<Self, String> {
        match value {
            0 => Ok(Self::Mixed),
            1 => Ok(Self::Intensity),
            2 => Ok(Self::Color),
            3 => Ok(Self::Position),
            4 => Ok(Self::Beam),
            _ => Err("preset type must be within 0-4".into()),
        }
    }

    pub fn accepts(self, attribute: &AttributeKey) -> bool {
        use light_core::AttributeClass;

        if self == Self::Mixed {
            return true;
        }
        let class = light_core::attribute_descriptor(attribute).family;
        match self {
            Self::Mixed => true,
            Self::Intensity => {
                attribute.is_intensity()
                    || attribute.0 == "dimmer"
                    || attribute.0.ends_with(".dimmer")
                    || class == AttributeClass::Intensity
            }
            Self::Color => {
                class == AttributeClass::Color
                    || attribute.0 == "color"
                    || attribute.0.starts_with("color.")
                    || attribute.0.contains(".color.")
            }
            Self::Position => attribute.is_position() || class == AttributeClass::Position,
            Self::Beam => {
                matches!(class, AttributeClass::Beam | AttributeClass::Focus)
                    || attribute.0.split('.').any(|part| {
                        matches!(
                            part,
                            "beam"
                                | "focus"
                                | "zoom"
                                | "iris"
                                | "gobo"
                                | "prism"
                                | "frost"
                                | "shaper"
                                | "shutter"
                                | "strobe"
                        )
                    })
            }
        }
    }
}

/// Domain identity of a Preset. `number` is local to its family, so Color 1 and Position 1 are
/// distinct Presets. The dotted `2.1` form is an operator/storage address, not a global ID.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct PresetAddress {
    pub family: PresetFamily,
    pub number: u32,
}

impl PresetAddress {
    pub fn new(family: PresetFamily, number: u32) -> Result<Self, String> {
        if number == 0 {
            return Err("preset numbers start at 1".into());
        }
        Ok(Self { family, number })
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        let (family, number) = value
            .split_once('.')
            .ok_or("expected <preset-type>.<preset-number>")?;
        if number.contains('.') {
            return Err("expected <preset-type>.<preset-number>".into());
        }
        Self::new(
            PresetFamily::from_type_number(
                family.parse::<u8>().map_err(|_| "preset type is invalid")?,
            )?,
            number
                .parse::<u32>()
                .map_err(|_| "preset number is invalid")?,
        )
    }

    pub fn storage_key(self) -> String {
        format!("{}.{}", self.family.type_number(), self.number)
    }

    pub fn from_storage_key(value: &str, legacy_family: PresetFamily) -> Result<Self, String> {
        if value.contains('.') {
            Self::parse(value)
        } else {
            Self::new(
                legacy_family,
                value
                    .parse::<u32>()
                    .map_err(|_| "preset number is invalid")?,
            )
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct Preset {
    pub name: String,
    pub family: PresetFamily,
    /// Pool-local number. Legacy Presets decode as zero until their show-object address supplies
    /// the number during migration/read.
    pub number: u32,
    pub values: HashMap<FixtureId, HashMap<AttributeKey, AttributeValue>>,
    pub group_values: HashMap<String, HashMap<AttributeKey, AttributeValue>>,
}

impl Preset {
    pub fn reconcile_address(&mut self, storage_key: &str) -> Result<PresetAddress, String> {
        let address = PresetAddress::from_storage_key(storage_key, self.family)?;
        if address.family != self.family {
            return Err(format!(
                "preset address family {} does not match stored {:?} family",
                address.family.type_number(),
                self.family
            ));
        }
        if self.number != 0 && self.number != address.number {
            return Err(format!(
                "preset address number {} does not match stored number {}",
                address.number, self.number
            ));
        }
        self.number = address.number;
        Ok(address)
    }

    pub fn retain_family_attributes(&mut self) {
        let family = self.family;
        for attributes in self.values.values_mut() {
            attributes.retain(|attribute, _| family.accepts(attribute));
        }
        for attributes in self.group_values.values_mut() {
            attributes.retain(|attribute, _| family.accepts(attribute));
        }
    }

    pub fn store(&mut self, incoming: Preset, mode: PresetStoreMode) {
        if !incoming.name.is_empty() {
            self.name = incoming.name;
        }
        self.family = incoming.family;
        match mode {
            PresetStoreMode::Overwrite => {
                self.values = incoming.values;
                self.group_values = incoming.group_values;
            }
            PresetStoreMode::Merge => {
                for (fixture, attributes) in incoming.values {
                    self.values.entry(fixture).or_default().extend(attributes);
                }
                for (group, attributes) in incoming.group_values {
                    self.group_values
                        .entry(group)
                        .or_default()
                        .extend(attributes);
                }
            }
            PresetStoreMode::AddMissingFixtures => {
                for (fixture, attributes) in incoming.values {
                    self.values.entry(fixture).or_insert(attributes);
                }
                for (group, attributes) in incoming.group_values {
                    self.group_values.entry(group).or_insert(attributes);
                }
            }
        }
        self.retain_family_attributes();
    }
}
