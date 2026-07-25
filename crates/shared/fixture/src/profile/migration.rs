use super::color::color_distance;
use super::color_model::{
    SEMANTIC_WHITE_XYZ, identifies_open_or_white, legacy_emitter_is_visible, semantic_highlight_raw,
};
use super::geometry::stable_uuid;
use super::{
    ChannelFunction, ChannelFunctionBehavior, ChannelResolution, ColorSystem, EmitterBinding,
    FIXTURE_PROFILE_SCHEMA_VERSION, FixtureChannel, FixtureHead, FixtureMode, FixtureProfile,
    FixtureSplit, GeometryGraph, GeometryTemplate, HeadColorSystem, ModelUnits, PatchPolicy,
    ProfileError, ProfilePhysicalProperties,
};
use crate::{ChannelBehavior, FixtureDefinition};
use light_core::AttributeKey;
use std::collections::HashMap;

impl FixtureProfile {
    pub fn from_legacy_modes(definitions: &[FixtureDefinition]) -> Result<Self, ProfileError> {
        let first = definitions
            .first()
            .ok_or_else(|| ProfileError::Invalid("legacy mode list is empty".into()))?;
        if definitions.iter().any(|definition| {
            !definition
                .manufacturer
                .eq_ignore_ascii_case(&first.manufacturer)
                || !definition.model.eq_ignore_ascii_case(&first.model)
                || definition.device_type != first.device_type
                || definition.physical.width_millimetres != first.physical.width_millimetres
                || definition.physical.height_millimetres != first.physical.height_millimetres
                || definition.physical.depth_millimetres != first.physical.depth_millimetres
                || definition.physical.weight_kilograms != first.physical.weight_kilograms
        }) {
            return Err(ProfileError::Invalid(
                "legacy modes have conflicting fixture-level metadata".into(),
            ));
        }
        // Reuse one legacy definition identity so migration remains deterministic even when two
        // records share a manufacturer/model label but have conflicting fixture-level metadata.
        let profile_id = first.id;
        let modes = definitions
            .iter()
            .map(FixtureMode::from_legacy)
            .collect::<Result<Vec<_>, _>>()?;
        let profile = Self {
            schema_version: FIXTURE_PROFILE_SCHEMA_VERSION,
            id: profile_id,
            revision: 1,
            manufacturer: first.manufacturer.clone(),
            name: first.display_name().to_owned(),
            short_name: first.model.clone(),
            fixture_type: first.device_type.clone(),
            patch_policy: PatchPolicy::Dmx,
            notes: String::new(),
            photograph_asset: None,
            stage_icon_asset: first.icon_asset.clone(),
            model_asset: first.model_asset.clone(),
            model_units: ModelUnits::Auto,
            physical: ProfilePhysicalProperties {
                width_millimetres: first.physical.width_millimetres,
                height_millimetres: first.physical.height_millimetres,
                depth_millimetres: first.physical.depth_millimetres,
                weight_kilograms: first.physical.weight_kilograms,
                power_watts: first.physical.power_watts,
                ..Default::default()
            },
            modes,
            hazardous: first.hazardous,
            direct_control_protocols: first.direct_control_protocols.clone(),
            signal_loss_policy: first.signal_loss_policy,
            reserved_source: None,
        };
        profile.validate()?;
        Ok(profile)
    }
}

impl FixtureMode {
    /// Apply the semantic full-and-white look only while deriving a new schema-v2 mode from a
    /// legacy definition. Authored schema-v2 Highlight raw values are never normalized on load or
    /// save, and per-fixture overrides live outside the profile entirely.
    fn apply_derived_highlight_defaults(&mut self) -> Result<(), ProfileError> {
        let mut color_values = HashMap::new();
        for system in &self.color_systems {
            match &system.system {
                ColorSystem::Additive { .. } | ColorSystem::Subtractive { .. } => {
                    color_values.extend(self.resolve_color(system.head_id, SEMANTIC_WHITE_XYZ)?);
                }
                ColorSystem::DiscreteWheel { channel_id, slots } => {
                    let slot = slots
                        .iter()
                        .find(|slot| {
                            identifies_open_or_white(&slot.semantic_id)
                                || identifies_open_or_white(&slot.label)
                        })
                        .or_else(|| {
                            slots
                                .iter()
                                .filter_map(|slot| {
                                    slot.measured_xyz
                                        .map(|xyz| (slot, color_distance(SEMANTIC_WHITE_XYZ, xyz)))
                                })
                                .min_by(|left, right| left.1.total_cmp(&right.1))
                                .map(|(slot, _)| slot)
                        });
                    if let Some(slot) = slot {
                        color_values.insert(
                            *channel_id,
                            slot.dmx_from + (slot.dmx_to - slot.dmx_from) / 2,
                        );
                    }
                }
            }
        }
        for channel in &mut self.channels {
            if let Some(raw) = color_values.get(&channel.id) {
                channel.highlight_raw = *raw;
            }
        }
        Ok(())
    }

