use super::*;

#[test]
fn releasing_one_scoped_attribute_preserves_every_other_contribution() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let fixture = FixtureId::new();
    registry.start(session, UserId::new());
    registry.set(
        session,
        fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.5),
    );
    registry.set(
        session,
        fixture,
        AttributeKey("pan".into()),
        AttributeValue::Normalized(0.25),
    );
    registry.set_group(
        session,
        "1".into(),
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.75),
    );
    registry.set_group(
        session,
        "1".into(),
        AttributeKey("pan".into()),
        AttributeValue::Normalized(0.8),
    );

    assert!(registry.release_fixture_attribute(session, fixture, &AttributeKey::intensity(),));
    assert!(!registry.release_fixture_attribute(session, fixture, &AttributeKey::intensity(),));
    assert_eq!(registry.get(session).unwrap().values.len(), 1);
    assert!(registry.release_group_attribute(session, "1", &AttributeKey::intensity(),));
    let state = registry.get(session).unwrap();
    assert_eq!(state.group_values["1"].len(), 1);
    assert!(state.group_values["1"].contains_key(&AttributeKey("pan".into())));
}

#[test]
fn restoring_multiple_sessions_for_one_user_does_not_deadlock() {
    let source = ProgrammerRegistry::default();
    let user = UserId::new();
    let first = SessionId::new();
    let mut first_state = source.start(first, user);
    first_state.connected = false;
    let mut second_state = first_state.clone();
    second_state.session_id = SessionId::new();
    second_state.id = ProgrammerId::new();

    let restored = ProgrammerRegistry::default();
    restored.restore(first_state);
    restored.restore(second_state);
    assert_eq!(restored.active().len(), 1);
    assert_eq!(restored.active_for_sessions().len(), 2);
}
#[test]
fn legacy_group_programmer_values_migrate_with_a_timestamp() {
    let value: GroupProgrammerValue =
        serde_json::from_value(serde_json::json!({"kind":"normalized","value":0.5})).unwrap();
    assert_eq!(value.value.normalized(), Some(0.5));
}

#[test]
fn ordered_selection_macros_derived_groups_and_cycles_are_deterministic() {
    let fixtures = (0..6).map(|_| FixtureId::new()).collect::<Vec<_>>();
    assert_eq!(
        apply_selection_rule(&fixtures, &SelectionRule::Odd),
        vec![fixtures[0], fixtures[2], fixtures[4]]
    );
    assert_eq!(
        apply_selection_rule(&fixtures, &SelectionRule::EveryNth { n: 3, offset: 1 }),
        vec![fixtures[1], fixtures[4]]
    );
    let mut groups = HashMap::from([
        (
            "source".into(),
            GroupDefinition {
                id: "source".into(),
                fixtures: fixtures.clone(),
                ..Default::default()
            },
        ),
        (
            "odd".into(),
            GroupDefinition {
                id: "odd".into(),
                derived_from: Some(DerivedGroup {
                    source_group_id: "source".into(),
                    rule: SelectionRule::Odd,
                }),
                ..Default::default()
            },
        ),
    ]);
    assert_eq!(
        resolve_group("odd", &groups).unwrap(),
        vec![fixtures[0], fixtures[2], fixtures[4]]
    );
    groups
        .get_mut("source")
        .unwrap()
        .fixtures
        .push(FixtureId::new());
    assert_eq!(resolve_group("odd", &groups).unwrap().len(), 4);
    groups.insert(
        "cycle".into(),
        GroupDefinition {
            id: "cycle".into(),
            derived_from: Some(DerivedGroup {
                source_group_id: "cycle".into(),
                rule: SelectionRule::All,
            }),
            ..Default::default()
        },
    );
    assert!(
        resolve_group("cycle", &groups)
            .unwrap_err()
            .contains("cycle")
    );
}

#[test]
fn preload_clear_does_not_release_active_preload() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let fixture = FixtureId::new();
    registry.start(session, UserId::new());
    registry.set_modes(session, Some(true), None, None, None);
    registry.set(
        session,
        fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.7),
    );
    assert!(registry.activate_preload(session));
    registry.set(
        session,
        fixture,
        AttributeKey("pan".into()),
        AttributeValue::Normalized(0.2),
    );
    assert!(registry.clear_preload_pending(session));
    let state = registry.get(session).unwrap();
    assert_eq!(state.preload_active.len(), 1);
    assert!(state.preload_pending.is_empty());
    assert_eq!(state.values.len(), 1);
    assert_eq!(state.values[0].attribute, AttributeKey("pan".into()));
}
#[test]
fn preload_retains_multiple_group_scopes_with_edit_timestamps() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    registry.start(session, UserId::new());
    registry.set_modes(session, Some(true), None, None, None);
    registry.set_preload_group(
        session,
        "a".into(),
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.4),
    );
    registry.set_preload_group(
        session,
        "b".into(),
        AttributeKey("pan".into()),
        AttributeValue::Normalized(0.7),
    );
    let state = registry.get(session).unwrap();
    assert_eq!(state.preload_group_pending.len(), 2);
    assert_eq!(
        state.preload_group_pending["a"][&AttributeKey::intensity()]
            .value
            .normalized(),
        Some(0.4)
    );
}

