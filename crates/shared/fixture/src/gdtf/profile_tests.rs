use super::*;
use crate::{ChannelFunction, FixtureHead};
use light_core::AttributeKey;

fn channel(head: Uuid, attribute: &str, resolution: ChannelResolution) -> FixtureChannel {
    FixtureChannel {
        id: Uuid::new_v4(),
        head_id: head,
        split: 1,
        fixture_attribute: AttributeKey(attribute.into()),
        attribute: AttributeKey(attribute.into()),
        canonical_transform: Default::default(),
        resolution,
        secondary_slots: Vec::new(),
        default_raw: 0,
        highlight_raw: resolution.max_raw(),
        physical_min: None,
        physical_max: None,
        unit: None,
        invert: false,
        snap: false,
        reacts_to_virtual_intensity: false,
        reacts_to_sequence_master: false,
        reacts_to_group_master: false,
        reacts_to_grand_master: false,
        behavior: Default::default(),
        functions: Vec::new(),
    }
}

fn head(name: &str) -> FixtureHead {
    FixtureHead {
        id: Uuid::new_v4(),
        name: name.into(),
        master_shared: false,
    }
}

/// A two-cell bar: a 16-bit master dimmer with a fine byte, a red channel per cell, a second red
/// on the second cell, and a gobo rotation with named ranges.
fn bar() -> FixtureProfile {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Acme".into();
    profile.name = "Cell Bar".into();
    profile.short_name = "Bar".into();
    profile.physical.width_millimetres = Some(1000.0);
    let mode = &mut profile.modes[0];
    mode.name = "8 bit – 2 cells".into();
    let master = mode.heads[0].id;
    let (first, second) = (head("Cell"), head("Cell"));
    let mut dimmer = channel(master, "intensity", ChannelResolution::U16);
    dimmer.secondary_slots = vec![2];
    dimmer.default_raw = 65_535;
    let mut rotation = channel(master, "gobo.1.rotation", ChannelResolution::U8);
    let mut stopped = ChannelFunction::continuous("Stop", rotation.attribute.clone(), 9);
    stopped.behavior = ChannelFunctionBehavior::Fixed {
        semantic_id: "stop".into(),
        label: "Stopped".into(),
        raw_value: 0,
    };
    let mut spin = ChannelFunction::continuous("Spin", rotation.attribute.clone(), 255);
    spin.dmx_from = 10;
    let mut shadowed = spin.clone();
    shadowed.name = "Shadowed".into();
    rotation.functions = vec![spin, stopped, shadowed];
    mode.channels = vec![
        dimmer,
        channel(first.id, "color.red", ChannelResolution::U8),
        channel(second.id, "color.red", ChannelResolution::U8),
        channel(second.id, "color.red", ChannelResolution::U8),
        rotation,
    ];
    mode.heads.extend([first, second]);
    mode.splits[0].footprint = 6;
    profile
}

#[test]
fn every_channel_keeps_its_exact_slots_head_and_standard_attribute() {
    let fixture = fixture_type(&bar()).expect("a valid profile describes");
    let mode = &fixture.modes[0];
    let described: Vec<_> = mode
        .channels
        .iter()
        .map(|channel| {
            (
                channel.attribute.as_str(),
                channel.offsets(),
                channel.geometry.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        described,
        vec![
            ("Dimmer", vec![1, 2], None),
            ("ColorAdd_R", vec![3], Some("Cell")),
            ("ColorAdd_R", vec![4], Some("Cell 2")),
            ("ColorAdd_R_2", vec![5], Some("Cell 2")),
            ("Gobo1PosRotate", vec![6], None),
        ]
    );
    assert_eq!(mode.footprint(), 6, "the footprint an MVR patches");
    assert_eq!(mode.channels[0].default, 65_535);
    assert_eq!(mode.channels[0].highlight, Some(65_535));
    assert_eq!(fixture.beams, vec!["Cell", "Cell 2"]);
    assert_eq!(fixture.body_size, Some([1.0, 0.3, 0.3]));
}

#[test]
fn mode_names_are_valid_gdtf_names_and_unique() {
    let mut profile = bar();
    let copy = profile.modes[0].clone();
    profile.modes.push(copy);
    assert_eq!(
        mode_names(&profile),
        vec!["8 bit - 2 cells", "8 bit - 2 cells 2"]
    );
    let xml = super::super::description_xml(&fixture_type(&profile).unwrap());
    assert!(xml.contains("<DMXMode Name=\"8 bit - 2 cells 2\""), "{xml}");
}

#[test]
fn named_ranges_start_where_their_functions_start_and_never_overlap() {
    let xml = super::super::description_xml(&fixture_type(&bar()).unwrap());
    assert!(
        xml.contains("<ChannelSet Name=\"Stopped\" DMXFrom=\"0/1\"/>"),
        "{xml}"
    );
    assert!(xml.contains("<ChannelSet Name=\"Spin\" DMXFrom=\"10/1\"/>"));
    assert!(!xml.contains("Shadowed"));
    assert!(xml.contains("Feature=\"Dimmer.Dimmer\""));
    assert!(xml.contains("Feature=\"Gobo.Gobo\""));
}

#[test]
fn a_fixture_with_only_the_shared_master_still_has_a_beam() {
    let mut profile = FixtureProfile::blank();
    profile.name = "Par".into();
    let master = profile.modes[0].heads[0].id;
    profile.modes[0].channels = vec![channel(master, "intensity", ChannelResolution::U8)];
    let fixture = fixture_type(&profile).unwrap();
    assert_eq!(fixture.beams, vec!["Beam"]);
    assert_eq!(
        fixture.manufacturer, "Generic",
        "GDTF requires a manufacturer"
    );
    assert_eq!(
        fixture.modes[0].channels[0].geometry, None,
        "the body controls the beam below it"
    );
}

#[test]
fn a_profile_whose_slots_cannot_be_derived_is_refused() {
    let mut profile = bar();
    profile.modes[0].splits[0].footprint = 3;
    assert!(fixture_type(&profile).is_err());
}
