use super::*;

fn fixture_set(
    fixture_id: FixtureId,
    attribute: &str,
    value: f32,
    timing: NormalProgrammerValueTiming,
) -> NormalProgrammerValueMutation {
    NormalProgrammerValueMutation::SetFixture {
        fixture_id,
        attribute: AttributeKey(attribute.into()),
        value: AttributeValue::Normalized(value),
        timing,
    }
}

fn group_set(
    group_id: &str,
    attribute: &str,
    value: AttributeValue,
    timing: NormalProgrammerValueTiming,
) -> NormalProgrammerValueMutation {
    NormalProgrammerValueMutation::SetGroup {
        group_id: group_id.into(),
        attribute: AttributeKey(attribute.into()),
        value,
        timing,
    }
}

#[test]
fn normal_batch_uses_one_checkpoint_and_skips_exact_writes() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let user = UserId::new();
    let fixture_a = FixtureId::new();
    let fixture_b = FixtureId::new();
    let faded = NormalProgrammerValueTiming {
        fade: true,
        fade_millis: Some(1_000),
        delay_millis: Some(250),
    };
    registry.start(session, user);

    let initial = [
        fixture_set(fixture_a, "intensity", 0.25, Default::default()),
        fixture_set(fixture_b, "pan", 0.5, faded),
        group_set(
            "front",
            "tilt",
            AttributeValue::Spread(vec![0.1, 0.9]),
            faded,
        ),
    ];
    assert!(registry.apply_normal_values(session, &initial));

    let first = registry.get(session).unwrap();
    assert_eq!(registry.normal_values_generation(session), Some(1));
    assert_eq!(first.undo.len(), 1);
    assert_eq!(first.values.len(), 2);
    let first_a_order = first.values[0].programmer_order;
    let first_b_order = first.values[1].programmer_order;
    let first_group_order =
        first.group_values["front"][&AttributeKey("tilt".into())].programmer_order;
    assert!(first_a_order < first_b_order && first_b_order < first_group_order);

    assert!(!registry.apply_normal_values(session, &initial));
    let unchanged = registry.get(session).unwrap();
    assert_eq!(registry.normal_values_generation(session), Some(1));
    assert_eq!(unchanged.undo.len(), 1);
    assert_eq!(unchanged.values[0].programmer_order, first_a_order);

    let partially_changed = [
        fixture_set(fixture_a, "intensity", 0.25, Default::default()),
        fixture_set(
            fixture_b,
            "pan",
            0.5,
            NormalProgrammerValueTiming {
                fade: true,
                fade_millis: Some(2_000),
                delay_millis: Some(250),
            },
        ),
    ];
    assert!(registry.apply_normal_values(session, &partially_changed));
    let changed = registry.get(session).unwrap();
    assert_eq!(registry.normal_values_generation(session), Some(2));
    assert_eq!(changed.undo.len(), 2);
    assert_eq!(changed.values[0].programmer_order, first_a_order);
    assert!(changed.values[1].programmer_order > first_group_order);
    assert_eq!(changed.values[1].fade_millis, Some(2_000));

    assert!(registry.undo(session));
    let undone = registry.get(session).unwrap();
    assert_eq!(undone.values[0].programmer_order, first_a_order);
    assert_eq!(undone.values[1].programmer_order, first_b_order);
    assert_eq!(undone.values[1].fade_millis, Some(1_000));
}

#[test]
fn normal_actions_bypass_preload_and_clear_preserves_transient_state() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let user = UserId::new();
    let normal_fixture = FixtureId::new();
    let preload_fixture = FixtureId::new();
    let transient_fixture = FixtureId::new();
    registry.start(session, user);
    assert!(registry.arm_preload(session, true));
    registry.set(
        session,
        preload_fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.2),
    );
    assert!(registry.set_group(
        session,
        "preload".into(),
        AttributeKey("pan".into()),
        AttributeValue::Normalized(0.3),
    ));
    assert!(
        registry
            .set_transient_action(
                session,
                "lamp-on".into(),
                [(
                    transient_fixture,
                    AttributeKey("fixture-control".into()),
                    AttributeValue::RawDmxExact(255),
                )],
            )
            .is_some()
    );

    assert!(registry.apply_normal_values(
        session,
        &[
            fixture_set(normal_fixture, "intensity", 0.75, Default::default(),),
            group_set(
                "normal",
                "tilt",
                AttributeValue::Normalized(0.6),
                Default::default(),
            ),
        ],
    ));
    let populated = registry.get(session).unwrap();
    assert_eq!(populated.values.len(), 1);
    assert_eq!(populated.group_values.len(), 1);
    assert_eq!(populated.preload_pending.len(), 1);
    assert_eq!(populated.preload_group_pending.len(), 1);
    assert_eq!(populated.transient_values.len(), 1);

    assert!(registry.clear_normal_values(session));
    let cleared = registry.get(session).unwrap();
    assert!(cleared.values.is_empty());
    assert!(cleared.group_values.is_empty());
    assert_eq!(cleared.preload_pending.len(), 1);
    assert_eq!(cleared.preload_group_pending.len(), 1);
    assert_eq!(cleared.transient_values.len(), 1);
    assert_eq!(registry.normal_values_generation(session), Some(2));
    assert!(!registry.clear_normal_values(session));
    assert_eq!(registry.normal_values_generation(session), Some(2));
}
