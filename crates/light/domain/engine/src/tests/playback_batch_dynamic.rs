use super::*;
use light_playback::{
    PlaybackIdentity, PlaybackPage, PlaybackRuntimeEffect, VirtualPlaybackAddress,
};
use uuid::Uuid;

fn command(virtual_playback: bool, action: PlaybackBatchAction) -> PlaybackBatchCommand {
    PlaybackBatchCommand {
        number: if virtual_playback { 1_001 } else { 1 },
        page: virtual_playback.then_some(1),
        action,
        exclusion_zones: Arc::default(),
        activation_origin: None,
    }
}

fn identity(virtual_playback: bool) -> PlaybackIdentity {
    if virtual_playback {
        PlaybackIdentity::Virtual(VirtualPlaybackAddress::new(1, 1_001).unwrap())
    } else {
        PlaybackIdentity::physical(1).unwrap()
    }
}

fn assign(snapshot: &mut EngineSnapshot, definition: PlaybackDefinition, virtual_playback: bool) {
    if virtual_playback {
        snapshot.playback_pages = vec![PlaybackPage {
            number: 1,
            name: "Preview".into(),
            slots: HashMap::new(),
            virtual_playbacks: HashMap::from([(definition.number, definition)]),
        }]
        .into();
    } else {
        snapshot.playbacks = vec![definition].into();
    }
}

fn dynamic_engine(virtual_playback: bool) -> (Engine, Arc<ManualClock>) {
    let clock = Arc::new(ManualClock::new(
        Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
    ));
    let engine = Engine::new(ProgrammerRegistry::with_clock(clock.clone()));
    let dynamic = light_dynamics::DynamicDefinition {
        id: Uuid::new_v4(),
        pool_number: 1,
        revision: 1,
        name: "Preload Dynamic".into(),
        color: None,
        icon: None,
        target_binding: light_dynamics::DynamicTargetBinding::FrozenTargets {
            targets: vec![FixtureId::new()],
        },
        lanes: Vec::new(),
        random_groups: Vec::new(),
        phase_spread_mode: light_dynamics::DynamicPhaseSpreadMode::Uniform,
        spatial_mapping: Default::default(),
        phase: light_dynamics::PhaseDistribution {
            ordering: light_dynamics::PhaseOrdering::Selection,
            offset_degrees: 0.0,
            span_degrees: 360.0,
            block_size: 1,
            repeats: 1,
            wings: false,
            anchors_degrees: Vec::new(),
        },
        speed: light_dynamics::DynamicSpeed::Fixed {
            duration_millis: 1_000,
        },
        overall_speed_multiplier: light_dynamics::Rational::ONE,
        run_mode: light_dynamics::DynamicRunMode::Loop,
        default_activation: light_dynamics::ActivationPolicy::StartNow,
        activation_boundary: light_dynamics::ActivationBoundary::Beat,
    };
    let target = PlaybackTarget::Dynamic {
        assignment: light_playback::DynamicPlaybackAssignment {
            dynamic: light_dynamics::DynamicReference {
                dynamic_id: Some(dynamic.id),
                last_known_pool_number: 1,
                embedded_fallback: light_dynamics::DynamicDefinitionSnapshot {
                    definition: Arc::new(dynamic),
                },
            },
            revision: 1,
            target_scope: None,
            fader_mode: light_playback::DynamicPlaybackFaderMode::SizeAndMaster,
            priority: 0,
            activation_override: None,
            resume_policy: light_playback::DynamicPlaybackResumePolicy::FollowDynamic,
            local_speed_multiplier: light_dynamics::Rational::ONE,
            learned_duration_millis: None,
            crossfade_non_intensity: false,
            auto_off_at_zero: true,
            auto_off_flash_release: true,
            auto_off_full_control: true,
        },
    };
    let mut snapshot = EngineSnapshot::default();
    assign(
        &mut snapshot,
        test_playback_for_target(
            command(virtual_playback, PlaybackBatchAction::On).number,
            target,
        ),
        virtual_playback,
    );
    engine.replace_snapshot(snapshot).unwrap();
    (engine, clock)
}

