use super::{ColorSystem, FIXTURE_PROFILE_SCHEMA_VERSION, FixtureProfile, ProfileError};
use crate::{
    ByteOrder, ChannelComponent, ColorCalibration, EmitterCalibration, FixtureDefinition,
    FixturePhysicalProperties, LogicalHead, Parameter, ParameterMetadata,
};
use std::collections::{BTreeMap, HashMap};
use uuid::Uuid;

impl FixtureProfile {
    pub fn resolved_definition(&self, mode_id: Uuid) -> Result<FixtureDefinition, ProfileError> {
        self.validate()?;
        let mode = self
            .mode(mode_id)
            .ok_or_else(|| ProfileError::Invalid("mode does not exist".into()))?;
        let primary = mode.primary_slots()?;
        let head_indices = mode
            .heads
            .iter()
            .enumerate()
            .map(|(index, head)| (head.id, index as u16))
            .collect::<HashMap<_, _>>();
        let mut heads = Vec::new();
        for (index, head) in mode.heads.iter().enumerate() {
            let parameters = mode
                .channels
                .iter()
                .filter(|channel| channel.head_id == head.id)
                .map(|channel| {
                    let mut slots = vec![*primary.get(&channel.id).expect("validated primary")];
                    slots.extend(channel.secondary_slots.iter().copied());
                    let max = channel.resolution.max_raw();
                    Parameter {
                        attribute: channel.attribute.clone(),
                        components: slots
                            .into_iter()
                            .map(|slot| ChannelComponent {
                                offset: slot - 1,
                                byte_order: ByteOrder::MsbFirst,
                            })
                            .collect(),
                        default: channel.default_raw as f32 / max as f32,
                        virtual_dimmer: channel.reacts_to_virtual_intensity,
                        metadata: ParameterMetadata {
                            physical_min: channel.physical_min.unwrap_or(0.0),
                            physical_max: channel.physical_max.unwrap_or(1.0),
                            unit: channel.unit.clone(),
                            invert: channel.invert,
                            ..Default::default()
                        },
                        capabilities: Vec::new(),
                    }
                })
                .collect();
            heads.push(LogicalHead {
                index: index as u16,
                name: head.name.clone(),
                shared: head.master_shared,
                parameters,
            });
        }
        let color_calibration = mode.color_systems.iter().find_map(|system| {
            let ColorSystem::Additive { emitters } = &system.system else {
                return None;
            };
            Some(ColorCalibration {
                emitters: emitters
                    .iter()
                    .map(|emitter| EmitterCalibration {
                        name: emitter.name.clone(),
                        xyz: emitter.xyz,
                        limit: emitter.maximum_level,
                    })
                    .collect(),
                correction_matrix: system.correction_matrix,
            })
        });
        let footprint = mode
            .splits
            .iter()
            .find(|split| split.number == 1)
            .or_else(|| mode.splits.first())
            .map(|split| split.footprint)
            .unwrap_or(1);
        let _ = head_indices;
        Ok(FixtureDefinition {
            schema_version: FIXTURE_PROFILE_SCHEMA_VERSION,
            id: self.id,
            revision: self.revision,
            manufacturer: self.manufacturer.clone(),
            device_type: self.fixture_type.clone(),
            name: self.name.clone(),
            model: self.short_name.clone(),
            mode: mode.name.clone(),
            footprint,
            heads,
            color_calibration,
            physical: FixturePhysicalProperties {
                width_millimetres: self.physical.width_millimetres,
                height_millimetres: self.physical.height_millimetres,
                depth_millimetres: self.physical.depth_millimetres,
                weight_kilograms: self.physical.weight_kilograms,
                power_watts: self.physical.power_watts,
                ..Default::default()
            },
            model_asset: self.model_asset.clone(),
            icon_asset: self.stage_icon_asset.clone(),
            hazardous: self.hazardous,
            direct_control_protocols: self.direct_control_protocols.clone(),
            signal_loss_policy: self.signal_loss_policy,
            safe_values: BTreeMap::new(),
            profile_id: Some(self.id),
            mode_id: Some(mode.id),
            profile_snapshot: Some(Box::new(self.clone())),
        })
    }
}
