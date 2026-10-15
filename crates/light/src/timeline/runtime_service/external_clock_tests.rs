use super::*;
use light_core::{ApplicationClock, AttributeKey, AttributeValue, FixtureId, ManualClock};
use light_playback::{CueChange, TimecodeCueStart};

struct ExternalRig {
    service: TimecodeRuntimeService,
    transport_clock: Arc<ManualTimecodeClock>,
    clock: Arc<ManualClock>,
    engine: Engine,
    definition: light_playback::TimecodeDefinition,
    list: CueList,
    fixtures: [FixtureId; 2],
    peer: CueList,
    peer_fixture: FixtureId,
}

impl ExternalRig {
    fn new() -> Self {
        let clock = Arc::new(ManualClock::new(
            chrono::DateTime::parse_from_rfc3339("2026-10-15T00:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
        ));
        let transport_clock = Arc::new(ManualTimecodeClock::default());
        let service = TimecodeRuntimeService::new(
            transport_clock.clone(),
            Arc::new(RecordingPublisher::default()),
            TimecodeFrameRate::whole_frames(44).unwrap(),
        );
        let fixtures = [FixtureId::new(), FixtureId::new()];
        let mut list = execution_cue_list();
        for (index, levels) in [[0.2, 0.8], [0.8, 0.2]].into_iter().enumerate() {
            let cue = &mut list.cues[index];
            cue.trigger = CueTrigger::Manual;
            cue.fade_millis = if index == 0 { 0 } else { 2_000 };
            cue.delay_millis = if index == 0 { 0 } else { 500 };
            if index == 0 {
                cue.out_fade_millis = Some(1_000);
                cue.out_delay_millis = Some(250);
            }
            for (fixture, level) in fixtures.into_iter().zip(levels) {
                let mut change = CueChange::set(
                    fixture,
                    AttributeKey::intensity(),
                    AttributeValue::Normalized(level),
                );
                if index == 0 {
                    change.fade_millis = Some(0);
                    change.delay_millis = Some(0);
                }
                cue.changes.push(change);
            }
        }
        let mut definition = execution_definition(&list);
        definition.duration = Some(TimecodeFrame(440));
        let TimecodeLaneContent::CueList { clips, .. } = &mut definition.lanes[0].content else {
            unreachable!()
        };
        clips[0].end_frame = TimecodeFrame(264);
        clips[0].cue_starts = vec![TimecodeCueStart {
            cue_id: list.cues[1].id,
            offset_frame: TimecodeFrame(44),
        }];
        let peer_fixture = FixtureId::new();
        let mut peer = execution_cue_list();
        peer.id = CueListId(Uuid::new_v4());
        peer.cues.truncate(1);
        peer.cues[0].changes.push(CueChange::set(
            peer_fixture,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.3),
        ));
        let engine = Engine::new(ProgrammerRegistry::with_clock(clock.clone()));
        engine
            .replace_snapshot(EngineSnapshot {
                cue_lists: vec![list.clone(), peer.clone()].into(),
                playback_pages: vec![alias_page(1, list.id), alias_page(2, list.id)].into(),
                ..EngineSnapshot::default()
            })
            .unwrap();
        engine
            .execute_playback(EnginePlaybackCommand::CueList {
                id: peer.id,
                action: CueListPlaybackAction::Go,
            })
            .unwrap();
        service.install(definition.clone(), None).unwrap();
        service
            .handle(definition.id, TimecodeTransportAction::Go)
            .unwrap();
        Self {
            service,
            transport_clock,
            clock,
            engine,
            definition,
            list,
            fixtures,
            peer,
            peer_fixture,
        }
    }

    fn advance(&self, millis: i64) {
        self.transport_clock
            .advance(u64::try_from(millis).unwrap() * 1_000);
        self.clock.advance_millis(millis);
    }

    fn external(&self, frame: u64) {
        self.service.synchronize_external(TimecodeFrame(frame));
        self.service.reconcile_cue_lists(&self.engine);
    }