#[test]
fn queued_dynamic_controls_skip_cue_timing_and_install_their_changed_runtime() {
    for virtual_playback in [false, true] {
        let (engine, clock) = dynamic_engine(virtual_playback);
        for action in [
            PlaybackBatchAction::On,
            PlaybackBatchAction::TogglePause,
            PlaybackBatchAction::DynamicRestart,
            PlaybackBatchAction::DynamicDoubleSpeed,
            PlaybackBatchAction::DynamicHalfSpeed,
            PlaybackBatchAction::DynamicLearnSpeed,
            PlaybackBatchAction::SetFader {
                value_permyriad: 4_000,
            },
            PlaybackBatchAction::Off,
            PlaybackBatchAction::Toggle,
        ] {
            clock.advance_millis(500);
            let before = engine.active_dynamic_playbacks();
            let prepared = engine
                .prepare_playback_batch(&[command(virtual_playback, action)], clock.now(), 800)
                .unwrap();
            let pickup_only =
                !virtual_playback && matches!(action, PlaybackBatchAction::SetFader { .. });
            let expected_effect = if pickup_only {
                PlaybackRuntimeEffect::Transient
            } else {
                PlaybackRuntimeEffect::Durable
            };
            assert_eq!(prepared.effect(), expected_effect, "{action:?}");
            assert_eq!(
                engine.active_dynamic_playbacks(),
                before,
                "preparation remains isolated"
            );
            if !virtual_playback {
                assert_eq!(prepared.effect_for(1), expected_effect);
                assert_eq!(
                    prepared.changed_playback_numbers().collect::<Vec<_>>(),
                    vec![1]
                );
            }
            engine.install_prepared_playback_batch(prepared).unwrap();
            let current = engine
                .active_dynamic_playback_at(identity(virtual_playback))
                .unwrap();
            if pickup_only {
                assert_eq!(
                    engine.active_dynamic_playbacks(),
                    before,
                    "pickup cannot change the live Dynamic level"
                );
                let control = engine.playback_control_state_at(identity(false));
                assert_eq!(control.fader_position, 0.4);
                assert!(control.fader_pickup_required);
            } else {
                assert_ne!(
                    engine.active_dynamic_playbacks(),
                    before,
                    "{action:?} was installed"
                );
            }
            match action {
                PlaybackBatchAction::On | PlaybackBatchAction::Toggle => assert!(current.enabled),
                PlaybackBatchAction::TogglePause => assert!(current.paused),
                PlaybackBatchAction::DynamicRestart => {
                    assert!(!current.paused);
                    assert_eq!(current.activated_at, clock.now());
                }
                PlaybackBatchAction::DynamicDoubleSpeed => {
                    assert_eq!(current.local_speed_multiplier.factor(), 2.0)
                }
                PlaybackBatchAction::DynamicHalfSpeed => {
                    assert_eq!(current.local_speed_multiplier.factor(), 1.0)
                }
                PlaybackBatchAction::DynamicLearnSpeed => assert_eq!(
                    current.last_learn_tap_millis,
                    Some(clock.now().timestamp_millis() as u64)
                ),
                PlaybackBatchAction::SetFader { .. } => {
                    let level = if pickup_only { 1.0 } else { 0.4 };
                    assert_eq!(current.size, level);
                    assert_eq!(current.master, level);
                }
                PlaybackBatchAction::Off => assert!(!current.enabled),
                _ => unreachable!(),
            }
        }
    }
}

