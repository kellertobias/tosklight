use crate::*;
use light_core::AttributeKey;
use serde::{Deserialize, Serialize};

/// One ordered lane list, one clock and one body per lane. Typed lanes never carry a scalar
/// shadow address/value. The legacy body keeps its inactive configurations for mode switching.
#[derive(Clone, Debug, PartialEq)]
pub enum DynamicLaneBody {
    LegacyScalar(LegacyScalarLaneBody),
    Programming(ProgrammingLaneBody),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegacyScalarLaneBody {
    pub attribute: AttributeKey,
    pub mode: DynamicLaneMode,
    pub keyframes: KeyframeConfiguration,
    pub max_min: MaxMinConfiguration,
    pub middle_amplitude: MiddleAmplitudeConfiguration,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingLaneBody {
    pub address: DynamicValueAddress,
    pub configuration: ProgrammingLaneConfiguration,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "mode",
    content = "configuration",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ProgrammingLaneConfiguration {
    Keyframes(KeyframeConfiguration<DynamicValueSource>),
    MaxMin(MaxMinConfiguration<DynamicValueSource>),
    MiddleAmplitude(MiddleAmplitudeConfiguration<DynamicValueSource, DynamicValue>),
    Random,
}

impl ProgrammingLaneConfiguration {
    pub const fn mode(&self) -> DynamicLaneMode {
        match self {
            Self::Keyframes(_) => DynamicLaneMode::Keyframes,
            Self::MaxMin(_) => DynamicLaneMode::MaxMin,
            Self::MiddleAmplitude(_) => DynamicLaneMode::MiddleAmplitude,
            Self::Random => DynamicLaneMode::Random,
        }
    }
}

impl DynamicLane {
    /// Whether this lane drives a native channel of `attribute`: a legacy scalar lane its own
    /// attribute, a semantic Position lane Pan and Tilt, a semantic Color lane any colour
    /// channel. Focus and Zoom owners name their channel attribute.
    pub fn drives_attribute(&self, attribute: &AttributeKey) -> bool {
        let owner = self.output_owner();
        if owner == *attribute {
            return true;
        }
        match &*owner.0 {
            "position" => matches!(&*attribute.0, "pan" | "tilt"),
            "color" => attribute.0.starts_with("color."),
            _ => false,
        }
    }

    pub fn output_owner(&self) -> AttributeKey {
        self.output_owner_ref().clone()
    }

    /// [`Self::output_owner`] borrowed (TL-639 round 7): frame workers compare it without
    /// touching a shared canonical key's count.
    pub fn output_owner_ref(&self) -> &AttributeKey {
        match &self.body {
            DynamicLaneBody::LegacyScalar(body) => &body.attribute,
            DynamicLaneBody::Programming(body) => body.address.owner().key_ref(),
        }
    }

    pub const fn mode(&self) -> DynamicLaneMode {
        match &self.body {
            DynamicLaneBody::LegacyScalar(body) => body.mode,
            DynamicLaneBody::Programming(body) => body.configuration.mode(),
        }
    }

    pub fn legacy(&self) -> Option<&LegacyScalarLaneBody> {
        match &self.body {
            DynamicLaneBody::LegacyScalar(body) => Some(body),
            DynamicLaneBody::Programming(_) => None,
        }
    }

    pub fn legacy_mut(&mut self) -> Option<&mut LegacyScalarLaneBody> {
        match &mut self.body {
            DynamicLaneBody::LegacyScalar(body) => Some(body),
            DynamicLaneBody::Programming(_) => None,
        }
    }
}

impl DynamicDefinition {
    pub fn required_programming_contract(&self) -> u16 {
        u16::from(
            self.lanes
                .iter()
                .any(|lane| matches!(lane.body, DynamicLaneBody::Programming(_)))
                || self
                    .random_groups
                    .iter()
                    .any(|group| matches!(group.range, DynamicRandomRange::Programming { .. })),
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum DynamicRandomRange {
    LegacyScalar {
        low: ScalarSource,
        high: ScalarSource,
    },
    Programming {
        low: DynamicValueSource,
        high: DynamicValueSource,
    },
}

mod stored;
mod validation;
pub(crate) use validation::validate_random_members;
