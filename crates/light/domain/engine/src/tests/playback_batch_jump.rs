use super::*;
use light_playback::{CueAction, PlaybackRuntimeEffect};

#[test]
fn explicit_off_after_automatic_empty_release_resets_jump_history_for_every_alias() {
    let clock = Arc::new(ManualClock::new(Utc::now()));
    let engine = Engine::new(ProgrammerRegistry::with_clock(clock.clone()));
    let (fixture, logical) = fixture();
    let mut first = Cue::new(1_u16.into());
    first.changes.push(CueChange::set(
        logical,
        AttributeKey::intensity(),
        AttributeValue::Normalized(1.0),
    ));
    let mut jump = Cue::new(2_u16.into());
    jump.actions.push(CueAction::Jump {
        cue_id: first.id,
        count: 1,
    });
    let mut release = Cue::new(3_u16.into());
    release.changes.push(CueChange {
        replacement_projection: None,
        preset_reference: None,
        fixture_id: logical,
        attribute: AttributeKey::intensity(),
        value: None,
        automatic_restore: false,
        fade_millis: None,
        delay_millis: None,
    });
    let mut cue_list = test_cue_list("Jump then release", Vec::new());
    cue_list.restart_mode = RestartMode::ContinueCurrentCue;
    cue_list.cues = vec![first, jump, release];
    let cue_list_id = cue_list.id;
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            cue_lists: vec![cue_list].into(),
            playback_pages: vec![light_playback::PlaybackPage {
                number: 1,
                name: "Main".into(),
                assignments: HashMap::from([
                    (1, test_playback(1, cue_list_id)),
                    (2, test_playback(2, cue_list_id)),
                ]),
            }]
            .into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();

    execute_address(&engine, playback_address(1, 1), PlaybackAction::On);
    for expected in [2_u16, 1, 2, 3] {
        execute_address(&engine, playback_address(1, 1), PlaybackAction::Go);
        assert_eq!(
            engine.playback_runtime()[0].current_cue_number,
            Some(expected.into())
        );
    }
    clock.advance_millis(1);
    let _ = engine.prepare_output_frame(RenderOptions::default());
    let released = engine.playback_runtime();
    assert_eq!(released.len(), 1);
    assert!(
        !released[0].enabled,
        "the empty final Cue automatically released"
    );

    let prepared = engine
        .prepare_playback_batch(
            &[PlaybackBatchCommand {
                address: light_playback::PlaybackAddress::new(1, 1).unwrap(),
                control_source: light_playback::PlaybackControlSource {
                    surface_id: "main".into(),
                    control_id: 1,
                },
                action: PlaybackBatchAction::Off,
                activation_origin: None,
            }],
            clock.now(),
            0,
        )
        .unwrap();
    assert_eq!(prepared.effect(), PlaybackRuntimeEffect::Durable);
    assert_eq!(
        prepared.effect_for(playback_address(1, 1)),
        PlaybackRuntimeEffect::Durable
    );
    assert_eq!(
        prepared.effect_for(playback_address(1, 2)),
        PlaybackRuntimeEffect::Durable
    );
    assert_eq!(
        prepared.changed_playback_addresses().collect::<Vec<_>>(),
        vec![playback_address(1, 1), playback_address(1, 2)]
    );
    assert_eq!(
        engine.playback_runtime(),
        released,
        "preparation remains isolated"
    );
    engine.install_prepared_playback_batch(prepared).unwrap();

    // Loading the first Cue avoids the FirstCue restart policy, which would independently clear
    // Jump counters and hide a lost Off effect. The next pass must take the one allowed jump.
    execute_address(
        &engine,
        playback_address(1, 1),
        PlaybackAction::Load(1_u16.into()),
    );
    for expected in [1_u16, 2, 1] {
        execute_address(&engine, playback_address(1, 1), PlaybackAction::Go);
        assert_eq!(
            engine.playback_runtime()[0].current_cue_number,
            Some(expected.into())
        );
    }
}

fn preload_solo_go_rig(explicit_millis: u64) -> (Engine, Arc<ManualClock>, FixtureId) {
    let clock = Arc::new(ManualClock::new(Utc::now()));
    let engine = Engine::new(ProgrammerRegistry::with_clock(clock.clone()));
    let (fixture, logical) = fixture();
    let mut low = test_cue_list(
        "Low",
        vec![CueChange::set(
            logical,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.25),
        )],
    );
    low.cues[0].fade_millis = explicit_millis;
    low.cues[0].out_fade_millis = (explicit_millis > 0).then_some(explicit_millis);
    let mut high = test_cue_list(
        "High",
        vec![CueChange::set(
            logical,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.75),
        )],
    );
    high.cues[0].fade_millis = explicit_millis;
    let low_id = low.id;
    let high_id = high.id;
    let mut low_definition = test_playback(101, low_id);
    let mut high_definition = test_playback(102, high_id);
    // Match native test configuration: full-takeover auto-off is disabled.
    low_definition.auto_off = false;
    high_definition.auto_off = false;
    let low_address = playback_address(2, 101);
    let high_address = playback_address(4, 102);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            cue_lists: vec![low, high].into(),
            playback_pages: vec![
                playback_page(2, [(101, low_definition)]),
                playback_page(4, [(102, high_definition)]),
            ]
            .into(),
            playback_solo_regions: vec![light_playback::PlaybackSoloRegion {
                id: uuid::Uuid::new_v4(),
                name: "Scalar handover".into(),
                members: vec![low_address, high_address],
            }]
            .into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    // Reproduce retained Off with master 1, rather than the untested fresh-runtime branch.
    execute_address(&engine, high_address, PlaybackAction::Go);
    clock.advance_millis(4_000);
    execute_address(&engine, high_address, PlaybackAction::Off);
    execute_address(&engine, low_address, PlaybackAction::Go);
    clock.advance_millis(4_000);
    assert!((normalized(&engine.resolved_values(), logical, "intensity") - 0.25).abs() < 0.001);
    let high = engine
        .playback_runtime_status_at(high_address)
        .unwrap()
        .playback;
    assert!(!high.enabled);
    assert_eq!(high.master, 1.0);
    (engine, clock, logical)
}