#[test]
fn dynamic_batch_exact_cancellation_and_repeated_on_have_no_retained_effect() {
    for virtual_playback in [false, true] {
        let (engine, clock) = dynamic_engine(virtual_playback);
        let initial = engine
            .prepare_playback_batch(
                &[command(virtual_playback, PlaybackBatchAction::On)],
                clock.now(),
                900,
            )
            .unwrap();
        engine.install_prepared_playback_batch(initial).unwrap();
        let before = engine.active_dynamic_playbacks();
        clock.advance_millis(1_000);
        let prepared = engine
            .prepare_playback_batch(
                &[
                    command(virtual_playback, PlaybackBatchAction::TogglePause),
                    command(virtual_playback, PlaybackBatchAction::TogglePause),
                    command(virtual_playback, PlaybackBatchAction::On),
                ],
                clock.now(),
                900,
            )
            .unwrap();
        assert_eq!(prepared.effect(), PlaybackRuntimeEffect::None);
        assert_eq!(prepared.changed_playback_numbers().count(), 0);
        assert_eq!(
            prepared.outcomes()[0].effect,
            PlaybackRuntimeEffect::Durable
        );
        assert_eq!(
            prepared.outcomes()[1].effect,
            PlaybackRuntimeEffect::Durable
        );
        assert_eq!(prepared.outcomes()[2].effect, PlaybackRuntimeEffect::None);
        engine.install_prepared_playback_batch(prepared).unwrap();
        assert_eq!(engine.active_dynamic_playbacks(), before);
    }
}

#[test]
fn queued_cues_keep_explicit_timing_and_use_programmer_fallback_when_unspecified() {
    for virtual_playback in [false, true] {
        for (explicit_fade, expected_half_second) in [(0, 0.5), (2_000, 0.25)] {
            let clock = Arc::new(ManualClock::new(
                Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
            ));
            let engine = Engine::new(ProgrammerRegistry::with_clock(clock.clone()));
            let (fixture, target) = fixture();
            let mut list = test_cue_list(
                "Timing",
                vec![CueChange::set(
                    target,
                    AttributeKey::intensity(),
                    AttributeValue::Normalized(0.0),
                )],
            );
            let mut next = Cue::new(2_u16.into());
            next.fade_millis = explicit_fade;
            next.changes.push(CueChange::set(
                target,
                AttributeKey::intensity(),
                AttributeValue::Normalized(1.0),
            ));
            list.cues.push(next);
            let definition = test_playback(
                command(virtual_playback, PlaybackBatchAction::On).number,
                list.id,
            );
            let mut snapshot = EngineSnapshot {
                fixtures: vec![fixture].into(),
                cue_lists: vec![list].into(),
                ..Default::default()
            };
            assign(&mut snapshot, definition, virtual_playback);
            engine.replace_snapshot(snapshot).unwrap();
            let on = engine
                .prepare_playback_batch(
                    &[command(virtual_playback, PlaybackBatchAction::On)],
                    clock.now(),
                    0,
                )
                .unwrap();
            engine.install_prepared_playback_batch(on).unwrap();
            clock.advance_millis(100);
            let go = engine
                .prepare_playback_batch(
                    &[command(virtual_playback, PlaybackBatchAction::Go)],
                    clock.now(),
                    1_000,
                )
                .unwrap();
            engine.install_prepared_playback_batch(go).unwrap();
            assert_eq!(
                engine.playback_runtime()[0].transition_fade_fallback_millis,
                Some(1_000)
            );
            clock.advance_millis(500);
            let rendered = engine.render(RenderOptions::default()).unwrap();
            let value = rendered
                .resolved_values
                .value(target, &AttributeKey::intensity())
                .and_then(AttributeValue::normalized)
                .unwrap();
            assert!(
                (value - expected_half_second).abs() < 0.0001,
                "virtual={virtual_playback}, explicit={explicit_fade}: {value}"
            );
        }
    }
}

