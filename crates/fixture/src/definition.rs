use crate::profile::{FIXTURE_PROFILE_SCHEMA_VERSION, PatchPolicy};
use crate::{FixtureDefinition, FixtureError, SignalLossPolicy};
use light_core::{AttributeKey, AttributeValue};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

impl FixtureDefinition {
    pub fn is_dmx_patchable(&self) -> bool {
        self.profile_snapshot
            .as_deref()
            .is_none_or(|profile| profile.patch_policy == PatchPolicy::Dmx)
    }

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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GeneratedPreset {
    pub family: String,
    pub name: String,
    pub head_index: u16,
    pub attribute: AttributeKey,
    pub value: AttributeValue,
}
