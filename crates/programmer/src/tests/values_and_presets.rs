use super::*;

#[test]
fn transient_control_action_is_not_serialized_and_reveals_latest_latched_value() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let fixture = FixtureId::new();
    let attribute = AttributeKey("__fixture_control_channel.shared".into());
    registry.start(session, UserId::new());
    registry.set(
        session,
        fixture,
        attribute.clone(),
        AttributeValue::RawDmxExact(180),
    );
    let generation = registry
        .set_transient_action(
            session,
            "lamp-on".into(),
            [(fixture, attribute.clone(), AttributeValue::RawDmxExact(255))],
        )
        .unwrap();

    let active = registry.get(session).unwrap();
    assert_eq!(active.values[0].value, AttributeValue::RawDmxExact(180));
    assert_eq!(
        active.transient_values[0].values[0].value,
        AttributeValue::RawDmxExact(255)
    );
    let restored: ProgrammerState =
        serde_json::from_str(&serde_json::to_string(&active).unwrap()).unwrap();
    assert!(restored.transient_values.is_empty());

    registry.set_many(
        session,
        [(fixture, attribute.clone(), AttributeValue::RawDmxExact(120))],
    );
    let layered = registry.get(session).unwrap();
    assert!(
        layered.transient_values[0].values[0].programmer_order > layered.values[0].programmer_order
    );
    assert!(registry.release_transient_action(session, "lamp-on", Some(generation)));
    let released = registry.get(session).unwrap();
    assert!(released.transient_values.is_empty());
    assert_eq!(released.values[0].value, AttributeValue::RawDmxExact(120));
}

#[test]
fn stale_timed_release_does_not_clear_a_retriggered_action() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let fixture = FixtureId::new();
    let attribute = AttributeKey("__fixture_control_channel.shared".into());
    registry.start(session, UserId::new());
    let first = registry
        .set_transient_action(
            session,
            "lamp-on".into(),
            [(fixture, attribute.clone(), AttributeValue::RawDmxExact(200))],
        )
        .unwrap();
    let second = registry
        .set_transient_action(
            session,
            "lamp-on".into(),
            [(fixture, attribute, AttributeValue::RawDmxExact(255))],
        )
        .unwrap();

    assert!(!registry.release_transient_action(session, "lamp-on", Some(first)));
    assert_eq!(registry.get(session).unwrap().transient_values.len(), 1);
    assert!(registry.release_transient_action(session, "lamp-on", Some(second)));
}

#[test]
fn faded_batch_is_one_undo_step_and_respects_preload_capture() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let fixture = FixtureId::new();
    registry.start(session, UserId::new());
    registry.set(
        session,
        fixture,
        AttributeKey("pan".into()),
        AttributeValue::Normalized(0.9),
    );
    registry.set_many_faded_with_timing(
        session,
        [
            (
                fixture,
                AttributeKey("pan".into()),
                AttributeValue::Normalized(0.25),
            ),
            (
                fixture,
                AttributeKey("tilt".into()),
                AttributeValue::Normalized(0.75),
            ),
        ],
        Some(400),
        None,
    );
    let values = registry.get(session).unwrap().values;
    assert_eq!(values.len(), 2);
    assert!(values.iter().all(|value| value.fade));
    assert!(values.iter().all(|value| value.fade_millis == Some(400)));
    assert_eq!(values[0].changed_at, values[1].changed_at);

    assert!(registry.undo(session));
    let values = registry.get(session).unwrap().values;
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].attribute, AttributeKey("pan".into()));
    assert_eq!(values[0].value, AttributeValue::Normalized(0.9));

    assert!(registry.arm_preload(session, true));
    registry.set_many_faded_with_timing(
        session,
        [(
            fixture,
            AttributeKey("pan".into()),
            AttributeValue::Normalized(0.5),
        )],
        Some(400),
        None,
    );
    let programmer = registry.get(session).unwrap();
    assert_eq!(programmer.values[0].value, AttributeValue::Normalized(0.9));
    assert_eq!(programmer.preload_pending.len(), 1);
    assert!(programmer.preload_pending[0].fade);
}

#[test]
fn preset_store_modes_are_explicit() {
    let fixture = FixtureId::new();
    let other = FixtureId::new();
    let mut preset = Preset {
        name: "A".into(),
        family: PresetFamily::Intensity,
        number: 1,
        values: HashMap::from([(
            fixture,
            HashMap::from([(AttributeKey::intensity(), AttributeValue::Normalized(0.5))]),
        )]),
        group_values: HashMap::new(),
    };
    preset.store(
        Preset {
            name: String::new(),
            family: PresetFamily::Intensity,
            number: 1,
            values: HashMap::from([
                (
                    fixture,
                    HashMap::from([(AttributeKey("pan".into()), AttributeValue::Normalized(0.2))]),
                ),
                (other, HashMap::new()),
            ]),
            group_values: HashMap::new(),
        },
        PresetStoreMode::AddMissingFixtures,
    );
    assert_eq!(preset.values[&fixture].len(), 1);
    assert!(preset.values.contains_key(&other));
    preset.store(
        Preset {
            name: "B".into(),
            family: PresetFamily::Mixed,
            number: 1,
            values: HashMap::from([(
                fixture,
                HashMap::from([(AttributeKey("pan".into()), AttributeValue::Normalized(0.2))]),
            )]),
            group_values: HashMap::new(),
        },
        PresetStoreMode::Merge,
    );
    assert_eq!(preset.name, "B");
    assert_eq!(preset.family, PresetFamily::Mixed);
    assert_eq!(preset.values[&fixture].len(), 2);
}

