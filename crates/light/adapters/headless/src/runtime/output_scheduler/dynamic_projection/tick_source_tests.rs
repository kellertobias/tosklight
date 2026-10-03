use super::*;
use light_programmer::ProgrammerRegistry;
use std::sync::atomic::{AtomicBool, Ordering};

#[test]
fn scalar_stage_applies_captured_cue_suppression_before_point_geometry() {
    let engine = Engine::new(ProgrammerRegistry::default());
    let frame = engine.prepare_output_frame(Default::default());
    let sources = TickSources::prepared(&engine, &frame, &[]);
    let controller = Uuid::new_v4();
    let target = FixtureId::new();
    let sample = |attribute| light_dynamics::DynamicRuntimeSample {
        instance_id: Uuid::new_v4(),
        controller_id: controller,
        target,
        lane_id: Uuid::new_v4(),
        expression: light_dynamics::DynamicSampleExpression::LegacyScalar {
            attribute,
            value: 0.8,
            occurrence: None,
            dependency_occurrence: None,
        },
        priority: 100,
        activated_at_millis: 100,
        activation_mix: 1.0,
        address: None,
    };
    let samples = [
        sample(AttributeKey("point.position.x".into())),
        sample(AttributeKey::intensity()),
    ];
    let collect = |enabled| {
        collect_dynamic_candidates(
            &frame.frame_addresser(),
            &[],
            &[],
            &[],
            &samples,
            &HashMap::new(),
            &HashMap::from([(
                controller,
                CueDynamicOutputControl {
                    enabled,
                    sequence_master: 0.5,
                },
            )]),
            &sources,
            100,
        )
    };
    assert!(
        collect(false).is_empty(),
        "suppressed Cue motion must not move the geometry used by typed sampling"
    );
    let enabled = collect(true);
    assert_eq!(enabled.len(), 2);
    for stack in enabled.values() {
        let expected = if stack.attribute.is_intensity() {
            0.4
        } else {
            0.8
        };
        assert_eq!(
            stack.candidates[0].value,
            AttributeValue::Normalized(expected)
        );
    }
    assert!(
        sources.values.get().is_none(),
        "fully active constants need no extra Current solve"
    );
}

#[test]
fn semantic_activation_blends_the_complete_owner_and_holds_unknown_appearance() {
    use light_core::programming::{ColorIntent, ColorProgram, PositionIntent, UvIntent};
    let position = |pan| AttributeValue::Position(Arc::new(PositionIntent::angles(pan, 0.0)));
    assert_eq!(
        blend_attribute_value(position(0.0), position(720.0), 0.5),
        position(360.0)
    );
    let color = |uv| {
        AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
            intent: ColorIntent {
                uv: UvIntent { amount: uv },
                ..Default::default()
            },
        }))
    };
    let AttributeValue::ColorProgram(blended) = blend_attribute_value(color(0.0), color(1.0), 0.25)
    else {
        panic!()
    };
    let ColorProgram::Semantic { intent } = blended.as_ref() else {
        panic!()
    };
    assert_eq!(intent.uv.amount, 0.25);
    assert_eq!(intent.recipe, ColorIntent::default().recipe);
    // A legacy scalar cannot be reinterpreted as a complete physical family at half mix.
    assert_eq!(
        blend_attribute_value(AttributeValue::Normalized(0.2), color(1.0), 0.75),
        AttributeValue::Normalized(0.2)
    );
    assert_eq!(
        blend_attribute_value(AttributeValue::Normalized(0.2), color(1.0), 1.0),
        color(1.0)
    );
}

#[test]
fn tick_sources_defer_whole_show_resolution() {
    let engine = Engine::new(ProgrammerRegistry::default());
    let sources = TickSources::new(&engine);

    assert!(
        sources.values.get().is_none(),
        "constructing an output tick must not eagerly resolve the whole show"
    );
}