fn capture_queue(engine: &Engine, commands: &[PlaybackBatchCommand]) -> PreparedOutputFrame {
    let session = SessionId::new();
    engine.programmers.start(session);
    for command in commands {
        let action = match command.action {
            PlaybackBatchAction::Go => light_programmer::PreloadPlaybackQueueAction::Go,
            PlaybackBatchAction::On => light_programmer::PreloadPlaybackQueueAction::On,
            PlaybackBatchAction::TogglePause => {
                light_programmer::PreloadPlaybackQueueAction::DynamicPause
            }
            PlaybackBatchAction::DynamicDoubleSpeed => {
                light_programmer::PreloadPlaybackQueueAction::DynamicDoubleSpeed
            }
            PlaybackBatchAction::SetFader { value_permyriad } => {
                light_programmer::PreloadPlaybackQueueAction::Fader { value_permyriad }
            }
            _ => panic!("this test helper needs a matching captured action"),
        };
        assert!(engine.programmers.queue_preload_playback_action(
            session,
            command.number,
            command.page,
            action,
            if command.page.is_some() {
                light_programmer::PreloadPlaybackQueueSurface::Virtual
            } else {
                light_programmer::PreloadPlaybackQueueSurface::Physical
            },
        ));
    }
    engine.prepare_output_frame(RenderOptions::default())
}

fn queued_cue_engine(virtual_playback: bool) -> (Engine, Arc<ManualClock>) {
    let clock = Arc::new(ManualClock::new(
        Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
    ));
    let engine = Engine::new(ProgrammerRegistry::with_clock(clock.clone()));
    let (fixture, target) = fixture();
    let mut list = test_cue_list("Captured queue", Vec::new());
    list.cues = [0.2, 0.5, 0.8]
        .into_iter()
        .enumerate()
        .map(|(index, level)| {
            let mut cue = Cue::new((index + 1).into());
            cue.changes.push(CueChange::set(
                target,
                AttributeKey::intensity(),
                AttributeValue::Normalized(level),
            ));
            cue
        })
        .collect();
    let definition = test_playback(
        command(virtual_playback, PlaybackBatchAction::On).number,
        list.id,
    );
    let mut snapshot = EngineSnapshot {
        fixtures: vec![fixture].into(),
        cue_lists: vec![list].into(),
        ..Default::default()
    };
    assign(&mut snapshot, definition, virtual_playback);
    engine.replace_snapshot(snapshot).unwrap();
    let on = engine
        .prepare_playback_batch(
            &[command(virtual_playback, PlaybackBatchAction::On)],
            clock.now(),
            0,
        )
        .unwrap();
    engine.install_prepared_playback_batch(on).unwrap();
    (engine, clock)
}

#[test]
fn captured_queue_executes_duplicate_go_in_order_from_its_original_clock_and_generation() {
    for virtual_playback in [false, true] {
        let (engine, clock) = queued_cue_engine(virtual_playback);
        engine.set_control_timing([120.0; 5], 700, 0, 0);
        clock.advance_millis(100);
        let commands = [
            command(virtual_playback, PlaybackBatchAction::Go),
            command(virtual_playback, PlaybackBatchAction::Go),
        ];
        let frame = capture_queue(&engine, &commands);
        let captured_queue = Arc::clone(&frame.programmer().preload_playback_actions);
        assert_eq!(captured_queue.len(), 2);
        assert_eq!(captured_queue[0], captured_queue[1]);
        assert_eq!(
            frame.preload_playback_source().unwrap().runtime()[0].cue_index,
            0
        );

        clock.advance_millis(5_000);
        engine.set_control_timing([60.0; 5], 9_000, 0, 0);
        engine.replace_snapshot(EngineSnapshot::default()).unwrap();
        let live_after_replacement = engine.playback_runtime();
        let current_queue = engine.programmers.capture_output_sources();
        for _ in 0..2 {
            let result = engine
                .prepare_preload_playback_batch(&frame, &commands)
                .unwrap();
            assert_eq!(result.outcomes().len(), 2);
            assert_eq!(result.effect(), PlaybackRuntimeEffect::Durable);
            let runtimes = result.runtime().runtime();
            assert_eq!(runtimes.len(), 1);
            assert_eq!(
                runtimes[0].cue_index, 2,
                "duplicate Go actions each advance exactly once"
            );
            assert_eq!(runtimes[0].activated_at, frame.sampled_at());
            assert_eq!(runtimes[0].transition_fade_fallback_millis, Some(700));
            assert_eq!(result.runtime().clock().now(), frame.sampled_at());
            assert_eq!(engine.playback_runtime(), live_after_replacement);
            assert_eq!(
                frame.preload_playback_source().unwrap().runtime()[0].cue_index,
                0
            );
            assert_eq!(
                frame.programmer().preload_playback_actions.as_ref(),
                captured_queue.as_ref()
            );
            assert_eq!(
                engine
                    .programmers
                    .capture_output_sources()
                    .preload_playback_actions
                    .as_ref(),
                current_queue.preload_playback_actions.as_ref()
            );
        }
    }
}

