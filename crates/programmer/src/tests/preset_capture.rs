use super::*;

#[test]
fn narrow_capture_filters_every_family_and_strips_timing() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let user = UserId::new();
    let fixture = FixtureId::new();
    registry.start(session, user);
    for (attribute, value) in family_values() {
        registry.set_faded_with_timing(session, fixture, attribute, value, Some(1_000), Some(250));
    }
    assert!(registry.set_group(
        session,
        "front".into(),
        AttributeKey("color.wheel.1".into()),
        AttributeValue::Discrete("red".into()),
    ));

    for (family, expected) in [
        (PresetFamily::Intensity, vec!["intensity"]),
        (PresetFamily::Color, vec!["color.wheel.1"]),
        (PresetFamily::Position, vec!["pan"]),
        (PresetFamily::Beam, vec!["gobo.1"]),
        (
            PresetFamily::Mixed,
            vec![
                "color.wheel.1",
                "custom.channel",
                "gobo.1",
                "intensity",
                "pan",
            ],
        ),
    ] {
        let address = PresetAddress::new(family, 7).unwrap();
        let captured = registry
            .capture_normal_preset(session, address, "Captured".into())
            .unwrap();
        let mut attributes = captured.values[&fixture]
            .keys()
            .map(|attribute| attribute.0.as_str())
            .collect::<Vec<_>>();
        attributes.sort_unstable();
        assert_eq!(attributes, expected);
        assert_eq!(
            captured.group_values.contains_key("front"),
            family.accepts(&AttributeKey("color.wheel.1".into()))
        );
        assert_eq!(captured.name, "Captured");
        assert_eq!(captured.family, family);
        assert_eq!(captured.number, 7);
    }
}

#[test]
fn narrow_capture_ignores_preload_and_transient_values() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let user = UserId::new();
    let normal = FixtureId::new();
    let hidden = FixtureId::new();
    registry.start(session, user);
    registry.set(
        session,
        normal,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.4),
    );
    registry.set_transient_action(
        session,
        "highlight".into(),
        [(
            hidden,
            AttributeKey::intensity(),
            AttributeValue::Normalized(1.0),
        )],
    );
    assert!(registry.arm_preload(session, true));
    registry.set(
        session,
        hidden,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.8),
    );

    let preset = registry
        .capture_normal_preset(
            session,
            PresetAddress::new(PresetFamily::Mixed, 1).unwrap(),
            "Normal".into(),
        )
        .unwrap();
    assert_eq!(preset.values.len(), 1);
    assert!(preset.values.contains_key(&normal));
    assert!(!preset.values.contains_key(&hidden));
}

fn family_values() -> Vec<(AttributeKey, AttributeValue)> {
    vec![
        (AttributeKey::intensity(), AttributeValue::Normalized(0.5)),
        (
            AttributeKey("color.wheel.1".into()),
            AttributeValue::Discrete("blue".into()),
        ),
        (AttributeKey("pan".into()), AttributeValue::Normalized(0.2)),
        (
            AttributeKey("gobo.1".into()),
            AttributeValue::Discrete("dots".into()),
        ),
        (
            AttributeKey("custom.channel".into()),
            AttributeValue::RawDmxExact(17),
        ),
    ]
}