    fn levels(&self, expected: [f32; 2]) {
        let rows = self.engine.playback_contributions_at(self.clock.now());
        for (fixture, expected) in self.fixtures.into_iter().zip(expected) {
            let actual = rows
                .iter()
                .find(|row| row.value.fixture_id == fixture)
                .unwrap()
                .value
                .value
                .normalized()
                .unwrap();
            assert!(
                (actual - expected).abs() < 0.001,
                "fixture {fixture:?}: actual {actual}, expected {expected}"
            );
        }
        let peer = rows
            .iter()
            .find(|row| row.value.fixture_id == self.peer_fixture)
            .unwrap();
        assert!((peer.value.value.normalized().unwrap() - 0.3).abs() < 0.001);
    }
}

#[test]
fn external_held_frames_freeze_real_engine_asymmetric_fades_and_rewind() {
    let rig = ExternalRig::new();
    let peer = rig
        .engine
        .playback_runtime_status_for_cue_list(rig.peer.id)
        .unwrap()
        .playback;
    // Source arithmetic: at77, Cue2 age=.75s; rising .2+.6*(.75-.5)/2=.275,
    // falling .8-.6*(.75-.25)/1=.5. At110: .5/.2; at154: .8/.2.
    for (frame, expected) in [
        (77, [0.275, 0.5]),
        (110, [0.5, 0.2]),
        (154, [0.8, 0.2]),
        (77, [0.275, 0.5]),
    ] {
        rig.external(frame);
        assert!(
            !rig.engine
                .playback_runtime_status_for_cue_list(rig.list.id)
                .unwrap()
                .playback
                .paused,
            "external Playing must not advertise operator Pause"
        );
        rig.levels(expected);
        let ordinal = rig
            .engine
            .playback_runtime_status_for_cue_list(rig.list.id)
            .unwrap()
            .playback
            .transition_ordinal;
        for _ in 0..75 {
            rig.advance(40);
            rig.external(frame);
            rig.levels(expected);
        }
        let active = rig
            .engine
            .playback_runtime_status_for_cue_list(rig.list.id)
            .unwrap()
            .playback;
        assert_eq!(
            active.transition_ordinal, ordinal,
            "held frame must not rebuild every output tick"
        );
        assert!(active.discrete_cue_actions_suppressed);
        assert_eq!(
            rig.service.snapshot(rig.definition.id).unwrap().frame,
            TimecodeFrame(frame)
        );
        assert_eq!(
            rig.engine
                .playback_runtime_status_for_cue_list(rig.peer.id)
                .unwrap()
                .playback,
            peer
        );
    }
    rig.external(264);
    assert!(
        rig.engine
            .playback_runtime_status_for_cue_list(rig.list.id)
            .is_none()
    );
    assert_eq!(
        rig.engine
            .playback_runtime_status_for_cue_list(rig.peer.id)
            .unwrap()
            .playback,
        peer
    );
}

#[test]
fn external_source_loss_pause_keeps_last_authoritative_frame_and_output() {
    let rig = ExternalRig::new();
    rig.external(77);
    rig.advance(501);
    let paused = rig
        .service
        .handle_source_loss(rig.definition.id, TimecodeTransportAction::Pause)
        .unwrap();
    assert_eq!(
        paused.snapshot.frame,
        TimecodeFrame(77),
        "Pause loss must not freewheel through the timeout"
    );
    assert!(paused.snapshot.external_armed);
    rig.service.reconcile_cue_lists(&rig.engine);
    rig.levels([0.275, 0.5]);
    rig.advance(3_000);
    rig.service.tick();
    rig.service.reconcile_cue_lists(&rig.engine);
    rig.levels([0.275, 0.5]);
    rig.external(110);
    assert_eq!(
        rig.service.snapshot(rig.definition.id).unwrap().transport,
        TimecodeTransportState::Playing
    );
    rig.levels([0.5, 0.2]);
}

