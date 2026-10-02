use super::*;

#[test]
fn prepared_snapshot_identity_survives_both_install_policies() {
    for preserve_playback in [true, false] {
        let engine = Engine::new(ProgrammerRegistry::default());
        let before = engine.snapshot();
        let prepared = engine.prepare_snapshot((*before).clone()).unwrap();
        let identity = prepared.snapshot_arc();
        assert!(Arc::ptr_eq(&before, &engine.snapshot()));
        assert!(!Arc::ptr_eq(&before, &identity));
        assert!(Arc::ptr_eq(&identity, &prepared.snapshot_arc()));

        if preserve_playback {
            engine.install_prepared_snapshot(prepared);
        } else {
            engine.install_prepared_snapshot_releasing_playback(prepared);
        }

        assert!(Arc::ptr_eq(&identity, &engine.snapshot()));
        let frame = engine.prepare_output_frame(Default::default());
        assert!(Arc::ptr_eq(&identity, &frame.snapshot()));
        assert!(Arc::ptr_eq(&before.dynamics, &identity.dynamics));
        assert_eq!(before.revision, identity.revision);
    }
}

#[test]
fn prepared_snapshot_identity_is_private_until_its_own_installation() {
    let engine = Engine::new(ProgrammerRegistry::default());
    let live = engine.snapshot();
    let first = engine.prepare_snapshot((*live).clone()).unwrap();
    let second = engine.prepare_snapshot((*live).clone()).unwrap();
    let first_identity = first.snapshot_arc();
    let second_identity = second.snapshot_arc();
    assert!(!Arc::ptr_eq(&first_identity, &second_identity));
    assert!(Arc::ptr_eq(&live, &engine.snapshot()));

    engine.install_prepared_snapshot(second);
    assert!(Arc::ptr_eq(&second_identity, &engine.snapshot()));
    assert!(Arc::ptr_eq(&first_identity, &first.snapshot_arc()));
    engine.install_prepared_snapshot(first);
    assert!(Arc::ptr_eq(&first_identity, &engine.snapshot()));
}

#[test]
fn group_master_changes_output_generation_without_changing_snapshot_identity() {
    let engine = Engine::new(ProgrammerRegistry::default());
    let prepared = engine
        .prepare_snapshot(EngineSnapshot {
            playbacks: vec![test_group_playback_with_master(1, "1", 1.0)].into(),
            groups: vec![GroupDefinition {
                id: "1".into(),
                ..Default::default()
            }]
            .into(),
            ..Default::default()
        })
        .unwrap();
    let identity = prepared.snapshot_arc();
    engine.install_prepared_snapshot(prepared);
    let before = engine.prepare_output_frame(Default::default());

    assert!(engine.set_group_master("1", 0.5).unwrap());
    let after = engine.prepare_output_frame(Default::default());
    assert_ne!(before.generation(), after.generation());
    assert!(Arc::ptr_eq(&identity, &engine.snapshot()));
    assert!(Arc::ptr_eq(&identity, &before.snapshot()));
    assert!(Arc::ptr_eq(&identity, &after.snapshot()));
    assert_eq!(engine.group_master("1"), Some(0.5));
}

#[test]
fn queued_playback_preview_base_is_captured_with_output_and_only_when_needed() {
    let programmers = ProgrammerRegistry::default();
    let session = SessionId::new();
    programmers.start(session);
    let (patched, fixture) = fixture();
    let cue = test_cue_list(
        "Queued capture",
        vec![CueChange::set(
            fixture,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.6),
        )],
    );
    let engine = Engine::new(programmers.clone());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![patched].into(),
            cue_lists: vec![cue.clone()].into(),
            playbacks: vec![test_playback(1, cue.id)].into(),
            ..Default::default()
        })
        .unwrap();
    execute_pool(&engine, 1, PoolPlaybackAction::Go);
    assert!(
        engine
            .prepare_output_frame(Default::default())
            .preload_playback_source()
            .is_none()
    );
    programmers.queue_preload_playback_action(
        session,
        1,
        None,
        light_programmer::PreloadPlaybackQueueAction::Off,
        light_programmer::PreloadPlaybackQueueSurface::Physical,
    );
    let frame = engine.prepare_output_frame(Default::default());
    let preview = frame.preload_playback_source().unwrap();
    assert_eq!(frame.programmer().preload_playback_actions.len(), 1);
    assert!(preview.runtime()[0].enabled);
    execute_pool(&engine, 1, PoolPlaybackAction::Off);
    assert!(
        preview.runtime()[0].enabled,
        "the queued base cannot follow a later Live action"
    );
    assert!(!engine.playback_runtime()[0].enabled);
}

#[test]
fn reset_while_acquiring_sources_invalidates_the_entire_prepared_frame() {
    #[derive(Debug)]
    struct CaptureClock {
        value: chrono::DateTime<Utc>,
        armed: std::sync::atomic::AtomicBool,
        entered: std::sync::Barrier,
        resume: std::sync::Barrier,
    }
    impl ApplicationClock for CaptureClock {
        fn now(&self) -> chrono::DateTime<Utc> {
            if self.armed.swap(false, std::sync::atomic::Ordering::SeqCst) {
                self.entered.wait();
                self.resume.wait();
            }
            self.value
        }
    }
    let clock = Arc::new(CaptureClock {
        value: Utc::now(),
        armed: false.into(),
        entered: std::sync::Barrier::new(2),
        resume: std::sync::Barrier::new(2),
    });
    let programmers = ProgrammerRegistry::with_clock(clock.clone());
    programmers.start(SessionId::new());
    let engine = Arc::new(Engine::new(programmers));
    clock.armed.store(true, std::sync::atomic::Ordering::SeqCst);
    let capturing = Arc::clone(&engine);
    let capture = std::thread::spawn(move || capturing.prepare_output_frame(Default::default()));
    clock.entered.wait();
    // The clock call follows Programmer capture but precedes Playback/source acquisition.
    engine.clear_programmer_transitions();
    clock.resume.wait();
    let frame = capture.join().unwrap();
    assert!(matches!(
        engine.render_prepared(&frame, &[]),
        Err(EngineError::StalePreparedFrame)
    ));
    assert!(engine.render(Default::default()).is_ok());
}

