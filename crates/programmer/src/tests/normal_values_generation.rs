use super::*;

#[test]
fn dirty_generation_tracks_only_normal_recordable_values() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let user = UserId::new();
    let fixture = FixtureId::new();
    registry.start(session, user);

    assert_eq!(registry.normal_values_generation(session), Some(0));
    assert_eq!(registry.normal_values_revision(user), 0);

    registry.select(session, [fixture]);
    registry.set_command_line(session, "GROUP 1".into());
    assert!(registry.set_priority(session, 7));
    assert!(registry.set_modes(session, Some(true), None, None, None));
    let transient = registry
        .set_transient_action(
            session,
            "lamp-on".into(),
            [(
                fixture,
                AttributeKey("fixture-control".into()),
                AttributeValue::RawDmxExact(255),
            )],
        )
        .unwrap();
    assert!(registry.release_transient_action(session, "lamp-on", Some(transient)));
    assert!(registry.set_preload_group(
        session,
        "1".into(),
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.25),
    ));
    assert!(registry.activate_preload(session));
    assert!(registry.release_preload(session));
    assert_eq!(registry.normal_values_generation(session), Some(0));

    registry.set(
        session,
        fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.5),
    );
    assert_eq!(registry.normal_values_generation(session), Some(1));
    assert!(registry.undo(session));
    assert_eq!(registry.normal_values_generation(session), Some(2));
    assert!(registry.redo(session));
    assert_eq!(registry.normal_values_generation(session), Some(3));
    assert!(!registry.redo(session));
    assert_eq!(registry.normal_values_generation(session), Some(3));

    assert!(registry.set_group(
        session,
        "2".into(),
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.75),
    ));
    assert_eq!(registry.normal_values_generation(session), Some(4));
    assert!(registry.release_group_attribute(session, "2", &AttributeKey::intensity()));
    assert_eq!(registry.normal_values_generation(session), Some(5));
    assert!(!registry.release_group_attribute(session, "2", &AttributeKey::intensity()));
    assert_eq!(registry.normal_values_generation(session), Some(5));

    assert!(registry.clear_values(session));
    assert_eq!(registry.normal_values_generation(session), Some(6));
    assert!(registry.clear_values(session));
    assert_eq!(registry.normal_values_generation(session), Some(6));
    assert_eq!(registry.normal_values_revision(user), 0);
}

#[test]
fn rejected_transactions_do_not_dirty_the_live_generation() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let fixture = FixtureId::new();
    registry.start(session, UserId::new());

    let rejected = registry.with_transaction(session, || {
        registry.set(
            session,
            fixture,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.5),
        );
        Err::<(), _>("rejected")
    });
    assert_eq!(rejected, Err("rejected"));
    assert_eq!(registry.normal_values_generation(session), Some(0));

    let rejected = registry.with_staged_transaction(session, |staged| {
        staged.set(
            session,
            fixture,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.75),
        );
        Err::<(), _>("rejected".to_owned())
    });
    assert_eq!(rejected, Err("rejected".to_owned()));
    assert_eq!(registry.normal_values_generation(session), Some(0));

    registry
        .with_staged_transaction(session, |staged| {
            staged.set(
                session,
                fixture,
                AttributeKey::intensity(),
                AttributeValue::Normalized(1.0),
            );
            Ok::<_, String>(())
        })
        .unwrap();
    assert_eq!(registry.normal_values_generation(session), Some(1));
}

#[test]
fn rejected_transactions_do_not_dirty_pending_preload_values_generation() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let fixture = FixtureId::new();
    registry.start(session, UserId::new());
    assert!(registry.arm_preload(session, true));
    let mutation = [PreloadProgrammerValueMutation::SetFixture {
        fixture_id: fixture,
        attribute: AttributeKey::intensity(),
        value: AttributeValue::Normalized(0.5),
        timing: Default::default(),
    }];

    let rejected = registry.with_transaction(session, || {
        assert!(registry.apply_preload_values(session, &mutation));
        Err::<(), _>("rejected")
    });
    assert_eq!(rejected, Err("rejected"));
    assert_eq!(registry.preload_values_generation(session), Some(0));
    assert!(registry.get(session).unwrap().preload_pending.is_empty());

    let rejected = registry.with_staged_transaction(session, |staged| {
        assert!(staged.apply_preload_values(session, &mutation));
        Err::<(), _>("rejected".to_owned())
    });
    assert_eq!(rejected, Err("rejected".to_owned()));
    assert_eq!(registry.preload_values_generation(session), Some(0));
    assert!(registry.get(session).unwrap().preload_pending.is_empty());

    registry
        .with_staged_transaction(session, |staged| {
            assert!(staged.apply_preload_values(session, &mutation));
            Ok::<_, String>(())
        })
        .unwrap();
    assert_eq!(registry.preload_values_generation(session), Some(1));
    assert_eq!(registry.get(session).unwrap().preload_pending.len(), 1);
}