#[test]
fn preload_go_restamps_every_programmer_value_and_release_is_idempotent() {
    let entered_at = chrono::DateTime::parse_from_rfc3339("2026-07-16T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let clock = Arc::new(ManualClock::new(entered_at));
    let shared: SharedClock = clock.clone();
    let registry = ProgrammerRegistry::with_clock(shared);
    let session = SessionId::new();
    let fixture = FixtureId::new();
    registry.start(session, UserId::new());
    assert!(registry.arm_preload(session, true));
    registry.set_faded_with_timing(
        session,
        fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.5),
        None,
        None,
    );
    assert!(registry.set_group_faded_with_timing(
        session,
        "back".into(),
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.7),
        Some(1_000),
        None,
    ));

    let committed_at = clock.advance_millis(2_500);
    assert!(registry.activate_preload_at(session, committed_at));
    let active = registry.get(session).unwrap();
    assert_eq!(active.preload_active[0].changed_at, committed_at);
    assert_eq!(active.preload_active[0].fade_millis, None);
    assert_eq!(
        active.preload_group_active["back"][&AttributeKey::intensity()].changed_at,
        committed_at
    );
    assert_eq!(
        active.preload_group_active["back"][&AttributeKey::intensity()].fade_millis,
        Some(1_000)
    );

    assert!(registry.release_preload(session));
    assert!(!registry.release_preload(session));
}

#[test]
fn disabled_programmer_domain_stays_live_and_playback_verbs_retain_order() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let fixture = FixtureId::new();
    registry.start(session, UserId::new());
    assert!(registry.arm_preload(session, false));
    registry.set(
        session,
        fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.4),
    );
    assert!(registry.set_group(
        session,
        "front".into(),
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.6),
    ));
    for action in [
        "toggle", "go", "go-minus", "off", "on", "temp-on", "temp-off",
    ] {
        assert!(registry.queue_preload_playback_action(
            session,
            1,
            PreloadPlaybackQueueAction::try_from(action).unwrap(),
            PreloadPlaybackQueueSurface::Physical,
        ));
    }
    let state = registry.get(session).unwrap();
    assert!(state.preload_pending.is_empty());
    assert!(state.preload_group_pending.is_empty());
    assert_eq!(state.values.len(), 1);
    assert!(state.group_values.contains_key("front"));
    assert_eq!(
        state
            .preload_playback_pending
            .iter()
            .map(|pending| pending.action.legacy_name())
            .collect::<Vec<_>>(),
        [
            "toggle", "go", "go-minus", "off", "on", "temp-on", "temp-off"
        ]
    );
}

#[test]
fn disconnect_keeps_programmer_until_explicit_clear() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    registry.start(session, UserId::new());
    registry.disconnect(session);
    assert!(!registry.get(session).unwrap().connected);
    assert!(registry.clear(session));
    assert!(registry.get(session).is_none());
}

#[test]
fn history_and_console_state_are_session_local() {
    let registry = ProgrammerRegistry::default();
    let first = SessionId::new();
    let second = SessionId::new();
    registry.start(first, UserId::new());
    registry.start(second, UserId::new());
    assert!(registry.set_command_line(first, "Fixture 1 At Full".into()));
    assert!(registry.set_modes(
        first,
        Some(true),
        None,
        Some(true),
        Some(Some("live".into()))
    ));
    assert!(registry.undo(first));
    assert!(!registry.get(first).unwrap().highlight);
    assert!(registry.redo(first));
    assert!(!registry.get(first).unwrap().highlight);
    assert!(registry.get(first).unwrap().blind);
    assert!(registry.get(second).unwrap().command_line.is_empty());
}

#[test]
fn multi_channel_action_is_one_atomic_undo_step() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let fixture = FixtureId::new();
    registry.start(session, UserId::new());
    registry.set_many(
        session,
        [
            (
                fixture,
                AttributeKey("__fixture_control_channel.one".into()),
                AttributeValue::RawDmxExact(255),
            ),
            (
                fixture,
                AttributeKey("__fixture_control_channel.two".into()),
                AttributeValue::RawDmxExact(128),
            ),
        ],
    );
    assert_eq!(registry.get(session).unwrap().values.len(), 2);
    registry.set_many_transient(
        session,
        [
            (
                fixture,
                AttributeKey("__fixture_control_channel.one".into()),
                AttributeValue::RawDmxExact(0),
            ),
            (
                fixture,
                AttributeKey("__fixture_control_channel.two".into()),
                AttributeValue::RawDmxExact(0),
            ),
        ],
    );

    assert!(registry.undo(session));
    assert!(registry.get(session).unwrap().values.is_empty());
}