#[test]
fn prepared_dynamic_tick_pins_programmer_sources_clock_and_lazy_current() {
    let started = chrono::DateTime::from_timestamp_millis(1_000_000).unwrap();
    let clock = Arc::new(light_core::ManualClock::new(started));
    let programmers = ProgrammerRegistry::with_clock(clock.clone());
    let session = light_core::SessionId::new();
    let fixture = FixtureId::new();
    let attribute = AttributeKey("pan".into());
    programmers.start(session);
    programmers.set(
        session,
        fixture,
        attribute.clone(),
        AttributeValue::Normalized(0.2),
    );
    let set_fat = |value| {
        assert!(programmers.apply_dynamic_values(
            session,
            &[light_programmer::DynamicProgrammerValueMutation::Set {
                fixture_id: fixture,
                attribute: attribute.clone(),
                value: light_dynamics::DynamicSemanticValue::Static {
                    value: AttributeValue::Normalized(value),
                    timing: light_dynamics::DynamicValueTiming {
                        fade_millis: Some(1_000),
                        delay_millis: None,
                    },
                },
            }],
            None,
        ));
    };
    // Legacy final contribution arbitration uses authored timestamps for distinct sources.
    // Keep this source-capture test independent of an exact-timestamp LTP tie.
    clock.advance_millis(1);
    set_fat(1.0);
    let engine = Engine::new(programmers.clone());
    clock.advance_millis(500);
    let frame = engine.prepare_output_frame(RenderOptions::default());
    let sources = TickSources::prepared(&engine, &frame, &[]);
    assert!(sources.values.get().is_none());

    // Neither first use of Current nor the Dynamic/FAT projection may reread these edits.
    clock.advance_millis(400);
    programmers.set(
        session,
        fixture,
        attribute.clone(),
        AttributeValue::Normalized(0.9),
    );
    set_fat(0.1);
    assert_eq!(sources.current(fixture, &attribute), Some(0.2));
    let dynamics = Mutex::new(light_dynamics::DynamicRuntime::default());
    let speed_groups = Mutex::new(std::array::from_fn(|_| {
        light_control::speed::SpeedGroupController::new(120.0, Default::default()).unwrap()
    }));
    let (sampled, _, _, _, _) = dynamic_contributions_prepared(
        &engine,
        &frame,
        &[],
        &dynamics,
        &speed_groups,
        &AtomicU16::new(40),
        &[],
        None,
        false,
    );
    assert_eq!(sampled.len(), 1);
    let sampled_value = sampled[0].samples()[0].value().value.normalized().unwrap();
    assert!(
        (sampled_value - 0.6).abs() < 0.0001,
        "captured half-fade from Current 0.2"
    );
    let rendered = engine.render_prepared(&frame, &sampled).unwrap();
    assert_eq!(
        rendered.sampled_at,
        started + chrono::Duration::milliseconds(501)
    );
    assert!(
        (rendered
            .resolved_values
            .value(fixture, &attribute)
            .and_then(AttributeValue::normalized)
            .unwrap()
            - 0.6)
            .abs()
            < 0.0001
    );
}

#[test]
fn preload_dynamic_projection_uses_pending_current_and_only_mutates_its_fork() {
    let started = chrono::DateTime::from_timestamp_millis(1_000_000).unwrap();
    let clock = Arc::new(light_core::ManualClock::new(started));
    let programmers = ProgrammerRegistry::with_clock(clock.clone());
    let session = light_core::SessionId::new();
    let fixture = FixtureId::new();
    let attribute = AttributeKey("pan".into());
    programmers.start(session);
    programmers.set(
        session,
        fixture,
        attribute.clone(),
        AttributeValue::Normalized(0.2),
    );
    programmers.arm_preload(session, true);
    clock.advance_millis(1);
    programmers.set(
        session,
        fixture,
        attribute.clone(),
        AttributeValue::Normalized(0.6),
    );
    let set_pending_fat = |value| {
        assert!(programmers.apply_dynamic_values(
            session,
            &[light_programmer::DynamicProgrammerValueMutation::Set {
                fixture_id: fixture,
                attribute: attribute.clone(),
                value: light_dynamics::DynamicSemanticValue::Static {
                    value: AttributeValue::Normalized(value),
                    timing: light_dynamics::DynamicValueTiming {
                        fade_millis: Some(1_000),
                        delay_millis: None,
                    },
                },
            }],
            None,
        ));
    };
    clock.advance_millis(1);
    set_pending_fat(1.0);
    let engine = Engine::new(programmers.clone());
    engine
        .execute_playback(light_engine::EnginePlaybackCommand::SetDynamicsPaused(true))
        .unwrap();
    clock.advance_millis(500);
    let frame = engine.prepare_output_frame(RenderOptions::default());
    let input = engine.prepare_preload_frame(&frame, None);
    let state = light_engine::PreloadFrameState::default();
    let sources = PreloadTickSources {
        engine: &engine,
        input: &input,
        state: &state,
        before_release: false,
        baseline_samples: &[],
        values: OnceLock::new(),
    };
    assert!(sources.values.get().is_none());
    let live_dynamics = light_dynamics::DynamicRuntime::default();
    let live_before = live_dynamics.output_projection_snapshot();
    let mut preview_dynamics = live_dynamics.fork_for_preview();
    let speed_groups = Mutex::new(std::array::from_fn(|_| {
        light_control::speed::SpeedGroupController::new(120.0, Default::default()).unwrap()
    }));
    let speed_transports = capture_dynamic_speed_transports(
        &speed_groups,
        u64::try_from(frame.sampled_at().timestamp_millis()).unwrap(),
    );

    // Edits after capture must affect neither the first lazy Current lookup nor sampling.
    clock.advance_millis(400);
    programmers.set(
        session,
        fixture,
        attribute.clone(),
        AttributeValue::Normalized(0.1),
    );
    set_pending_fat(0.0);
    engine
        .execute_playback(light_engine::EnginePlaybackCommand::SetDynamicsPaused(
            false,
        ))
        .unwrap();
    assert_eq!(sources.current(fixture, &attribute), Some(0.6));
    assert_eq!(
        TickSources::prepared(&engine, &frame, &[]).current(fixture, &attribute),
        Some(0.2),
    );
    let (batches, samples) = dynamic_projection_preload(
        &engine,
        &input,
        &state,
        false,
        &[],
        &mut preview_dynamics,
        &speed_transports,
        40,
    );
    assert!(
        samples.is_empty(),
        "Static/FAT produces a candidate, not a running lane"
    );
    assert_eq!(batches.len(), 1);
    let actual = batches[0].samples()[0].value().value.normalized().unwrap();
    assert!(
        (actual - 0.8).abs() < 0.0001,
        "half fade must start from pending Current0.6"
    );
    assert!(preview_dynamics.output_projection_snapshot().global_paused);
    assert_eq!(live_dynamics.output_projection_snapshot(), live_before);
    assert!(!engine.playback_dynamics().paused);
}