fn alias_page(number: u8, cue_list_id: CueListId) -> light_playback::PlaybackPage {
    use light_playback::*;
    PlaybackPage {
        number,
        name: format!("Aliases {number}"),
        assignments: std::collections::HashMap::from([(
            1,
            PlaybackDefinition {
                name: "Timeline alias".into(),
                target: PlaybackTarget::CueList { cue_list_id },
                buttons: [
                    PlaybackButtonAction::GoMinus,
                    PlaybackButtonAction::Go,
                    PlaybackButtonAction::Flash,
                ],
                button_count: 3,
                fader: PlaybackFaderMode::Master,
                has_fader: true,
                footprint: PlaybackFootprint::Normal,
                go_activates: true,
                auto_off: false,
                xfade_millis: 0,
                color: "#20c997".into(),
                flash_release: FlashReleaseMode::default(),
                protect_from_swap: false,
                presentation_icon: None,
                presentation_image: None,
            },
        )]),
    }
}

#[test]
fn external_clock_internal_takeover_then_stop_and_relock_keep_their_authority() {
    let rig = ExternalRig::new();
    rig.external(77);
    rig.advance(501);
    rig.service.tick(); // The router only calls tick after Continue Internal takeover.
    assert_eq!(
        rig.service.snapshot(rig.definition.id).unwrap().frame,
        TimecodeFrame(99)
    );
    rig.service.reconcile_cue_lists(&rig.engine);
    rig.levels([0.425, 0.2]);
    rig.advance(250);
    rig.service.tick();
    rig.service.reconcile_cue_lists(&rig.engine);
    rig.levels([0.5, 0.2]);
    rig.external(110);
    rig.advance(3_000);
    rig.external(110);
    rig.levels([0.5, 0.2]);
    rig.service
        .handle_source_loss(rig.definition.id, TimecodeTransportAction::Stop)
        .unwrap();
    rig.service.reconcile_cue_lists(&rig.engine);
    let stopped = rig.service.snapshot(rig.definition.id).unwrap();
    assert_eq!(stopped.frame, TimecodeFrame::ZERO);
    assert!(stopped.external_armed);
    assert!(
        rig.engine
            .playback_runtime_status_for_cue_list(rig.list.id)
            .is_none()
    );
    rig.external(77);
    rig.levels([0.275, 0.5]);
    rig.service
        .handle(rig.definition.id, TimecodeTransportAction::Stop)
        .unwrap();
    rig.external(154);
    assert_eq!(
        rig.service.snapshot(rig.definition.id).unwrap().transport,
        TimecodeTransportState::Stopped
    );
    assert!(
        rig.engine
            .playback_runtime_status_for_cue_list(rig.list.id)
            .is_none()
    );
}

#[test]
fn external_clock_changes_do_not_break_internal_operator_pause_resume() {
    let rig = ExternalRig::new();
    rig.advance(1_750);
    rig.service
        .handle(rig.definition.id, TimecodeTransportAction::Pause)
        .unwrap();
    assert_eq!(
        rig.service.snapshot(rig.definition.id).unwrap().frame,
        TimecodeFrame(77)
    );
    rig.service.reconcile_cue_lists(&rig.engine);
    rig.levels([0.275, 0.5]);
    rig.advance(1_000);
    rig.service.tick();
    rig.service.reconcile_cue_lists(&rig.engine);
    rig.levels([0.275, 0.5]);
    rig.service
        .handle(rig.definition.id, TimecodeTransportAction::Pause)
        .unwrap();
    rig.service.reconcile_cue_lists(&rig.engine);
    rig.advance(250);
    rig.service.tick();
    rig.service.reconcile_cue_lists(&rig.engine);
    rig.levels([0.35, 0.35]);
}

