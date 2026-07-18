use light_core::{AttributeKey, FixtureId};
use light_fixture::{
    ByteOrder, ChannelComponent, FixtureDefinition, FixtureLocation, FixturePhysicalProperties,
    FixtureVector, LogicalHead, MultiPatchInstance, Parameter, ParameterMetadata, PatchedFixture,
    PatchedHead, SignalLossPolicy,
};
use std::collections::BTreeMap;

fn parameter(attribute: &str, offset: u16, default: f32) -> Parameter {
    Parameter {
        attribute: AttributeKey(attribute.into()),
        components: vec![ChannelComponent {
            offset,
            byte_order: ByteOrder::MsbFirst,
        }],
        default,
        virtual_dimmer: false,
        metadata: ParameterMetadata::default(),
        capabilities: Vec::new(),
    }
}

pub(super) fn definition(name: &str, device_type: &str, attributes: &[&str]) -> FixtureDefinition {
    FixtureDefinition {
        schema_version: 1,
        id: FixtureId::new(),
        revision: 1,
        manufacturer: "ToskLight Built-in".into(),
        device_type: device_type.into(),
        name: name.into(),
        model: name.into(),
        mode: attributes
            .iter()
            .map(|value| match *value {
                "intensity" => "D",
                "pan" => "P",
                "tilt" => "T",
                "color.red" => "R",
                "color.green" => "G",
                "color.blue" => "B",
                "color.white" => "W",
                _ => "?",
            })
            .collect(),
        footprint: attributes.len() as u16,
        heads: vec![LogicalHead {
            index: 0,
            name: "Main".into(),
            shared: true,
            parameters: attributes
                .iter()
                .enumerate()
                .map(|(offset, attribute)| {
                    parameter(
                        attribute,
                        offset as u16,
                        if matches!(*attribute, "pan" | "tilt") {
                            0.5
                        } else {
                            0.0
                        },
                    )
                })
                .collect(),
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

pub(super) fn sunstrip_definition() -> FixtureDefinition {
    let mut fixture = definition(
        "RGB LED Sunstrip 10",
        "strip light",
        &["color.red", "color.green", "color.blue"],
    );
    fixture.mode = "10 × RGB".into();
    fixture.footprint = 30;
    fixture.heads = (0..10)
        .map(|index| LogicalHead {
            index,
            name: format!("Cell {}", index + 1),
            shared: false,
            parameters: std::iter::once(Parameter {
                attribute: AttributeKey::intensity(),
                components: Vec::new(),
                default: 0.0,
                virtual_dimmer: true,
                metadata: ParameterMetadata::default(),
                capabilities: Vec::new(),
            })
            .chain(
                ["color.red", "color.green", "color.blue"]
                    .iter()
                    .enumerate()
                    .map(|(component, attribute)| {
                        let mut parameter = parameter(attribute, index * 3 + component as u16, 0.0);
                        parameter.virtual_dimmer = true;
                        parameter
                    }),
            )
            .collect(),
        })
        .collect();
    fixture
}

pub(super) fn patched(
    name: String,
    fixture_number: u32,
    definition: &FixtureDefinition,
    location: FixtureLocation,
    rotation_y: f32,
) -> PatchedFixture {
    let (universe, address) =
        super::default_patch(&name).expect("built-in fixture has a default patch");
    PatchedFixture {
        fixture_id: FixtureId::new(),
        fixture_number: Some(fixture_number),
        virtual_fixture_number: None,
        name,
        definition: definition.clone(),
        universe: Some(universe),
        address: Some(address),
        split_patches: Vec::new(),
        layer_id: "default".into(),
        direct_control: None,
        location,
        rotation: FixtureVector {
            x: 0.0,
            y: rotation_y,
            z: 0.0,
        },
        logical_heads: definition
            .heads
            .iter()
            .filter(|head| !head.shared)
            .map(|head| PatchedHead {
                head_index: head.index,
                fixture_id: FixtureId::new(),
            })
            .collect(),
        multipatch: Vec::new(),
        move_in_black_enabled: true,
        move_in_black_delay_millis: 0,
        highlight_overrides: BTreeMap::new(),
    }
}

pub(super) fn multipatch(
    name: String,
    location: FixtureLocation,
    rotation_y: f32,
) -> MultiPatchInstance {
    MultiPatchInstance {
        id: uuid::Uuid::new_v4(),
        name,
        universe: None,
        address: None,
        split_patches: Vec::new(),
        location,
        rotation: FixtureVector {
            x: 0.0,
            y: rotation_y,
            z: 0.0,
        },
    }
}
