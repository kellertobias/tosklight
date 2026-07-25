use super::*;

#[test]
fn capture_mode_is_exact_user_shared_and_runtime_revisioned() {
    let registry = ProgrammerRegistry::default();
    let user = UserId::new();
    let first = SessionId::new();
    let peer = SessionId::new();
    registry.start(first, user);
    registry.start(peer, user);

    assert_eq!(registry.capture_mode(first), Some(Default::default()));
    assert_eq!(registry.capture_mode_revision(user), 0);
    assert!(registry.arm_preload(peer, false));
    assert!(registry.set_modes(peer, None, Some(true), None, None));

    let expected = ProgrammerCaptureMode {
        blind: true,
        preview: true,
        preload_capture_programmer: false,
    };
    assert_eq!(registry.capture_mode(first), Some(expected));
    assert_eq!(
        registry.interaction_version(peer).unwrap().capture_mode,
        expected
    );
    assert_eq!(registry.capture_mode_revision(user), 0);
    assert_eq!(registry.advance_capture_mode_revision(user), 1);
    assert_eq!(registry.capture_mode_revision(user), 1);
}

#[test]
fn restored_capture_tuple_starts_with_a_fresh_public_revision() {
    let source = ProgrammerRegistry::default();
    let session = SessionId::new();
    let user = UserId::new();
    let mut state = source.start(session, user);
    state.blind = true;
    state.preview = true;
    state.preload_capture_programmer = false;

    let restored = ProgrammerRegistry::default();
    restored.restore(state);

    assert_eq!(
        restored.capture_mode(session),
        Some(ProgrammerCaptureMode {
            blind: true,
            preview: true,
            preload_capture_programmer: false,
        })
    );
    assert_eq!(restored.capture_mode_revision(user), 0);
}

#[test]
fn a_new_programmer_after_explicit_clear_retains_the_live_revision() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let user = UserId::new();
    registry.start(session, user);
    registry.advance_capture_mode_revision(user);
    assert!(registry.clear(session));

    let restarted = SessionId::new();
    registry.start(restarted, user);

    assert_eq!(registry.capture_mode_revision(user), 1);
    assert_eq!(registry.capture_mode(restarted), Some(Default::default()));
}

#[test]
fn clear_marks_normal_values_changed_without_resetting_public_revisions() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let user = UserId::new();
    registry.start(session, user);
    registry.set(
        session,
        FixtureId::new(),
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.5),
    );
    registry.advance_normal_values_revision(user);
    registry.advance_capture_mode_revision(user);
    let generation = registry.normal_values_generation(session).unwrap();

    assert!(registry.clear(session));
    assert_eq!(
        registry.normal_values_generation_for_user(user),
        generation + 1
    );
    assert_eq!(registry.normal_values_revision(user), 1);
    assert_eq!(registry.capture_mode_revision(user), 1);
}