#[test]
fn external_clock_permanent_aliases_share_authority_but_temporary_clock_is_independent() {
    use light_playback::{PlaybackAddress, PlaybackControlSource};
    let rig = ExternalRig::new();
    let a = PlaybackAddress::new(1, 1).unwrap();
    let b = PlaybackAddress::new(2, 1).unwrap();
    rig.external(77);
    let telemetry = rig.engine.playback_telemetry_at(rig.clock.now());
    rig.advance(3_000);
    assert_eq!(rig.engine.playback_telemetry_at(rig.clock.now()), telemetry);
    rig.levels([0.275, 0.5]);
    for address in [a, b] {
        let shared = rig.engine.playback_runtime_status_at(address).unwrap();
        assert_eq!(shared.playback.current_cue_id, Some(rig.list.cues[1].id));
        assert!(!shared.playback.paused);
    }
    let source = PlaybackControlSource {
        surface_id: "hardware:test".into(),
        control_id: 1,
    };
    let command = |action| EnginePlaybackCommand::Playback {
        address: b,
        action,
        activation_origin: None,
        control_source: source.clone(),
    };
    rig.engine
        .execute_playback(command(light_engine::PlaybackAction::SetFlash(true)))
        .unwrap();
    rig.advance(3_000);
    let rows = rig.engine.playback_contributions_at(rig.clock.now());
    let normal = rows
        .iter()
        .find(|row| row.value.fixture_id == rig.fixtures[0] && !row.source.temporary)
        .unwrap();
    let temporary = rows
        .iter()
        .find(|row| row.value.fixture_id == rig.fixtures[0] && row.source.temporary)
        .unwrap();
    assert!((normal.value.value.normalized().unwrap() - 0.275).abs() < 0.001);
    assert!((temporary.value.value.normalized().unwrap() - 0.8).abs() < 0.001);
    rig.engine
        .execute_playback(command(light_engine::PlaybackAction::SetFlash(false)))
        .unwrap();
    rig.levels([0.275, 0.5]);
    rig.engine
        .execute_playback(command(light_engine::PlaybackAction::Back))
        .unwrap();
    assert_eq!(
        rig.engine
            .playback_runtime_status_at(a)
            .unwrap()
            .playback
            .current_cue_id,
        Some(rig.list.cues[0].id)
    );
    rig.engine
        .execute_playback(command(light_engine::PlaybackAction::Go))
        .unwrap();
    rig.advance(750);
    rig.levels([0.275, 0.5]);
    rig.advance(3_000);
    rig.levels([0.8, 0.2]);
}

#[test]
fn external_clock_is_transient_across_show_runtime_restoration() {
    let rig = ExternalRig::new();
    rig.external(77);
    let active = rig
        .engine
        .playback_runtime_status_for_cue_list(rig.list.id)
        .unwrap()
        .playback;
    let peer = rig
        .engine
        .playback_runtime_status_for_cue_list(rig.peer.id)
        .unwrap()
        .playback;
    rig.service.reset_for_show_activation();
    rig.engine
        .execute_playback(EnginePlaybackCommand::RestorePlaybackRuntime {
            playbacks: vec![active, peer],
            dynamic_playbacks: vec![],
        })
        .unwrap();
    assert!(rig.service.snapshots().is_empty());
    rig.advance(3_000);
    rig.levels([0.8, 0.2]);
}

#[derive(Debug)]
struct AdvancingReadClock(ManualClock);
impl ApplicationClock for AdvancingReadClock {
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        self.0.advance_millis(200)
    }
}

#[test]
fn external_clock_sampling_uses_reconstruction_instant_not_later_processing_time() {
    let rig = ExternalRig::new();
    let clock = Arc::new(AdvancingReadClock(ManualClock::new(rig.clock.now())));
    let mut playback = light_playback::PlaybackEngine::with_clock(clock);
    playback.register(rig.list.clone()).unwrap();
    playback
        .reconstruct_sampled_timeline_cues(
            rig.list.id,
            &[(rig.list.cues[0].id, 1_750), (rig.list.cues[1].id, 750)],
        )
        .unwrap();
    for (fixture, expected) in rig.fixtures.into_iter().zip([0.275, 0.5]) {
        let rows = playback.contributions_at(rig.clock.now() + chrono::Duration::hours(1));
        let actual = rows
            .iter()
            .find(|row| row.fixture_id == fixture)
            .unwrap()
            .value
            .normalized()
            .unwrap();
        assert!(
            (actual - expected).abs() < 0.001,
            "actual {actual}, expected {expected}"
        );
    }
}

