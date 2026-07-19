use super::color_model::{SEMANTIC_WHITE_XYZ, identity_color_correction};
use super::*;
use crate::{
    ByteOrder, Capability, ChannelComponent, ColorCalibration, EmitterCalibration,
    FixtureDefinition, FixturePhysicalProperties, LogicalHead, Parameter, ParameterMetadata,
    SignalLossPolicy,
};
use light_core::{AttributeKey, AttributeValue, FixtureId, Xyz};
use std::collections::{BTreeMap, HashMap};
use uuid::Uuid;

fn channel(
    head_id: Uuid,
    resolution: ChannelResolution,
    secondary_slots: Vec<u16>,
) -> FixtureChannel {
    let max = resolution.max_raw();
    FixtureChannel {
        id: Uuid::new_v4(),
        head_id,
        split: 1,
        attribute: AttributeKey("intensity".into()),
        resolution,
        secondary_slots,
        default_raw: 0,
        highlight_raw: max,
        physical_min: Some(0.0),
        physical_max: Some(100.0),
        unit: Some("percent".into()),
        invert: false,
        snap: false,
        reacts_to_virtual_intensity: false,
        reacts_to_sequence_master: true,
        reacts_to_group_master: true,
        reacts_to_grand_master: true,
        behavior: ChannelBehavior::Controlled,
        functions: vec![ChannelFunction::continuous(
            "Dimmer",
            AttributeKey("intensity".into()),
            max,
        )],
    }
}

fn additive_color_mode() -> FixtureMode {
    let mut profile = FixtureProfile::blank();
    let mode = &mut profile.modes[0];
    let head_id = mode.heads[0].id;
    let mut emitter = channel(head_id, ChannelResolution::U8, vec![]);
    emitter.attribute = AttributeKey("color.red".into());
    let channel_id = emitter.id;
    mode.channels = vec![emitter];
    mode.color_systems = vec![HeadColorSystem {
        head_id,
        correction_matrix: identity_color_correction(),
        system: ColorSystem::Additive {
            emitters: vec![EmitterBinding {
                channel_id,
                name: "Red".into(),
                xyz: Xyz {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                },
                maximum_level: 1.0,
                response_curve: 1.0,
                visible: true,
            }],
        },
    }];
    profile.modes.remove(0)
}

fn additive_emitter(mode: &mut FixtureMode) -> &mut EmitterBinding {
    let ColorSystem::Additive { emitters } = &mut mode.color_systems[0].system else {
        unreachable!("test mode is additive")
    };
    &mut emitters[0]
}

fn discrete_color_mode() -> FixtureMode {
    let mut profile = FixtureProfile::blank();
    let mode = &mut profile.modes[0];
    let head_id = mode.heads[0].id;
    let mut wheel = channel(head_id, ChannelResolution::U8, vec![]);
    wheel.attribute = AttributeKey("color.wheel.1".into());
    let channel_id = wheel.id;
    mode.channels = vec![wheel];
    mode.color_systems = vec![HeadColorSystem {
        head_id,
        correction_matrix: identity_color_correction(),
        system: ColorSystem::DiscreteWheel {
            channel_id,
            slots: vec![
                ColorWheelSlot {
                    semantic_id: "red".into(),
                    label: "Red".into(),
                    dmx_from: 0,
                    dmx_to: 40,
                    measured_xyz: Some(Xyz {
                        x: 1.0,
                        y: 0.0,
                        z: 0.0,
                    }),
                },
                ColorWheelSlot {
                    semantic_id: "blue".into(),
                    label: "Blue".into(),
                    dmx_from: 100,
                    dmx_to: 140,
                    measured_xyz: Some(Xyz {
                        x: 0.0,
                        y: 0.0,
                        z: 1.0,
                    }),
                },
            ],
        },
    }];
    profile.modes.remove(0)
}

fn wheel_slots(mode: &mut FixtureMode) -> &mut Vec<ColorWheelSlot> {
    let ColorSystem::DiscreteWheel { slots, .. } = &mut mode.color_systems[0].system else {
        unreachable!("test mode is a discrete wheel")
    };
    slots
}

mod color_geometry;
mod definition_projection;
mod encoding_plan;
mod inversion;
mod resolution;
mod validation;
