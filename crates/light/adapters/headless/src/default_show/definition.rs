use light_core::{AttributeKey, FixtureId};
use light_fixture::{
    CanonicalTransform, ChannelBehavior, ChannelResolution, FixtureChannel, FixtureDefinition,
    FixtureHead, FixtureProfile,
};
#[cfg(test)]
use light_fixture::{
    FixtureLocation, FixtureVector, MultiPatchInstance, PatchedFixture, PatchedHead,
};
#[cfg(test)]
use std::collections::BTreeMap;

/// One DMX channel of a built-in fixture, at the next slot in its block.
fn channel(
    head_id: uuid::Uuid,
    attribute: &str,
    default_raw: u32,
    virtual_dimmer: bool,
) -> FixtureChannel {
    let attribute = AttributeKey(attribute.into());
    FixtureChannel {
        id: uuid::Uuid::new_v4(),
        head_id,
        split: 1,
        fixture_attribute: attribute.clone(),
        attribute,
        canonical_transform: CanonicalTransform::Identity,
        resolution: ChannelResolution::U8,
        secondary_slots: Vec::new(),
        default_raw,
        highlight_raw: u32::from(u8::MAX),
        physical_min: None,
        physical_max: None,
        unit: None,
        invert: false,
        snap: false,
        reacts_to_virtual_intensity: virtual_dimmer,
        reacts_to_sequence_master: true,
        reacts_to_group_master: true,
        reacts_to_grand_master: true,
        behavior: ChannelBehavior::Controlled,
        functions: Vec::new(),
    }
}

/// A built-in fixture's profile, from which its definition is derived like any other.
fn profile(name: &str, device_type: &str) -> FixtureProfile {
    let mut profile = FixtureProfile::blank();
    profile.id = FixtureId::new();
    profile.revision = 1;
    profile.manufacturer = "ToskLight Built-in".into();
    profile.name = name.into();
    profile.short_name = name.into();
    profile.fixture_type = device_type.into();
    profile
}

#[cfg(test)]
fn mode_name(attributes: &[&str]) -> String {
    attributes
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
        .collect()
}

#[cfg(test)]
pub(super) fn definition(name: &str, device_type: &str, attributes: &[&str]) -> FixtureDefinition {
    let mut profile = profile(name, device_type);
    let mode_id = {
        let mode = &mut profile.modes[0];
        mode.name = mode_name(attributes);
        mode.splits[0].footprint = attributes.len() as u16;
        mode.heads[0].name = "Main".into();
        mode.heads[0].master_shared = true;
        let head_id = mode.heads[0].id;
        mode.channels = attributes
            .iter()
            .map(|attribute| {
                // Pan and tilt park centred; everything else parks off.
                let default_raw = if matches!(*attribute, "pan" | "tilt") {
                    128
                } else {
                    0
                };
                channel(head_id, attribute, default_raw, false)
            })
            .collect();
        mode.id
    };
    profile
        .resolved_definition(mode_id)
        .expect("a built-in fixture profile resolves")
}

pub(super) fn sunstrip_definition() -> FixtureDefinition {
    let mut profile = profile("RGB LED Sunstrip 10", "strip light");
    let mode_id = {
        let mode = &mut profile.modes[0];
        mode.name = "10 × RGB".into();
        mode.splits[0].footprint = 30;
        // Ten cells, each its own head so an operator can select one. None of them carries a
        // dimmer channel: the intensity they answer to is derived from their colour, which is what
        // marking those channels as reacting to virtual intensity declares.
        mode.heads = (0..10)
            .map(|index| FixtureHead {
                id: uuid::Uuid::new_v4(),
                name: format!("Cell {}", index + 1),
                master_shared: false,
            })
            .collect();
        mode.channels = mode
            .heads
            .iter()
            .flat_map(|head| {
                ["color.red", "color.green", "color.blue"]
                    .into_iter()
                    .map(|attribute| channel(head.id, attribute, 0, true))
                    .collect::<Vec<_>>()
            })
            .collect();
        mode.id
    };
    profile
        .resolved_definition(mode_id)
        .expect("the built-in sunstrip profile resolves")
}

#[cfg(test)]
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
        note: None,
        position_master: None,
        direct_control: None,
        internal_bindings: Default::default(),
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
                profile_head_id: None,
                head_index: head.index,
                fixture_id: FixtureId::new(),
            })
            .collect(),
        multipatch: Vec::new(),
        group_masters_enabled: true,
        grand_master_enabled: true,
        invert_pan: false,
        invert_tilt: false,
        bracket_angle: 0.0,
        shaper_angle: None,
        installed_appearance: Default::default(),
        move_in_black_enabled: true,
        move_in_black_delay_millis: 0,
        highlight_overrides: BTreeMap::new(),
        freeze: Default::default(),
    }
}

#[cfg(test)]
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
        invert_pan: false,
        invert_tilt: false,
        bracket_angle: 0.0,
        shaper_angle: None,
        installed_appearance: Default::default(),
    }
}