#[test]
fn prepared_output_keeps_sources_generation_clock_and_overlays_after_live_edits() {
    let clock = Arc::new(ManualClock::new(Utc::now()));
    let programmers = ProgrammerRegistry::with_clock(clock.clone());
    let session = SessionId::new();
    programmers.start(session);

    let (programmer_fixture, programmer_id) = fixture();
    let programmer_root = programmer_fixture.fixture_id;
    let (mut cue_fixture, cue_id) = fixture();
    cue_fixture.fixture_number = Some(2);
    cue_fixture.address = Some(10);
    let (mut point, point_id) =
        schema_v2_fixture(&[("point.position.x", false, false, false, false, false)]);
    point.universe = None;
    point.address = None;
    point.location.x = 1_000;
    let cue = test_cue_list(
        "Prepared source",
        vec![CueChange::set(
            cue_id,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.8),
        )],
    );
    let engine = Engine::new(programmers.clone());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![programmer_fixture, cue_fixture, point].into(),
            cue_lists: vec![cue.clone()].into(),
            playbacks: vec![test_playback(1, cue.id)].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    programmers.set(
        session,
        programmer_id,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.4),
    );
    engine.set_tracked_overrides([crate::TrackedOverride::new(
        point_id,
        AttributeKey("point.position.x".into()),
        AttributeValue::Normalized(0.75),
    )]);
    execute_pool(&engine, 1, PoolPlaybackAction::Go);

    let captured = engine.prepare_output_frame(RenderOptions {
        grand_master: 0.5,
        ..Default::default()
    });
    let initial = engine.observe_prepared_frame(&captured, &[]);
    assert_eq!(captured.snapshot().revision, 1);
    assert_eq!(
        initial
            .values()
            .value(programmer_id, &AttributeKey::intensity()),
        Some(&AttributeValue::Normalized(0.4))
    );
    assert_eq!(
        initial.values().value(cue_id, &AttributeKey::intensity()),
        Some(&AttributeValue::Normalized(0.8))
    );
    assert_eq!(initial.points()[0].offset_metres, [50.0, 0.0, 0.0]);

    programmers.set(
        session,
        programmer_id,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.1),
    );
    execute_pool(&engine, 1, PoolPlaybackAction::Off);
    engine.set_tracked_overrides([crate::TrackedOverride::new(
        point_id,
        AttributeKey("point.position.x".into()),
        AttributeValue::Normalized(0.25),
    )]);
    engine.set_highlighted_fixtures([programmer_root]);
    let mut replacement = (*engine.snapshot()).clone();
    replacement.revision = 2;
    let fixtures = Arc::make_mut(&mut replacement.fixtures);
    fixtures[0].address = Some(20);
    fixtures[2].location.x = 2_000;
    engine.replace_snapshot(replacement).unwrap();
    clock.advance_millis(1_000);

    let observed_again = engine.observe_prepared_frame(&captured, &[]);
    assert_eq!(
        observed_again
            .values()
            .value(programmer_id, &AttributeKey::intensity()),
        initial
            .values()
            .value(programmer_id, &AttributeKey::intensity())
    );
    assert_eq!(
        observed_again
            .values()
            .value(cue_id, &AttributeKey::intensity()),
        initial.values().value(cue_id, &AttributeKey::intensity())
    );
    assert_eq!(
        observed_again.points()[0].offset_metres,
        initial.points()[0].offset_metres
    );

    let rendered = engine.render_prepared(&captured, &[]).unwrap();
    assert_eq!(rendered.revision, 1);
    assert_eq!(
        rendered.source_snapshot.revision,
        captured.snapshot().revision
    );
    assert_eq!(rendered.generation, captured.generation());
    assert_eq!(rendered.sampled_at, captured.sampled_at());
    assert_eq!(
        rendered.universes[&1][0], 51,
        "captured Programmer value and grand master"
    );
    assert_eq!(
        rendered.universes[&1][9], 102,
        "captured cue and grand master"
    );
    assert_eq!(rendered.points[0].origin_metres, [1.0, 0.0, 0.0]);
    assert_eq!(rendered.points[0].offset_metres, [50.0, 0.0, 0.0]);
    assert!(!rendered.resolved_values.materialised_by_name());

    let live = engine.render(Default::default()).unwrap();
    assert_eq!(live.revision, 2);
    assert_ne!(live.generation, rendered.generation);
    assert_ne!(live.sampled_at, rendered.sampled_at);
    assert_eq!(live.universes[&1][0], 0, "the patch moved after capture");
    assert_eq!(
        live.universes[&1][19], 255,
        "the later Highlight belongs only to the live frame"
    );
    assert_eq!(live.universes[&1][9], 0, "the live cue is Off");
    assert_eq!(live.points[0].origin_metres, [2.0, 0.0, 0.0]);
    assert_eq!(live.points[0].offset_metres, [-50.0, 0.0, 0.0]);
}