#[test]
fn external_clock_survives_same_show_engine_generation_replacement() {
    let rig = ExternalRig::new();
    rig.external(77);
    let timecode = rig.service.snapshot(rig.definition.id).unwrap();
    rig.advance(3_000);
    rig.levels([0.275, 0.5]);
    let mut replacement = (*rig.engine.snapshot()).clone();
    // A new immutable Cue allocation forces production Playback compilation. The portable
    // Cue identities/bodies and current Timecode frame remain unchanged, as after a refresh.
    replacement.cue_lists = Arc::new(replacement.cue_lists.as_ref().clone());
    replacement.revision += 1;
    rig.engine.replace_snapshot(replacement).unwrap();
    rig.external(77);
    rig.levels([0.275, 0.5]);
    assert_eq!(
        rig.service.snapshot(rig.definition.id).unwrap().frame,
        timecode.frame
    );
    for _ in 0..75 {
        rig.advance(40);
        rig.external(77);
        rig.levels([0.275, 0.5]);
    }
}

#[test]
fn external_clock_cold_restore_rejects_replaced_owner_fingerprints() {
    let rig = ExternalRig::new();
    let mut source = light_playback::PlaybackEngine::with_clock(rig.clock.clone());
    source.register(rig.list.clone()).unwrap();
    source
        .reconstruct_sampled_timeline_cues(
            rig.list.id,
            &[(rig.list.cues[0].id, 1_750), (rig.list.cues[1].id, 750)],
        )
        .unwrap();
    let clocks = source.capture_timeline_clock_state();
    for change in 0..3 {
        let mut active = source.active().remove(0);
        if change == 1 {
            active.control_source_id = Some(Uuid::new_v4());
        } else if change == 0 {
            active.transition_ordinal += 1;
        }
        let mut destination = light_playback::PlaybackEngine::with_clock(rig.clock.clone());
        let mut next_list = rig.list.clone();
        if change == 2 {
            next_list.cues[1].id = Uuid::new_v4();
        }
        destination.register(next_list).unwrap();
        destination.restore_active([active]);
        destination.restore_timeline_clock_state(clocks.clone());
        // A different accepted owner must keep its ordinary advancing wall clock.
        let rows = destination.contributions_at(rig.clock.now() + chrono::Duration::seconds(3));
        for (fixture, expected) in rig.fixtures.into_iter().zip([0.8, 0.2]) {
            let actual = rows
                .iter()
                .find(|row| row.fixture_id == fixture)
                .unwrap()
                .value
                .normalized()
                .unwrap();
            assert!(
                (actual - expected).abs() < 0.001,
                "replacement owner frozen at {actual}"
            );
        }
    }
}

#[test]
fn external_clock_releasing_generation_and_deleted_target_do_not_inherit_authority() {
    let rig = ExternalRig::new();
    rig.external(77);
    let replacement = rig
        .engine
        .prepare_snapshot((*rig.engine.snapshot()).clone())
        .unwrap();
    // Production show activation finalizes Release, detaching even reused compiled Arcs.
    let replacement = rig
        .engine
        .finalize_snapshot_playback(replacement, false)
        .unwrap();
    rig.service.reset_for_show_activation();
    rig.engine.install_finalized_snapshot(replacement);
    assert!(rig.engine.playback_runtime().is_empty());
    for _ in 0..2 {
        rig.engine
            .execute_playback(EnginePlaybackCommand::CueList {
                id: rig.list.id,
                action: CueListPlaybackAction::Go,
            })
            .unwrap();
    }
    rig.advance(3_000);
    let rows = rig.engine.playback_contributions_at(rig.clock.now());
    for (fixture, expected) in rig.fixtures.into_iter().zip([0.8, 0.2]) {
        let actual = rows
            .iter()
            .find(|row| row.value.fixture_id == fixture)
            .unwrap()
            .value
            .value
            .normalized()
            .unwrap();
        assert!((actual - expected).abs() < 0.001);
    }
    let mut source = light_playback::PlaybackEngine::with_clock(rig.clock.clone());
    source.register(rig.list.clone()).unwrap();
    source
        .reconstruct_sampled_timeline_cues(rig.list.id, &[(rig.list.cues[0].id, 1_750)])
        .unwrap();
    let mut deleted = light_playback::PlaybackEngine::with_clock(rig.clock.clone());
    deleted.restore_active(source.active());
    deleted.restore_timeline_clock_state(source.capture_timeline_clock_state());
    assert!(deleted.active().is_empty());
    assert!(deleted.contributions_at(rig.clock.now()).is_empty());
}