#[test]
fn prepared_current_and_render_share_extra_static_samples() {
    let engine = Engine::new(ProgrammerRegistry::default());
    let frame = engine.prepare_output_frame(RenderOptions::default());
    let fixture = FixtureId::new();
    let attribute = AttributeKey("audio.volume".into());
    let baseline = [ContributionBatch::new([ContributionSample::independent(
        TimedValue {
            fixture_id: fixture,
            attribute: attribute.clone(),
            value: AttributeValue::Normalized(0.35),
            priority: 75,
            changed_at: frame.sampled_at(),
            programmer_order: 1,
            merge_mode: MergeMode::Ltp,
            fade: false,
            fade_millis: None,
            delay_millis: None,
        },
    )])];
    let sources = TickSources::prepared(&engine, &frame, &baseline);
    assert!(sources.values.get().is_none());
    assert_eq!(sources.current(fixture, &attribute), Some(0.35));
    let rendered = engine.render_prepared(&frame, &baseline).unwrap();
    assert_eq!(
        rendered
            .resolved_values
            .value(fixture, &attribute)
            .and_then(AttributeValue::normalized),
        Some(0.35)
    );
}

#[test]
fn fully_active_first_candidate_does_not_resolve_the_underlay() {
    let underlay_requested = AtomicBool::new(false);
    let stack = [DynamicCandidate {
        value: AttributeValue::Normalized(0.75),
        priority: 10,
        changed_at_millis: 1,
        exact_changed_at: None,
        stable_order: 1,
        activation_mix: 1.0,
        dynamic: true,
    }];

    let resolved = resolve_dynamic_stack(&stack, || {
        underlay_requested.store(true, Ordering::Relaxed);
        Some(AttributeValue::Normalized(0.25))
    });

    assert_eq!(resolved, AttributeValue::Normalized(0.75));
    assert!(
        !underlay_requested.load(Ordering::Relaxed),
        "a fully active winning candidate makes the underlay irrelevant"
    );
}

#[test]
fn fading_first_candidate_blends_from_the_underlay() {
    let stack = [DynamicCandidate {
        value: AttributeValue::Normalized(1.0),
        priority: 10,
        changed_at_millis: 1,
        exact_changed_at: None,
        stable_order: 1,
        activation_mix: 0.5,
        dynamic: true,
    }];

    let resolved = resolve_dynamic_stack(&stack, || Some(AttributeValue::Normalized(0.0)));

    assert_eq!(resolved, AttributeValue::Normalized(0.5));
}

#[test]
fn only_a_completely_static_tick_is_idle() {
    assert!(dynamic_tick_is_idle_from_presence(
        true, false, false, false, false, false, false,
    ));

    for non_idle in [
        dynamic_tick_is_idle_from_presence(false, false, false, false, false, false, false),
        dynamic_tick_is_idle_from_presence(true, false, true, false, false, false, false),
        dynamic_tick_is_idle_from_presence(true, false, false, true, false, false, false),
        dynamic_tick_is_idle_from_presence(true, false, false, false, true, false, false),
        dynamic_tick_is_idle_from_presence(true, false, false, false, false, true, false),
        dynamic_tick_is_idle_from_presence(true, false, false, false, false, false, true),
    ] {
        assert!(
            !non_idle,
            "runtime, pause, Dynamic/FAT, Playback, and extra inputs must use the full path"
        );
    }
}

#[test]
fn matching_paused_state_can_still_be_idle() {
    assert!(dynamic_tick_is_idle_from_presence(
        true, true, true, false, false, false, false,
    ));
}

