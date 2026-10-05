use super::*;

fn head_mode(attributes: &[&str]) -> FixtureMode {
    let mut profile = FixtureProfile::blank();
    let mut mode = profile.modes.remove(0);
    let head_id = mode.heads[0].id;
    mode.splits[0].footprint = attributes.len() as u16;
    mode.channels = attributes
        .iter()
        .map(|attribute| {
            let mut channel = channel(head_id, ChannelResolution::U8, vec![]);
            channel.fixture_attribute = AttributeKey((*attribute).into());
            channel.attribute = AttributeKey((*attribute).into());
            channel.functions[0].attribute = channel.attribute.clone();
            channel
        })
        .collect();
    mode
}

fn follows(mode: &FixtureMode) -> Vec<bool> {
    mode.channels
        .iter()
        .map(|channel| channel.reacts_to_virtual_intensity)
        .collect()
}

#[test]
fn a_light_emitting_head_without_a_dimmer_has_a_virtual_dimmer() {
    let rgb = head_mode(&["color.red", "color.green", "color.blue"]);
    assert!(rgb.head_has_virtual_dimmer(rgb.heads[0].id));

    let dimmed = head_mode(&["intensity", "color.red", "color.green", "color.blue"]);
    assert!(!dimmed.head_has_virtual_dimmer(dimmed.heads[0].id));

    // A head that emits nothing (Pan/Tilt only) gets no Intensity of its own.
    let mover = head_mode(&["pan", "tilt"]);
    assert!(!mover.head_has_virtual_dimmer(mover.heads[0].id));

    // Subtractive flags are not emitters; such a head has a virtual dimmer only by opting in.
    let mut cmy = head_mode(&["color.cyan", "color.magenta", "color.yellow"]);
    assert!(!cmy.head_has_virtual_dimmer(cmy.heads[0].id));
    cmy.channels[0].reacts_to_virtual_intensity = true;
    cmy.channels[0].virtual_intensity_inverted = true;
    assert!(cmy.head_has_virtual_dimmer(cmy.heads[0].id));
}

#[test]
fn emitters_of_a_head_without_a_dimmer_follow_its_virtual_dimmer_by_default() {
    let mut rgbw = head_mode(&[
        "color.red",
        "color.green",
        "color.blue",
        "color.white",
        "pan",
    ]);
    rgbw.default_virtual_dimmer_reactions();
    assert_eq!(follows(&rgbw), [true, true, true, true, false]);

    // A physical dimmer already dims the emitters; following it as well would dim them twice.
    let mut dimmed = head_mode(&["intensity", "color.red", "color.green"]);
    dimmed.default_virtual_dimmer_reactions();
    assert_eq!(follows(&dimmed), [false, false, false]);

    let mut cmy = head_mode(&["color.cyan", "color.magenta", "color.yellow"]);
    cmy.default_virtual_dimmer_reactions();
    assert_eq!(follows(&cmy), [false, false, false]);
}

#[test]
fn the_resolved_definition_exposes_the_virtual_dimmer_as_intensity() {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Virtual Dimmer Test".into();
    profile.name = "RGB".into();
    profile.short_name = "RGB".into();
    let mut mode = head_mode(&["color.red", "color.green", "color.blue"]);
    mode.id = profile.modes[0].id;
    mode.heads = profile.modes[0].heads.clone();
    for channel in &mut mode.channels {
        channel.head_id = mode.heads[0].id;
    }
    // Even with every emitter set to Ignore, the head keeps its programmable Intensity.
    profile.modes[0] = mode;
    let definition = profile.resolved_definition(profile.modes[0].id).unwrap();
    let intensity = &definition.heads[0].parameters[0];
    assert!(intensity.attribute.is_intensity());
    assert!(intensity.virtual_dimmer);
    assert!(intensity.components.is_empty());
}