fn assert_preload_solo_go_transition(explicit_millis: u64, duration_millis: u64) {
    assert_preload_solo_activation(explicit_millis, duration_millis, PlaybackBatchAction::Go);
}

fn assert_preload_solo_activation(
    explicit_millis: u64,
    duration_millis: u64,
    action: PlaybackBatchAction,
) {
    let (engine, clock, logical) = preload_solo_go_rig(explicit_millis);
    let low = playback_address(2, 101);
    let high = playback_address(4, 102);
    let prepared = engine
        .prepare_playback_batch(
            &[PlaybackBatchCommand {
                address: high,
                action,
                control_source: test_control_source(),
                activation_origin: None,
            }],
            clock.now(),
            3_000,
        )
        .unwrap();
    assert_eq!(prepared.outcomes()[0].released_playbacks, vec![low]);
    // Preparation must not release the Live peer or change its output.
    assert!(
        engine
            .playback_runtime_status_at(low)
            .unwrap()
            .playback
            .enabled
    );
    engine.install_prepared_playback_batch(prepared).unwrap();
    assert!(
        !engine
            .playback_runtime_status_at(low)
            .unwrap()
            .playback
            .enabled
    );
    let incoming = engine.playback_runtime_status_at(high).unwrap().playback;
    assert!(incoming.enabled);
    assert_eq!(incoming.current_cue_number, Some(1_u16.into()));
    assert_eq!(
        incoming.master, 1.0,
        "Activation timing must not add a competing master envelope"
    );
    for (elapsed, expected) in [
        (0, 0.25),
        (duration_millis / 2, 0.5),
        (duration_millis / 2, 0.75),
    ] {
        clock.advance_millis(i64::try_from(elapsed).unwrap());
        let actual = normalized(&engine.resolved_values(), logical, "intensity");
        assert!(
            (actual - expected).abs() < 0.002,
            "Solo handover must start at Low and use one timing envelope: got {actual}, expected {expected}"
        );
    }
}

#[test]
fn preload_solo_retained_off_go_honors_explicit_cue_timing() {
    assert_preload_solo_go_transition(2_000, 2_000);
}

#[test]
fn preload_solo_retained_off_go_uses_programmer_fade_when_cue_is_untimed() {
    assert_preload_solo_go_transition(0, 3_000);
}

#[test]
fn preload_solo_retained_off_on_honors_explicit_cue_timing() {
    assert_preload_solo_activation(2_000, 2_000, PlaybackBatchAction::On);
}

#[test]
fn preload_solo_retained_off_on_uses_untimed_fallback() {
    assert_preload_solo_activation(0, 3_000, PlaybackBatchAction::On);
}

#[test]
fn preload_solo_retained_off_toggle_honors_explicit_cue_timing() {
    assert_preload_solo_activation(2_000, 2_000, PlaybackBatchAction::Toggle);
}

#[test]
fn preload_solo_retained_off_toggle_uses_untimed_fallback() {
    assert_preload_solo_activation(0, 3_000, PlaybackBatchAction::Toggle);
}

fn assert_preload_temp_single_envelope(explicit_millis: u64, duration_millis: u64) {
    let (engine, clock, logical) = preload_solo_go_rig(explicit_millis);
    let low = playback_address(2, 101);
    let high = playback_address(4, 102);
    let prepared = engine
        .prepare_playback_batch(
            &[PlaybackBatchCommand {
                address: high,
                action: PlaybackBatchAction::SetTempButton(true),
                control_source: test_control_source(),
                activation_origin: None,
            }],
            clock.now(),
            3_000,
        )
        .unwrap();
    assert!(
        prepared.outcomes()[0].released_playbacks.is_empty(),
        "Temp must preserve Solo peer"
    );
    engine.install_prepared_playback_batch(prepared).unwrap();
    assert!(
        engine
            .playback_runtime_status_at(low)
            .unwrap()
            .playback
            .enabled
    );
    for (elapsed, expected) in [
        (0, 0.25),
        (duration_millis / 2, 0.375),
        (duration_millis / 2, 0.75),
    ] {
        clock.advance_millis(i64::try_from(elapsed).unwrap());
        let actual = normalized(&engine.resolved_values(), logical, "intensity");
        assert!(
            (actual - expected).abs() < 0.002,
            "Temp must use one Cue/fallback envelope above unchanged HTP Low: got {actual}, expected {expected}"
        );
    }
    execute_address(&engine, high, PlaybackAction::SetTempButton(false));
    assert!((normalized(&engine.resolved_values(), logical, "intensity") - 0.25).abs() < 0.002);
    assert!(
        engine
            .playback_runtime_status_at(low)
            .unwrap()
            .playback
            .enabled
    );
}

#[test]
fn preload_solo_retained_off_temp_honors_explicit_cue_timing() {
    assert_preload_temp_single_envelope(2_000, 2_000);
}

#[test]
fn preload_solo_retained_off_temp_uses_untimed_fallback() {
    assert_preload_temp_single_envelope(0, 3_000);
}