#[test]
fn programmer_reconciliation_ignores_unrelated_engine_snapshot_replacement() {
    let cache = ProgrammerReconciliationCache::default();
    let values = Arc::new(Vec::new());
    let snapshot = Arc::new(light_engine::EngineSnapshot::default());

    assert!(cache.changed(&values, &snapshot));
    assert!(
        cache.changed(&values, &snapshot),
        "comparison cannot acknowledge an uncommitted frame"
    );
    cache.acknowledge(&values, &snapshot);
    assert!(!cache.changed(&values, &Arc::new((*snapshot).clone())));

    let mut changed_definitions = (*snapshot).clone();
    changed_definitions.dynamics = Arc::new(Vec::new());
    assert!(cache.changed(&values, &Arc::new(changed_definitions)));

    let mut changed_positions = (*snapshot).clone();
    changed_positions.dynamic_stage_positions = Arc::new(HashMap::new());
    assert!(cache.changed(&values, &Arc::new(changed_positions)));

    assert!(cache.changed(&Arc::new(Vec::new()), &snapshot));
}

/// TL-553: the hybrid frame reads scalar Current from its static family lane instead of a second
/// resolution. Every value and family evidence must equal the observed resolution, and a capture
/// with a Freeze must keep the observation (which applies the Freeze overrides).
#[test]
fn static_lane_scalar_sources_equal_the_observed_resolution_and_freeze_keeps_observing() {
    use super::physical_adapter::color::profiles::{patched, rgbw};
    let programmers = ProgrammerRegistry::default();
    let session = light_core::SessionId::new();
    programmers.start(session);
    let engine = Engine::new(programmers.clone());
    let (lamp, frozen) = (FixtureId::new(), FixtureId::new());
    let install = |freeze: bool| {
        let mut held = patched(&rgbw(), frozen, 20);
        held.fixture_number = Some(2);
        if freeze {
            held.freeze.targets.insert(
                frozen,
                light_fixture::FrozenFixtureTarget {
                    values: HashMap::from([(
                        AttributeKey::intensity(),
                        AttributeValue::Normalized(0.125),
                    )]),
                    ..Default::default()
                },
            );
        }
        engine
            .replace_snapshot(light_engine::EngineSnapshot {
                fixtures: vec![patched(&rgbw(), lamp, 1), held].into(),
                revision: 1 + u64::from(freeze),
                ..Default::default()
            })
            .unwrap();
    };
    for (fixture, attribute, value) in [
        (lamp, "intensity", 0.6),
        (lamp, "color.red", 0.3),
        (frozen, "intensity", 0.9),
        (frozen, "color.blue", 0.4),
    ] {
        programmers.set(
            session,
            fixture,
            AttributeKey(attribute.into()),
            AttributeValue::Normalized(value),
        );
    }
    let attributes = [
        "intensity",
        "color.red",
        "color.green",
        "color.blue",
        "audio.volume",
    ]
    .map(|name| AttributeKey(name.into()));
    for freeze in [false, true] {
        install(freeze);
        let frame = engine.prepare_output_frame(RenderOptions::default());
        let baseline = [ContributionBatch::new([ContributionSample::independent(
            TimedValue {
                fixture_id: lamp,
                attribute: AttributeKey("audio.volume".into()),
                value: AttributeValue::Normalized(0.35),
                priority: 75,
                changed_at: frame.sampled_at(),
                programmer_order: 1,
                merge_mode: MergeMode::Ltp,
                fade: false,
                fade_millis: None,
                delay_millis: None,
            },
        )])];
        let static_frame = engine.prepare_static_family_frame(&frame, &baseline);
        let observed = TickSources::prepared(&engine, &frame, &baseline);
        let over_static =
            TickSources::prepared(&engine, &frame, &baseline).over_static(&static_frame);
        assert_eq!(over_static.static_frame.is_some(), !freeze);
        for fixture in [lamp, frozen] {
            for attribute in &attributes {
                assert_eq!(
                    DynamicTickSource::value(&over_static, fixture, attribute),
                    DynamicTickSource::value(&observed, fixture, attribute),
                    "freeze {freeze}: {attribute:?}"
                );
                let evidence = |sources: &TickSources<'_>| {
                    sources
                        .family_evidence(fixture, attribute)
                        .map(|evidence| Arc::as_ptr(evidence) as usize)
                };
                assert_eq!(evidence(&over_static), evidence(&observed));
            }
        }
        assert!(
            over_static.values.get().is_none() || freeze,
            "no second resolution without a Freeze"
        );
        let frozen_intensity = DynamicTickSource::value(&over_static, frozen, &attributes[0]);
        let expected = if freeze { 0.125 } else { 0.9 };
        assert_eq!(
            frozen_intensity.and_then(AttributeValue::normalized),
            Some(expected)
        );
    }
}
