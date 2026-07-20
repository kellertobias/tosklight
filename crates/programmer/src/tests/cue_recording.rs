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

#[test]
fn current_capture_owns_only_normal_recordable_values_in_programmer_order() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    registry.start(session, UserId::new());
    let fixtures = [FixtureId::new(), FixtureId::new()];
    let timing = NormalProgrammerValueTiming {
        fade: true,
        fade_millis: Some(1_250),
        delay_millis: Some(300),
    };
    assert!(registry.apply_normal_values(
        session,
        &[
            fixture_set(fixtures[1], "pan", 0.2, Default::default()),
            fixture_set(fixtures[0], "intensity", 0.8, timing),
            NormalProgrammerValueMutation::SetGroup {
                group_id: "front".into(),
                attribute: AttributeKey("tilt".into()),
                value: AttributeValue::Spread(vec![0.1, 0.9]),
                timing,
            },
        ],
    ));
    assert!(
        registry
            .set_transient_action(
                session,
                "fixture-control".into(),
                [(
                    FixtureId::new(),
                    AttributeKey("lamp".into()),
                    AttributeValue::RawDmx(255),
                )],
            )
            .is_some()
    );

    let captured = registry
        .capture_cue_recording(session, CueRecordingSource::CurrentCapture)
        .unwrap();

    assert_eq!(captured.source, CueRecordingCapturedSource::Normal);
    assert_eq!(captured.fixture_values.len(), 2);
    assert_eq!(captured.group_values.len(), 1);
    assert!(
        captured.fixture_values[0].programmer_order < captured.fixture_values[1].programmer_order
    );
    assert_eq!(captured.fixture_values[1].fade_millis, Some(1_250));
    assert_eq!(captured.fixture_values[1].delay_millis, Some(300));
    assert_eq!(captured.group_values[0].fade_millis, Some(1_250));
}

#[test]
fn current_capture_uses_pending_preload_only_in_capture_mode() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    registry.start(session, UserId::new());
    let normal = FixtureId::new();
    let preload = FixtureId::new();
    assert!(registry.apply_normal_values(
        session,
        &[fixture_set(normal, "intensity", 0.4, Default::default())],
    ));
    assert!(registry.arm_preload(session, true));
    assert!(registry.apply_preload_values(
        session,
        &[PreloadProgrammerValueMutation::SetFixture {
            fixture_id: preload,
            attribute: AttributeKey::intensity(),
            value: AttributeValue::Normalized(0.7),
            timing: Default::default(),
        }],
    ));

    let captured = registry
        .capture_cue_recording(session, CueRecordingSource::CurrentCapture)
        .unwrap();
    assert_eq!(captured.source, CueRecordingCapturedSource::PendingPreload);
    assert_eq!(captured.fixture_values[0].fixture_id, preload);

    assert!(registry.arm_preload(session, false));
    let captured = registry
        .capture_cue_recording(session, CueRecordingSource::CurrentCapture)
        .unwrap();
    assert_eq!(captured.source, CueRecordingCapturedSource::Normal);
    assert_eq!(captured.fixture_values[0].fixture_id, normal);
}

#[test]
fn explicit_preload_capture_prefers_pending_then_reports_active_fallback() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    registry.start(session, UserId::new());
    let fixture = FixtureId::new();
    assert!(registry.arm_preload(session, true));
    assert!(registry.apply_preload_values(
        session,
        &[PreloadProgrammerValueMutation::SetFixture {
            fixture_id: fixture,
            attribute: AttributeKey::intensity(),
            value: AttributeValue::Normalized(0.7),
            timing: PreloadProgrammerValueTiming {
                fade: true,
                fade_millis: Some(500),
                delay_millis: Some(20),
            },
        }],
    ));

    let pending = registry
        .capture_cue_recording(session, CueRecordingSource::PreloadPendingOrActive)
        .unwrap();
    assert_eq!(pending.source, CueRecordingCapturedSource::PendingPreload);
    assert!(!pending.used_active_preload_fallback());

    assert!(registry.activate_preload(session));
    registry
        .states
        .write()
        .get_mut(&session)
        .unwrap()
        .preload_group_pending
        .insert("legacy-empty".into(), Default::default());
    let active = registry
        .capture_cue_recording(session, CueRecordingSource::PreloadPendingOrActive)
        .unwrap();
    assert_eq!(active.source, CueRecordingCapturedSource::ActivePreload);
    assert!(active.used_active_preload_fallback());
    assert_eq!(active.fixture_values[0].fade_millis, Some(500));
}

#[test]
fn empty_preload_capture_is_valid_and_missing_session_is_distinct() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    registry.start(session, UserId::new());

    let empty = registry
        .capture_cue_recording(session, CueRecordingSource::PreloadPendingOrActive)
        .unwrap();
    assert!(empty.is_empty());
    assert_eq!(empty.source, CueRecordingCapturedSource::PendingPreload);
    assert_eq!(
        registry.capture_cue_recording(SessionId::new(), CueRecordingSource::CurrentCapture,),
        Err(CueRecordingCaptureError::MissingSession)
    );
}