    fn from_legacy(definition: &FixtureDefinition) -> Result<Self, ProfileError> {
        let mode_id = stable_uuid(&format!("{}\0{}", definition.id.0, definition.mode));
        let heads = definition
            .heads
            .iter()
            .map(|head| FixtureHead {
                id: stable_uuid(&format!("{mode_id}\0head\0{}", head.index)),
                name: head.name.clone(),
                master_shared: head.shared,
            })
            .collect::<Vec<_>>();
        let mut channels = Vec::new();
        for (head_index, legacy_head) in definition.heads.iter().enumerate() {
            let has_virtual_dimmer = legacy_head.parameters.iter().any(|parameter| {
                parameter.virtual_dimmer
                    && parameter.attribute.is_intensity()
                    && parameter.components.is_empty()
            });
            for (parameter_index, parameter) in legacy_head.parameters.iter().enumerate() {
                if parameter.components.is_empty() {
                    continue;
                }
                let resolution = match parameter.components.len() {
                    1 => ChannelResolution::U8,
                    2 => ChannelResolution::U16,
                    3 => ChannelResolution::U24,
                    4 => ChannelResolution::U32,
                    _ => {
                        return Err(ProfileError::Invalid(
                            "legacy channel resolution is invalid".into(),
                        ));
                    }
                };
                let max = resolution.max_raw();
                let attribute = parameter.attribute.clone();
                let default_raw = (parameter.default.clamp(0.0, 1.0) * max as f32).round() as u32;
                let highlight_raw = semantic_highlight_raw(
                    &attribute,
                    resolution,
                    default_raw,
                    parameter.metadata.invert,
                    &parameter.capabilities,
                );
                channels.push(FixtureChannel {
                    id: stable_uuid(&format!(
                        "{mode_id}\0channel\0{head_index}\0{parameter_index}\0{}",
                        attribute.0
                    )),
                    head_id: heads[head_index].id,
                    split: 1,
                    attribute: attribute.clone(),
                    resolution,
                    secondary_slots: parameter
                        .components
                        .iter()
                        .skip(1)
                        .map(|component| component.offset + 1)
                        .collect(),
                    default_raw,
                    highlight_raw,
                    physical_min: Some(parameter.metadata.physical_min),
                    physical_max: Some(parameter.metadata.physical_max),
                    unit: parameter.metadata.unit.clone(),
                    invert: parameter.metadata.invert,
                    snap: false,
                    reacts_to_virtual_intensity: parameter.virtual_dimmer
                        || (has_virtual_dimmer && attribute.0.starts_with("color.")),
                    reacts_to_sequence_master: attribute.is_intensity(),
                    reacts_to_group_master: attribute.is_intensity(),
                    reacts_to_grand_master: attribute.is_intensity(),
                    behavior: ChannelBehavior::Controlled,
                    functions: vec![ChannelFunction {
                        id: stable_uuid(&format!(
                            "{mode_id}\0function\0{head_index}\0{parameter_index}"
                        )),
                        name: attribute.0.clone(),
                        dmx_from: 0,
                        dmx_to: max,
                        attribute,
                        priority: 0,
                        behavior: ChannelFunctionBehavior::Continuous {
                            physical_min: parameter.metadata.physical_min,
                            physical_max: parameter.metadata.physical_max,
                            unit: parameter.metadata.unit.clone(),
                        },
                    }],
                });
            }
        }
        let color_systems = definition
            .color_calibration
            .as_ref()
            .map(|calibration| {
                heads
                    .iter()
                    .filter_map(|head| {
                        let emitters = calibration
                            .emitters
                            .iter()
                            .filter_map(|emitter| {
                                let attribute = AttributeKey(format!(
                                    "color.emitter.{}",
                                    emitter.name.to_lowercase()
                                ));
                                channels
                                    .iter()
                                    .find(|channel| {
                                        channel.head_id == head.id && channel.attribute == attribute
                                    })
                                    .map(|channel| EmitterBinding {
                                        channel_id: channel.id,
                                        name: emitter.name.clone(),
                                        xyz: emitter.xyz,
                                        maximum_level: emitter.limit,
                                        response_curve: 1.0,
                                        visible: legacy_emitter_is_visible(&emitter.name),
                                    })
                            })
                            .collect::<Vec<_>>();
                        (!emitters.is_empty()).then_some(HeadColorSystem {
                            head_id: head.id,
                            correction_matrix: calibration.correction_matrix,
                            system: ColorSystem::Additive { emitters },
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let geometry_head_ids = heads.iter().map(|head| head.id).collect::<Vec<_>>();
        let mut mode = Self {
            id: mode_id,
            name: definition.mode.clone(),
            notes: String::new(),
            splits: vec![FixtureSplit {
                number: 1,
                footprint: definition.footprint,
            }],
            heads,
            channels,
            color_systems,
            control_actions: Vec::new(),
            geometry: GeometryGraph::template(GeometryTemplate::Fixed, &geometry_head_ids),
        };
        mode.apply_derived_highlight_defaults()?;
        Ok(mode)
    }
}