#[test]
fn preset_addresses_use_pool_local_numbers() {
    let color = PresetAddress::new(PresetFamily::Color, 1).unwrap();
    let position = PresetAddress::new(PresetFamily::Position, 1).unwrap();

    assert_eq!(color.storage_key(), "2.1");
    assert_eq!(position.storage_key(), "3.1");
    assert_ne!(color, position);
    assert_eq!(PresetAddress::parse("2.1").unwrap(), color);
    assert_eq!(PresetAddress::parse("3.1").unwrap(), position);
    assert!(PresetAddress::parse("1").is_err());
    assert!(PresetAddress::new(PresetFamily::Mixed, 0).is_err());
}

#[test]
fn legacy_plain_preset_keys_reconcile_with_the_stored_family() {
    let mut legacy_color = Preset {
        name: "Red".into(),
        family: PresetFamily::Color,
        number: 0,
        ..Default::default()
    };

    let address = legacy_color.reconcile_address("1").unwrap();
    assert_eq!(address, PresetAddress::new(PresetFamily::Color, 1).unwrap());
    assert_eq!(legacy_color.number, 1);

    let mut mismatched = Preset {
        family: PresetFamily::Color,
        number: 1,
        ..Default::default()
    };
    assert!(mismatched.reconcile_address("3.1").is_err());
}

#[test]
fn preset_families_accept_only_their_attributes_while_mixed_accepts_any() {
    let intensity = AttributeKey::intensity();
    let color_wheel = AttributeKey("color.wheel.1".into());
    let position = AttributeKey("tilt".into());
    let beam = AttributeKey("gobo.1".into());
    let custom = AttributeKey("custom.channel".into());

    assert!(PresetFamily::Intensity.accepts(&intensity));
    assert!(PresetFamily::Intensity.accepts(&AttributeKey("head.dimmer".into())));
    assert!(PresetFamily::Color.accepts(&color_wheel));
    assert!(PresetFamily::Position.accepts(&position));
    assert!(PresetFamily::Beam.accepts(&beam));
    assert!(!PresetFamily::Color.accepts(&position));
    assert!(!PresetFamily::Beam.accepts(&custom));
    assert!(PresetFamily::Mixed.accepts(&custom));
}

#[test]
fn legacy_all_family_deserializes_as_mixed() {
    let preset: Preset = serde_json::from_value(serde_json::json!({
        "name": "Legacy",
        "family": "All",
        "values": {},
        "group_values": {}
    }))
    .unwrap();
    assert_eq!(preset.family, PresetFamily::Mixed);
    assert_eq!(serde_json::to_value(preset).unwrap()["family"], "Mixed");
}

#[test]
fn update_content_captures_only_normal_programmer_edits_without_consuming_them() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let fixture = FixtureId::new();
    let preload_fixture = FixtureId::new();
    registry.start(session, UserId::new());
    registry.select(session, [fixture]);
    registry.set_faded_with_timing(
        session,
        fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.75),
        Some(1_000),
        Some(250),
    );
    assert!(registry.set_group(
        session,
        "front".into(),
        AttributeKey("pan".into()),
        AttributeValue::Normalized(0.25),
    ));
    assert!(registry.arm_preload(session, true));
    registry.set(
        session,
        preload_fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(1.0),
    );

    let before = registry.get(session).unwrap();
    let update = before.update_content();
    let after = registry.get(session).unwrap();

    assert_eq!(update.selected_fixtures, vec![fixture]);
    assert_eq!(update.fixture_values.len(), 1);
    assert_eq!(update.fixture_values[0].fixture_id, fixture);
    assert_eq!(update.fixture_values[0].fade_millis, Some(1_000));
    assert_eq!(update.fixture_values[0].delay_millis, Some(250));
    assert_eq!(update.group_values.len(), 1);
    assert_eq!(update.group_values[0].group_id, "front");
    assert_eq!(after.values.len(), before.values.len());
    assert_eq!(after.group_values.len(), before.group_values.len());
    assert_eq!(after.preload_pending.len(), 1);
    assert_eq!(after.preload_pending[0].fixture_id, preload_fixture);
}

#[test]
fn ordered_group_merge_never_reorders_or_duplicates_existing_members() {
    let first = FixtureId::new();
    let second = FixtureId::new();
    let third = FixtureId::new();
    let fourth = FixtureId::new();

    assert_eq!(
        merge_ordered_group_membership(&[first, second], &[second, third, first, fourth, third]),
        vec![first, second, third, fourth]
    );
}

#[test]
fn transaction_snapshot_restores_programmer_and_desk_interaction_exactly() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let fixture = FixtureId::new();
    registry.start(session, UserId::new());
    registry.select(session, [fixture]);
    registry.set(
        session,
        fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.5),
    );
    registry.set_command_line(session, "GROUP 1 AT BOGUS".into());
    let programmer_before = serde_json::to_value(registry.get(session).unwrap()).unwrap();
    let selection_before = registry.selection(session).unwrap();
    let command_before = registry.command_line_state(session).unwrap();
    let checkpoint = registry.transaction_snapshot(session).unwrap();

    registry.clear_values(session);
    registry.select(session, [FixtureId::new()]);
    registry.set_command_target(session, "GROUP".into());
    registry.set_command_line(session, "GROUP 9".into());
    registry.restore_transaction_snapshot(checkpoint);

    assert_eq!(
        serde_json::to_value(registry.get(session).unwrap()).unwrap(),
        programmer_before
    );
    assert_eq!(registry.selection(session).unwrap(), selection_before);
    assert_eq!(
        registry.command_line_state(session).unwrap(),
        command_before
    );
}
