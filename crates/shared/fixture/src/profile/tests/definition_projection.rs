use super::*;

const LARGE_MODE_COUNT: usize = 2_000;
const SELECTED_MODE_INDEX: usize = 1_337;

#[test]
fn compact_projection_clones_only_the_selected_mode_from_a_large_validated_profile() {
    let profile = large_profile();
    let selected_mode_id = profile.modes[SELECTED_MODE_INDEX].id;
    profile.validate().unwrap();

    let definition = profile
        .compact_resolved_definition_from_validated_profile(selected_mode_id)
        .unwrap();

    assert_projection_identity_and_behavior(&profile, &definition, selected_mode_id);
    let snapshot = definition.profile_snapshot.as_deref().unwrap();
    assert_eq!(snapshot.modes.len(), 1);
    assert_eq!(snapshot.modes[0].id, selected_mode_id);
    assert_eq!(snapshot.modes[0].name, "Mode 1337");
    assert_eq!(snapshot.modes[0].channels.len(), 1);
    definition.validate().unwrap();
}

#[test]
fn public_projection_keeps_the_complete_profile_snapshot() {
    let profile = large_profile();
    let selected_mode_id = profile.modes[SELECTED_MODE_INDEX].id;
    let expected_mode_ids = profile.modes.iter().map(|mode| mode.id).collect::<Vec<_>>();

    let definition = profile.resolved_definition(selected_mode_id).unwrap();

    assert_projection_identity_and_behavior(&profile, &definition, selected_mode_id);
    let snapshot = definition.profile_snapshot.as_deref().unwrap();
    assert_complete_snapshot(&profile, snapshot, &expected_mode_ids);
    definition.validate().unwrap();
}

fn large_profile() -> FixtureProfile {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Projection Test".into();
    profile.name = "Large Fixture".into();
    profile.short_name = "Large".into();
    profile.fixture_type = "wash".into();
    profile.revision = 42;
    let mut template = profile.modes.remove(0);
    template.channels = vec![channel(
        template.heads[0].id,
        ChannelResolution::U8,
        Vec::new(),
    )];
    profile.modes = (0..LARGE_MODE_COUNT)
        .map(|index| mode_from(&template, index))
        .collect();
    configure_selected_mode(&mut profile.modes[SELECTED_MODE_INDEX]);
    profile
}

fn mode_from(template: &FixtureMode, index: usize) -> FixtureMode {
    let mut mode = template.clone();
    mode.id = Uuid::from_u128(index as u128 + 1);
    mode.name = format!("Mode {index}");
    mode
}

fn configure_selected_mode(mode: &mut FixtureMode) {
    mode.splits[0].footprint = 2;
    let channel = &mut mode.channels[0];
    channel.resolution = ChannelResolution::U16;
    channel.secondary_slots = vec![2];
    channel.default_raw = 32_768;
    channel.highlight_raw = u16::MAX.into();
    channel.functions = vec![ChannelFunction::continuous(
        "Dimmer",
        AttributeKey::intensity(),
        u16::MAX.into(),
    )];
}

fn assert_projection_identity_and_behavior(
    profile: &FixtureProfile,
    definition: &FixtureDefinition,
    selected_mode_id: Uuid,
) {
    assert_eq!(definition.id, profile.id);
    assert_eq!(definition.profile_id, Some(profile.id));
    assert_eq!(definition.revision, profile.revision);
    assert_eq!(definition.mode_id, Some(selected_mode_id));
    assert_eq!(definition.mode, "Mode 1337");
    assert_eq!(definition.footprint, 2);
    assert_eq!(definition.split_footprints(), BTreeMap::from([(1, 2)]));
    assert_parameter_behavior(&definition.heads[0].parameters[0]);
}

fn assert_parameter_behavior(parameter: &Parameter) {
    assert_eq!(
        parameter
            .components
            .iter()
            .map(|component| component.offset)
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
    assert!((parameter.default - 32_768.0 / 65_535.0).abs() < f32::EPSILON);
}

fn assert_complete_snapshot(
    profile: &FixtureProfile,
    snapshot: &FixtureProfile,
    expected_mode_ids: &[Uuid],
) {
    assert_eq!(snapshot.modes.len(), LARGE_MODE_COUNT);
    assert_eq!(
        serde_json::to_value(snapshot).unwrap(),
        serde_json::to_value(profile).unwrap()
    );
    let actual_mode_ids = snapshot
        .modes
        .iter()
        .map(|mode| mode.id)
        .collect::<Vec<_>>();
    assert_eq!(actual_mode_ids, expected_mode_ids);
}