#[test]
fn captured_queue_failure_discards_earlier_commands_without_mutating_live_or_the_queue() {
    let (engine, _) = queued_cue_engine(false);
    let commands = [
        command(false, PlaybackBatchAction::Go),
        command(
            false,
            PlaybackBatchAction::SetFader {
                value_permyriad: 10_001,
            },
        ),
    ];
    let frame = capture_queue(&engine, &commands);
    let live = engine.playback_runtime();
    let queue = Arc::clone(&frame.programmer().preload_playback_actions);
    let queue_generation = frame.programmer().preload_playback_queue_generation;
    assert!(
        engine
            .prepare_preload_playback_batch(&frame, &commands)
            .is_err()
    );
    assert_eq!(engine.playback_runtime(), live);
    assert_eq!(frame.preload_playback_source().unwrap().runtime(), live);
    let after = engine.programmers.capture_output_sources();
    assert_eq!(after.preload_playback_queue_generation, queue_generation);
    assert!(Arc::ptr_eq(&after.preload_playback_actions, &queue));
    let retried = engine
        .prepare_preload_playback_batch(&frame, &commands[..1])
        .unwrap();
    assert_eq!(
        retried.runtime().runtime()[0].cue_index,
        1,
        "failed Go was not retained"
    );
    let empty = engine.prepare_preload_playback_batch(&frame, &[]).unwrap();
    assert_eq!(empty.effect(), PlaybackRuntimeEffect::None);
    assert!(empty.outcomes().is_empty());
}

#[test]
fn captured_dynamic_queue_is_isolated_and_requires_an_explicit_captured_base() {
    for virtual_playback in [false, true] {
        let (engine, _) = dynamic_engine(virtual_playback);
        let missing = engine.prepare_output_frame(RenderOptions::default());
        let commands = [
            command(virtual_playback, PlaybackBatchAction::On),
            command(virtual_playback, PlaybackBatchAction::TogglePause),
            command(virtual_playback, PlaybackBatchAction::DynamicDoubleSpeed),
        ];
        assert!(
            engine
                .prepare_preload_playback_batch(&missing, &commands)
                .is_err()
        );
        let frame = capture_queue(&engine, &commands);
        let result = engine
            .prepare_preload_playback_batch(&frame, &commands)
            .unwrap();
        let active = result
            .runtime()
            .active_dynamic_playback_at(identity(virtual_playback))
            .unwrap();
        assert!(active.enabled);
        assert!(active.paused);
        assert_eq!(active.local_speed_multiplier.factor(), 2.0);
        assert_eq!(active.activated_at, frame.sampled_at());
        assert_eq!(result.effect(), PlaybackRuntimeEffect::Durable);
        assert_eq!(result.outcomes().len(), 3);
        assert!(engine.active_dynamic_playbacks().is_empty());
        assert!(
            frame
                .preload_playback_source()
                .unwrap()
                .active_dynamic_playbacks()
                .is_empty()
        );
        assert_eq!(
            engine
                .programmers
                .capture_output_sources()
                .preload_playback_actions
                .len(),
            3
        );
    }
}
