use super::*;
use chrono::{DateTime, Duration as ChronoDuration, Utc};

#[derive(Clone)]
struct ProjectedAssignment {
    value: TimedValue,
    source: ContributionSourceId,
    /// The Playback this assignment came from and its Cue master, when it came from one.
    sequence_master: Option<(light_playback::SequenceMasterSource, f32)>,
}

/// Test-only stateful producer. Its input is the ordinary semantic assignment projection owned by
/// Programmer, Preload, or Playback; only its immutable sample crosses into the engine.
#[derive(Default)]
struct FakeAnimatedSource {
    phase: f32,
}

impl FakeAnimatedSource {
    fn sample(&mut self, assignments: &[ProjectedAssignment]) -> ContributionBatch {
        self.phase = (self.phase + 0.2).min(1.0);
        ContributionBatch::new(assignments.iter().map(|assignment| {
            let mut sampled = assignment.value.clone();
            sampled.value = AttributeValue::Normalized(if sampled.attribute.is_intensity() {
                self.phase
            } else {
                1.0 - self.phase
            });
            sampled.merge_mode = merge_mode(&sampled.attribute);
            sampled.fade = false;
            sampled.fade_millis = None;
            sampled.delay_millis = None;
            match assignment.sequence_master {
                // A Playback producer samples its own output parameter: a level carries the
                // Playback's Cue master before it crosses into the engine.
                Some((source, master)) => {
                    if sampled.attribute.is_level()
                        && let Some(level) = sampled.value.normalized()
                    {
                        sampled.value = AttributeValue::Normalized(level * master);
                    }
                    ContributionSample::replacing_playback(sampled, source, 0)
                }
                None => ContributionSample::replacing(sampled, assignment.source.clone()),
            }
        }))
    }
}

struct FakeFixedSource;

impl FakeFixedSource {
    fn sample(animated: &ContributionBatch, priority_delta: i16) -> ContributionBatch {
        ContributionBatch::new(animated.samples().iter().map(|animated| {
            let mut fixed = animated.value().clone();
            fixed.value = AttributeValue::Normalized(if fixed.attribute.is_intensity() {
                0.25
            } else {
                0.9
            });
            fixed.priority += priority_delta;
            fixed.changed_at += ChronoDuration::milliseconds(1);
            ContributionSample::independent(fixed)
        }))
    }
}

#[test]
fn one_stateful_animated_source_samples_programmer_preload_and_cue_projections() {
    let started = test_time();
    let sources = [
        programmer_projection(started),
        preload_projection(started),
        playback_cue_projection(started),
    ];
    let mut animated = FakeAnimatedSource::default();

    for (index, (surface, engine, fixture_id, assignments)) in sources.into_iter().enumerate() {
        let batch = animated.sample(&assignments);
        let phase = (index as f32 + 1.0) * 0.2;

        assert_eq!(batch.len(), 2, "{surface} lost a combined attribute");
        assert!(
            batch
                .samples()
                .iter()
                .zip(&assignments)
                .all(
                    |(sample, assignment)| sample.value().changed_at == assignment.value.changed_at
                ),
            "{surface} relied on a forged change timestamp"
        );
        assert_sample(
            &engine,
            fixture_id,
            std::slice::from_ref(&batch),
            phase,
            1.0 - phase,
            surface,
        );
    }
}

#[test]
fn fixed_samples_use_normal_priority_ltp_and_htp_arbitration() {
    let (surface, engine, fixture_id, assignments) = programmer_projection(test_time());
    let mut animated = FakeAnimatedSource { phase: 0.6 };
    let animated = animated.sample(&assignments);

    let same_priority_fixed = FakeFixedSource::sample(&animated, 0);
    let resolved =
        engine.resolved_values_with_contribution_batches(&[animated.clone(), same_priority_fixed]);
    assert_normalized(&resolved, fixture_id, "intensity", 0.8);
    assert_normalized(&resolved, fixture_id, "tilt", 0.9);

    let lower_priority_fixed = FakeFixedSource::sample(&animated, -1);
    let resolved =
        engine.resolved_values_with_contribution_batches(&[animated.clone(), lower_priority_fixed]);
    assert_normalized(&resolved, fixture_id, "intensity", 0.8);
    assert_normalized(&resolved, fixture_id, "tilt", 0.2);

    let higher_priority_fixed = FakeFixedSource::sample(&animated, 1);
    let stomped = [animated, higher_priority_fixed];
    assert_sample(&engine, fixture_id, &stomped, 0.25, 0.9, surface);
}

#[test]
fn sampled_value_is_the_underlay_for_an_ordinary_programmer_fade() {
    let started = test_time();
    let clock = Arc::new(ManualClock::new(started));
    let shared_clock: SharedClock = clock.clone();
    let programmers = ProgrammerRegistry::with_clock(shared_clock);
    let session = SessionId::new();
    programmers.start(session);
    let (fixture, fixture_id) = animated_fixture();
    let engine = Engine::new(programmers.clone());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    engine.set_control_timing([120.0, 90.0, 60.0, 30.0, 15.0], 1_000, 0, 0);
    let sampled = independent_batch(timed_value(fixture_id, "tilt", 0.2, 100, started));

    clock.advance_millis(1_000);
    programmers.set_faded(
        session,
        fixture_id,
        AttributeKey("tilt".into()),
        AttributeValue::Normalized(1.0),
    );
    clock.advance_millis(500);

    let resolved = engine.resolved_values_with_contribution_batches(std::slice::from_ref(&sampled));
    assert_normalized(&resolved, fixture_id, "tilt", 0.6);
}

#[test]
fn playback_sample_master_scales_intensity_but_never_a_non_level_output() {
    let started = test_time();
    let clock: SharedClock = Arc::new(ManualClock::new(started));
    let programmers = ProgrammerRegistry::with_clock(clock);
    let (fixture, fixture_id) =
        schema_v2_fixture(&[("intensity", false, false), ("tilt", false, false)]);
    let cue_list = test_cue_list(
        "Mastered animation",
        [AttributeKey::intensity(), AttributeKey("tilt".into())]
            .into_iter()
            .map(|attribute| CueChange::set(fixture_id, attribute, AttributeValue::Normalized(0.0)))
            .collect(),
    );
    let playback = test_playback(1, cue_list.id);
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            cue_lists: vec![cue_list].into(),
            playbacks: vec![playback].into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    execute_pool(&engine, 1, PoolPlaybackAction::Go);

    // The Cue master scales level parameters only (2026-10-05); Tilt reaches DMX unmastered.
    for (master, expected_intensity) in [(0.5, 0.1), (0.0, 0.0)] {
        execute_pool(&engine, 1, PoolPlaybackAction::SetVirtualMaster(master));
        let assignments = playback_assignments(&engine, started, Some(1));
        let sampled = FakeAnimatedSource::default().sample(&assignments);
        let resolved =
            engine.resolved_values_with_contribution_batches(std::slice::from_ref(&sampled));
        assert_normalized(&resolved, fixture_id, "intensity", expected_intensity);
        assert_normalized(&resolved, fixture_id, "tilt", 0.8);
        let frame = engine
            .render_with_contribution_batches(
                RenderOptions::default(),
                std::slice::from_ref(&sampled),
            )
            .unwrap();
        assert_dmx(
            frame.universes[&1][0],
            expected_intensity,
            "Playback Intensity master",
        );
        assert_dmx(frame.universes[&1][1], 0.8, "Playback non-Intensity output");
    }
}

#[test]
fn sampled_playback_intensity_is_mastered_before_htp_arbitration() {
    let started = test_time();
    let clock: SharedClock = Arc::new(ManualClock::new(started));
    let programmers = ProgrammerRegistry::with_clock(clock);
    let (fixture, fixture_id) = schema_v2_fixture(&[("intensity", false, false)]);
    let sampled_list = test_cue_list(
        "Sampled",
        vec![CueChange::set(
            fixture_id,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.0),
        )],
    );
    let competing_list = test_cue_list(
        "Competing",
        vec![CueChange::set(
            fixture_id,
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.6),
        )],
    );
    let mut sampled_playback = test_playback(1, sampled_list.id);
    sampled_playback.auto_off = false;
    let mut competing_playback = test_playback(2, competing_list.id);
    competing_playback.auto_off = false;
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            cue_lists: vec![sampled_list, competing_list].into(),
            playbacks: vec![sampled_playback, competing_playback].into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    execute_pool(&engine, 1, PoolPlaybackAction::Go);
    execute_pool(&engine, 2, PoolPlaybackAction::Go);
    execute_pool(&engine, 1, PoolPlaybackAction::SetVirtualMaster(0.5));
    let assignments = playback_assignments(&engine, started, Some(1));
    let sampled = FakeAnimatedSource { phase: 0.6 }.sample(&assignments);

    let resolved = engine.resolved_values_with_contribution_batches(std::slice::from_ref(&sampled));
    assert_normalized(&resolved, fixture_id, "intensity", 0.6);
    let frame = engine
        .render_with_contribution_batches(RenderOptions::default(), std::slice::from_ref(&sampled))
        .unwrap();
    assert_dmx(
        frame.universes[&1][0],
        0.6,
        "HTP after sampled Playback master",
    );
}

#[test]
fn a_sample_replaces_only_its_independent_playback() {
    let started = test_time();
    let clock: SharedClock = Arc::new(ManualClock::new(started));
    let programmers = ProgrammerRegistry::with_clock(clock);
    let (fixture, fixture_id) = schema_v2_fixture(&[("tilt", false, false)]);
    let mut sampled_list = test_cue_list(
        "Sampled",
        vec![CueChange::set(
            fixture_id,
            AttributeKey("tilt".into()),
            AttributeValue::Normalized(0.9),
        )],
    );
    sampled_list.priority = 20;
    let independent_list = test_cue_list(
        "Independent",
        vec![CueChange::set(
            fixture_id,
            AttributeKey("tilt".into()),
            AttributeValue::Normalized(0.4),
        )],
    );
    let mut sampled_playback = test_playback(1, sampled_list.id);
    sampled_playback.auto_off = false;
    let mut independent_playback = test_playback(2, independent_list.id);
    independent_playback.auto_off = false;
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            cue_lists: vec![sampled_list, independent_list].into(),
            playbacks: vec![sampled_playback, independent_playback].into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    execute_pool(&engine, 1, PoolPlaybackAction::Go);
    execute_pool(&engine, 2, PoolPlaybackAction::Go);
    let assignments = playback_assignments(&engine, started, Some(1));
    let sampled = ContributionBatch::new(assignments.into_iter().map(|assignment| {
        let mut value = assignment.value;
        value.value = AttributeValue::Normalized(0.1);
        value.priority = 0;
        let (source, _) = assignment.sequence_master.unwrap();
        ContributionSample::replacing_playback(value, source, 0)
    }));

    assert_normalized(&engine.resolved_values(), fixture_id, "tilt", 0.9);
    let resolved = engine.resolved_values_with_contribution_batches(std::slice::from_ref(&sampled));
    assert_normalized(&resolved, fixture_id, "tilt", 0.4);
}

#[test]
fn live_programmer_sample_does_not_replace_the_same_programmers_preload() {
    let started = test_time();
    let clock = Arc::new(ManualClock::new(started));
    let shared_clock: SharedClock = clock.clone();
    let programmers = ProgrammerRegistry::with_clock(shared_clock);
    let session = SessionId::new();
    programmers.start(session);
    let (fixture, fixture_id) = schema_v2_fixture(&[("tilt", false, false)]);
    programmers.set(
        session,
        fixture_id,
        AttributeKey("tilt".into()),
        AttributeValue::Normalized(0.0),
    );
    clock.advance_millis(1);
    assert!(programmers.arm_preload(session, true));
    programmers.set(
        session,
        fixture_id,
        AttributeKey("tilt".into()),
        AttributeValue::Normalized(0.4),
    );
    assert!(programmers.activate_preload_at(session, clock.now()));
    let state = programmers.active().remove(0);
    let live = state
        .values
        .iter()
        .cloned()
        .map(|value| ProjectedAssignment {
            value,
            source: ContributionSourceId::programmer(state.id),
            sequence_master: None,
        });
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    let mut animated = FakeAnimatedSource { phase: 0.1 };
    let sampled = animated.sample(&live.collect::<Vec<_>>());

    let resolved = engine.resolved_values_with_contribution_batches(std::slice::from_ref(&sampled));
    assert_normalized(&resolved, fixture_id, "tilt", 0.4);
}

#[test]
fn replacing_newer_live_programmer_keeps_older_preload_as_an_htp_competitor() {
    let started = test_time();
    let clock = Arc::new(ManualClock::new(started));
    let shared_clock: SharedClock = clock.clone();
    let programmers = ProgrammerRegistry::with_clock(shared_clock);
    let session = SessionId::new();
    programmers.start(session);
    let (fixture, fixture_id) = schema_v2_fixture(&[("intensity", false, false)]);
    assert!(programmers.arm_preload(session, true));
    programmers.set(
        session,
        fixture_id,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.9),
    );
    assert!(programmers.activate_preload_at(session, started));
    clock.advance_millis(1);
    programmers.set(
        session,
        fixture_id,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.0),
    );
    let state = programmers.active().remove(0);
    let live = state
        .values
        .iter()
        .cloned()
        .map(|value| ProjectedAssignment {
            value,
            source: ContributionSourceId::programmer(state.id),
            sequence_master: None,
        });
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    let sampled = FakeAnimatedSource::default().sample(&live.collect::<Vec<_>>());

    let resolved = engine.resolved_values_with_contribution_batches(std::slice::from_ref(&sampled));
    assert_normalized(&resolved, fixture_id, "intensity", 0.9);
}

#[test]
fn transient_sample_replaces_only_the_named_transient_action() {
    let started = test_time();
    let clock = Arc::new(ManualClock::new(started));
    let shared_clock: SharedClock = clock.clone();
    let programmers = ProgrammerRegistry::with_clock(shared_clock);
    let session = SessionId::new();
    programmers.start(session);
    let (fixture, fixture_id) = animated_fixture();
    programmers
        .set_transient_action(
            session,
            "background".into(),
            [(fixture_id, AttributeKey("tilt".into()), normalized(0.4))],
        )
        .unwrap();
    clock.advance_millis(1);
    programmers
        .set_transient_action(
            session,
            "sampled".into(),
            [(fixture_id, AttributeKey("tilt".into()), normalized(0.9))],
        )
        .unwrap();
    let state = programmers.active().remove(0);
    let original = state
        .transient_values
        .iter()
        .find(|action| action.source == "sampled")
        .unwrap()
        .values[0]
        .clone();
    let sampled = lower_priority_replacement(
        original,
        ContributionSourceId::programmer_transient(state.id, "sampled"),
    );
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();

    assert_normalized(&engine.resolved_values(), fixture_id, "tilt", 0.9);
    let resolved = engine.resolved_values_with_contribution_batches(&[sampled]);
    assert_normalized(&resolved, fixture_id, "tilt", 0.4);
}

#[test]
fn live_group_sample_replaces_only_the_assigned_group() {
    let started = test_time();
    let clock = Arc::new(ManualClock::new(started));
    let shared_clock: SharedClock = clock.clone();
    let (engine, programmers, session, fixture_id) =
        grouped_source_engine(shared_clock, &["background", "sampled"]);
    assert!(programmers.set_group(
        session,
        "background".into(),
        AttributeKey("tilt".into()),
        normalized(0.4),
    ));
    clock.advance_millis(1);
    assert!(programmers.set_group(
        session,
        "sampled".into(),
        AttributeKey("tilt".into()),
        normalized(0.9),
    ));
    let state = programmers.active().remove(0);
    let original = group_programmer_value(&state, "sampled", fixture_id, false);
    let sampled = lower_priority_replacement(
        original,
        ContributionSourceId::programmer_group(state.id, "sampled"),
    );

    assert_normalized(&engine.resolved_values(), fixture_id, "tilt", 0.9);
    let resolved = engine.resolved_values_with_contribution_batches(&[sampled]);
    assert_normalized(&resolved, fixture_id, "tilt", 0.4);
}

#[test]
fn exclusion_only_release_preserves_other_group_and_fixture_sources_and_reveals_the_cue() {
    let clock = Arc::new(ManualClock::new(test_time()));
    let (engine, programmers, session, fixture_id) =
        grouped_source_engine(clock.clone(), &["a", "b"]);
    let attribute = AttributeKey("tilt".into());
    let cue = test_cue_list(
        "Underlay",
        vec![CueChange::set(
            fixture_id,
            attribute.clone(),
            normalized(0.1),
        )],
    );
    let playback = test_playback(1, cue.id);
    let mut snapshot = engine.snapshot().as_ref().clone();
    snapshot.cue_lists = vec![cue].into();
    snapshot.playbacks = vec![playback].into();
    snapshot.revision += 1;
    engine.replace_snapshot(snapshot).unwrap();
    execute_pool(&engine, 1, PoolPlaybackAction::Go);
    programmers.set_group(session, "b".into(), attribute.clone(), normalized(0.4));
    clock.advance_millis(1);
    programmers.set_group(session, "a".into(), attribute.clone(), normalized(0.9));
    clock.advance_millis(1);
    programmers.set(session, fixture_id, attribute.clone(), normalized(0.7));
    let state = programmers.active().remove(0);
    let depth = programmers.undo_depth(session);
    let sources = [
        ContributionSourceId::programmer_group(state.id, "a"),
        ContributionSourceId::programmer(state.id),
        ContributionSourceId::programmer_group(state.id, "b"),
    ];
    for (count, expected) in [(1, 0.7), (2, 0.4), (3, 0.1)] {
        let batch = ContributionBatch::excluding(
            sources[..count]
                .iter()
                .map(|source| (source.clone(), fixture_id, attribute.clone())),
        );
        assert_eq!(batch.len(), 0);
        assert!(!batch.is_empty());
        assert_normalized(
            &engine.resolved_values_with_contribution_batches(std::slice::from_ref(&batch)),
            fixture_id,
            "tilt",
            expected,
        );
        let frame = engine
            .render_with_contribution_batches(
                RenderOptions::default(),
                std::slice::from_ref(&batch),
            )
            .unwrap();
        assert_dmx(
            frame.universes[&1][1],
            expected,
            "source-scoped release underlay",
        );
    }
    assert_normalized(&engine.resolved_values(), fixture_id, "tilt", 0.7);
    assert_eq!(programmers.undo_depth(session), depth);
}

#[test]
fn preload_group_sample_keeps_the_live_group_lane_independent() {
    let started = test_time();
    let clock = Arc::new(ManualClock::new(started));
    let shared_clock: SharedClock = clock.clone();
    let (engine, programmers, session, fixture_id) = grouped_source_engine(shared_clock, &["wash"]);
    assert!(programmers.set_group(
        session,
        "wash".into(),
        AttributeKey("tilt".into()),
        normalized(0.4),
    ));
    clock.advance_millis(1);
    assert!(programmers.set_preload_group(
        session,
        "wash".into(),
        AttributeKey("tilt".into()),
        normalized(0.9),
    ));
    assert!(programmers.activate_preload_at(session, clock.now()));
    let state = programmers.active().remove(0);
    let original = group_programmer_value(&state, "wash", fixture_id, true);
    let sampled = lower_priority_replacement(
        original,
        ContributionSourceId::preload_group(state.id, "wash"),
    );

    assert_normalized(&engine.resolved_values(), fixture_id, "tilt", 0.9);
    let resolved = engine.resolved_values_with_contribution_batches(&[sampled]);
    assert_normalized(&resolved, fixture_id, "tilt", 0.4);
}

#[test]
fn empty_batches_are_equivalent_to_the_original_engine_entry_points() {
    let (engine, programmers, session, fixture_id) = source_engine(test_time());
    programmers.set_many(session, zero_assignments(fixture_id));
    let empty = [ContributionBatch::default()];

    let ordinary_values = engine.resolved_values();
    assert_eq!(
        ordinary_values,
        engine.resolved_values_with_contribution_batches(&[])
    );
    assert_eq!(
        ordinary_values,
        engine.resolved_values_with_contribution_batches(&empty)
    );

    let ordinary_frame = engine.render(RenderOptions::default()).unwrap();
    let extended_frame = engine
        .render_with_contribution_batches(RenderOptions::default(), &empty)
        .unwrap();
    assert_eq!(ordinary_frame.universes, extended_frame.universes);
    assert_eq!(ordinary_frame.patched_slots, extended_frame.patched_slots);
}

#[test]
fn sampled_intensity_participates_in_move_in_black_darkness() {
    let started = test_time();
    let clock = Arc::new(ManualClock::new(started));
    let shared_clock: SharedClock = clock.clone();
    let programmers = ProgrammerRegistry::with_clock(shared_clock);
    let (fixture, fixture_id) = moving_fixture(1, true, 1_000);
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(mib_snapshot(vec![fixture], &[fixture_id]))
        .unwrap();
    execute_pool(&engine, 1, PoolPlaybackAction::Go);
    execute_pool(&engine, 1, PoolPlaybackAction::Go);
    clock.set(started + ChronoDuration::milliseconds(5_000));
    let sampled = independent_batch(timed_value(fixture_id, "intensity", 0.2, 100, clock.now()));

    engine
        .render_with_contribution_batches(RenderOptions::default(), std::slice::from_ref(&sampled))
        .unwrap();
    assert_eq!(mib_state(&engine, fixture_id), MoveInBlackState::Blocked);
    engine.render(RenderOptions::default()).unwrap();
    assert_eq!(mib_state(&engine, fixture_id), MoveInBlackState::Delaying);
}

fn programmer_projection(
    started: DateTime<Utc>,
) -> (&'static str, Engine, FixtureId, Vec<ProjectedAssignment>) {
    let (engine, programmers, session, fixture_id) = source_engine(started);
    programmers.set_many(session, zero_assignments(fixture_id));
    let mut states = programmers.active();
    let state = states.remove(0);
    let source = ContributionSourceId::programmer(state.id);
    let assignments = state
        .values
        .iter()
        .cloned()
        .map(|value| ProjectedAssignment {
            value,
            source: source.clone(),
            sequence_master: None,
        })
        .collect();
    ("Programmer", engine, fixture_id, assignments)
}

fn preload_projection(
    started: DateTime<Utc>,
) -> (&'static str, Engine, FixtureId, Vec<ProjectedAssignment>) {
    let (engine, programmers, session, fixture_id) = source_engine(started);
    assert!(programmers.arm_preload(session, true));
    programmers.set_many(session, zero_assignments(fixture_id));
    assert!(programmers.activate_preload_at(session, started));
    let mut states = programmers.active();
    let state = states.remove(0);
    let source = ContributionSourceId::preload(state.id);
    let assignments = state
        .preload_active
        .iter()
        .cloned()
        .map(|value| ProjectedAssignment {
            value,
            source: source.clone(),
            sequence_master: None,
        })
        .collect();
    ("Preload", engine, fixture_id, assignments)
}

fn playback_cue_projection(
    started: DateTime<Utc>,
) -> (&'static str, Engine, FixtureId, Vec<ProjectedAssignment>) {
    let clock: SharedClock = Arc::new(ManualClock::new(started));
    let programmers = ProgrammerRegistry::with_clock(clock);
    let (fixture, fixture_id) = animated_fixture();
    let cue_list = test_cue_list(
        "Animated projection",
        zero_assignments(fixture_id)
            .map(|(_, attribute, value)| CueChange::set(fixture_id, attribute, value))
            .collect(),
    );
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            cue_lists: vec![cue_list.clone()].into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    execute_cue_list(&engine, cue_list.id, CueListPlaybackAction::GoAt(started));
    let assignments = playback_assignments(&engine, started, None);
    ("Playback/Cue", engine, fixture_id, assignments)
}

fn playback_assignments(
    engine: &Engine,
    at: DateTime<Utc>,
    playback_number: Option<u16>,
) -> Vec<ProjectedAssignment> {
    engine
        .playback_contributions_at(at)
        .into_iter()
        .filter(|contribution| contribution.source.playback_number == playback_number)
        .map(|contribution| ProjectedAssignment {
            source: ContributionSourceId::playback(contribution.source),
            sequence_master: Some((contribution.source, contribution.sequence_master)),
            value: contribution.value,
        })
        .collect()
}

fn source_engine(started: DateTime<Utc>) -> (Engine, ProgrammerRegistry, SessionId, FixtureId) {
    let clock: SharedClock = Arc::new(ManualClock::new(started));
    let programmers = ProgrammerRegistry::with_clock(clock);
    let session = SessionId::new();
    programmers.start(session);
    let (fixture, fixture_id) = animated_fixture();
    let engine = Engine::new(programmers.clone());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    (engine, programmers, session, fixture_id)
}

fn grouped_source_engine(
    clock: SharedClock,
    group_ids: &[&str],
) -> (Engine, ProgrammerRegistry, SessionId, FixtureId) {
    let programmers = ProgrammerRegistry::with_clock(clock);
    let session = SessionId::new();
    programmers.start(session);
    let (fixture, fixture_id) = animated_fixture();
    let groups = group_ids
        .iter()
        .map(|id| GroupDefinition {
            replacement_projections: Default::default(),
            id: (*id).into(),
            name: (*id).into(),
            fixtures: vec![fixture_id],
            ..Default::default()
        })
        .collect::<Vec<_>>();
    let engine = Engine::new(programmers.clone());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            groups: groups.into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    (engine, programmers, session, fixture_id)
}

fn group_programmer_value(
    state: &light_programmer::ProgrammerState,
    group_id: &str,
    fixture_id: FixtureId,
    preload: bool,
) -> TimedValue {
    let groups = if preload {
        &state.preload_group_active
    } else {
        &state.group_values
    };
    let scoped = &groups[group_id][&AttributeKey("tilt".into())];
    TimedValue {
        fixture_id,
        attribute: AttributeKey("tilt".into()),
        value: scoped.value.clone(),
        priority: state.priority,
        changed_at: scoped.changed_at,
        programmer_order: scoped.programmer_order,
        merge_mode: MergeMode::Ltp,
        fade: false,
        fade_millis: None,
        delay_millis: None,
    }
}

fn lower_priority_replacement(
    mut value: TimedValue,
    source: ContributionSourceId,
) -> ContributionBatch {
    value.priority -= 1;
    value.value = normalized(0.1);
    ContributionBatch::new([ContributionSample::replacing(value, source)])
}

fn normalized(value: f32) -> AttributeValue {
    AttributeValue::Normalized(value)
}

fn animated_fixture() -> (PatchedFixture, FixtureId) {
    schema_v2_fixture(&[("intensity", false, false), ("tilt", false, false)])
}

fn zero_assignments(
    fixture_id: FixtureId,
) -> impl Iterator<Item = (FixtureId, AttributeKey, AttributeValue)> {
    [AttributeKey::intensity(), AttributeKey("tilt".into())]
        .into_iter()
        .map(move |attribute| (fixture_id, attribute, AttributeValue::Normalized(0.0)))
}

fn timed_value(
    fixture_id: FixtureId,
    attribute: &str,
    value: f32,
    priority: i16,
    changed_at: DateTime<Utc>,
) -> TimedValue {
    let attribute = AttributeKey(attribute.into());
    TimedValue {
        fixture_id,
        merge_mode: merge_mode(&attribute),
        attribute,
        value: AttributeValue::Normalized(value),
        priority,
        changed_at,
        programmer_order: 0,
        fade: false,
        fade_millis: None,
        delay_millis: None,
    }
}

fn independent_batch(value: TimedValue) -> ContributionBatch {
    ContributionBatch::new([ContributionSample::independent(value)])
}

fn merge_mode(attribute: &AttributeKey) -> MergeMode {
    if attribute.is_intensity() {
        MergeMode::Htp
    } else {
        MergeMode::Ltp
    }
}

fn assert_sample(
    engine: &Engine,
    fixture_id: FixtureId,
    batches: &[ContributionBatch],
    intensity: f32,
    tilt: f32,
    surface: &str,
) {
    assert_eq!(
        engine.resolved_values(),
        engine.resolved_values_with_contribution_batches(&[]),
        "{surface} changed without a sampled batch"
    );
    let resolved = engine.resolved_values_with_contribution_batches(batches);
    assert_normalized(&resolved, fixture_id, "intensity", intensity);
    assert_normalized(&resolved, fixture_id, "tilt", tilt);

    let frame = engine
        .render_with_contribution_batches(RenderOptions::default(), batches)
        .unwrap();
    assert_dmx(frame.universes[&1][0], intensity, surface);
    assert_dmx(frame.universes[&1][1], tilt, surface);
}

fn assert_normalized(
    values: &crate::ResolvedValues,
    fixture_id: FixtureId,
    attribute: &str,
    expected: f32,
) {
    let actual = values[&(fixture_id, AttributeKey(attribute.into()))]
        .normalized()
        .unwrap();
    assert!((actual - expected).abs() < 0.0001, "{attribute}: {actual}");
}

fn assert_dmx(actual: u8, expected: f32, surface: &str) {
    let expected = (expected * f32::from(u8::MAX)).round() as i16;
    assert!(
        (i16::from(actual) - expected).abs() <= 1,
        "{surface}: DMX {actual} did not project {expected}"
    );
}

fn mib_state(engine: &Engine, fixture_id: FixtureId) -> MoveInBlackState {
    engine
        .move_in_black_runtime()
        .into_iter()
        .find(|runtime| runtime.fixture_id == fixture_id)
        .unwrap()
        .state
}

fn test_time() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 7, 19, 12, 0, 0).unwrap()
}

#[test]
fn preload_release_go_reveals_underlay_without_erasing_the_older_programmer() {
    let clock = Arc::new(ManualClock::new(test_time()));
    let (engine, programmers, session, fixture) =
        grouped_source_engine(clock.clone(), &["underlay"]);
    let tilt = AttributeKey("tilt".into());
    programmers.set_group(session, "underlay".into(), tilt.clone(), normalized(0.2));
    programmers.set_many(session, [(fixture, tilt.clone(), normalized(0.5))]);
    programmers.arm_preload(session, true);
    programmers.set_many(session, [(fixture, tilt.clone(), normalized(0.8))]);
    programmers.activate_preload(session);
    assert_normalized(&engine.resolved_values(), fixture, "tilt", 0.8);
    programmers.arm_preload(session, true);
    assert!(programmers.apply_release_values(
        session,
        &[light_programmer::ReleaseProgrammerFixtureValue {
            fixture_id: fixture,
            attribute: tilt.clone()
        }],
        &[]
    ));
    assert_normalized(&engine.resolved_values(), fixture, "tilt", 0.8);
    programmers.activate_preload(session);
    assert_normalized(&engine.resolved_values(), fixture, "tilt", 0.2);
    let rendered = engine.render(RenderOptions::default()).unwrap();
    assert_dmx(rendered.universes[&1][1], 0.2, "Preload Release Go");
    assert_eq!(
        programmers.get(session).unwrap().values[0].value,
        normalized(0.5)
    );
    assert!(programmers.undo(session));
    assert_normalized(&engine.resolved_values(), fixture, "tilt", 0.8);
    assert!(programmers.redo(session));
    assert_normalized(&engine.resolved_values(), fixture, "tilt", 0.2);
    assert!(programmers.release_preload(session));
    assert_normalized(&engine.resolved_values(), fixture, "tilt", 0.5);
}

#[test]
fn preload_release_keeps_newer_edits_and_does_not_let_a_released_sample_replace_them() {
    let clock = Arc::new(ManualClock::new(test_time()));
    let (engine, programmers, session, fixture) =
        grouped_source_engine(clock.clone(), &["underlay"]);
    let tilt = AttributeKey("tilt".into());
    programmers.set_group(session, "underlay".into(), tilt.clone(), normalized(0.2));
    programmers.set_many(session, [(fixture, tilt.clone(), normalized(0.5))]);
    let state = programmers.get(session).unwrap();
    let sample = ContributionBatch::new([ContributionSample::replacing(
        state.values[0].clone(),
        ContributionSourceId::programmer(state.id),
    )]);
    programmers.arm_preload(session, true);
    programmers.apply_release_values(
        session,
        &[light_programmer::ReleaseProgrammerFixtureValue {
            fixture_id: fixture,
            attribute: tilt.clone(),
        }],
        &[],
    );
    programmers.arm_preload(session, false);
    // All authored timestamps are identical. Only the monotonic edit order distinguishes them.
    programmers.set_many(session, [(fixture, tilt.clone(), normalized(0.7))]);
    clock.advance_millis(5_000);
    programmers.activate_preload(session);
    assert_normalized(
        &engine.resolved_values_with_contribution_batches(std::slice::from_ref(&sample)),
        fixture,
        "tilt",
        0.7,
    );
    // Clearing the newer value leaves the retained Preload Release active.
    programmers.release_fixture_attribute(session, fixture, &tilt);
    assert_normalized(
        &engine.resolved_values_with_contribution_batches(&[sample]),
        fixture,
        "tilt",
        0.2,
    );
}

#[test]
fn group_preload_release_preserves_other_groups_direct_exceptions_and_updates_membership() {
    let clock: SharedClock = Arc::new(ManualClock::new(test_time()));
    let (engine, programmers, session, fixture) = grouped_source_engine(clock, &["a", "b"]);
    let tilt = AttributeKey("tilt".into());
    programmers.set_group(session, "b".into(), tilt.clone(), normalized(0.3));
    programmers.set_group(session, "a".into(), tilt.clone(), normalized(0.6));
    programmers.arm_preload(session, true);
    programmers.apply_release_values(
        session,
        &[],
        &[light_programmer::ReleaseProgrammerGroupValue {
            group_id: "a".into(),
            attribute: tilt.clone(),
        }],
    );
    programmers.activate_preload(session);
    assert_normalized(&engine.resolved_values(), fixture, "tilt", 0.3);
    programmers.set_many(session, [(fixture, tilt.clone(), normalized(0.9))]);
    assert_normalized(&engine.resolved_values(), fixture, "tilt", 0.9);
    programmers.release_fixture_attribute(session, fixture, &tilt);
    assert_normalized(&engine.resolved_values(), fixture, "tilt", 0.3);
    // Remove the competing Group, keeping the released Group's old value suppressed.
    let mut snapshot = engine.snapshot().as_ref().clone();
    let mut groups = snapshot.groups.as_ref().clone();
    groups
        .iter_mut()
        .find(|group| group.id == "b")
        .unwrap()
        .fixtures
        .clear();
    snapshot.groups = Arc::new(groups);
    snapshot.revision += 1;
    engine.replace_snapshot(snapshot).unwrap();
    assert!(
        !engine
            .resolved_values()
            .contains_key(&(fixture, tilt.clone()))
    );
    programmers.release_preload(session);
    assert_normalized(&engine.resolved_values(), fixture, "tilt", 0.6);
}

#[test]
fn active_preload_release_filters_older_fixat_sources_before_dynamic_reconciliation() {
    let (engine, programmers, session, fixture) = source_engine(test_time());
    let tilt = AttributeKey("tilt".into());
    let set = |value| light_programmer::DynamicProgrammerValueMutation::Set {
        fixture_id: fixture,
        attribute: tilt.clone(),
        value: light_dynamics::DynamicSemanticValue::FixAt {
            value,
            timing: Default::default(),
        },
    };
    programmers.apply_dynamic_values(session, &[set(0.6)], None);
    assert_eq!(engine.dynamic_programmer_values().len(), 1);
    programmers.arm_preload(session, true);
    programmers.apply_release_values(
        session,
        &[light_programmer::ReleaseProgrammerFixtureValue {
            fixture_id: fixture,
            attribute: tilt.clone(),
        }],
        &[],
    );
    programmers.activate_preload(session);
    let released = engine.dynamic_programmer_values();
    assert_eq!(released.len(), 1);
    assert!(matches!(
        released[0].2.value,
        light_dynamics::DynamicSemanticValue::Release
    ));
    programmers.apply_dynamic_values(session, &[set(0.9)], None);
    assert_eq!(engine.dynamic_programmer_values().len(), 2);
    programmers.release_preload(session);
    assert_eq!(engine.dynamic_programmer_values().len(), 1);
}

#[test]
fn captured_dynamic_tuple_and_source_row_share_one_arc_memo_boundary() {
    let (engine, programmers, session, fixture) = source_engine(test_time());
    let attribute = AttributeKey("focus".into());
    let set = |value| light_programmer::DynamicProgrammerValueMutation::Set {
        fixture_id: fixture,
        attribute: attribute.clone(),
        value: light_dynamics::DynamicSemanticValue::FixAt {
            value,
            timing: Default::default(),
        },
    };
    programmers.apply_dynamic_values(session, &[set(0.3)], None);
    let first_capture = programmers.capture_output_sources();
    let (first_values, first_rows) = engine
        .captured_dynamic_programmer_values_from_sources(first_capture.normal_dynamics.clone());
    let (same_values, same_rows) = engine
        .captured_dynamic_programmer_values_from_sources(first_capture.normal_dynamics.clone());
    assert!(Arc::ptr_eq(&first_values, &same_values));
    assert!(Arc::ptr_eq(&first_rows, &same_rows));
    assert_eq!(first_values.len(), 1);
    assert_eq!(first_rows.len(), first_values.len());
    assert_eq!(
        first_rows[0].source,
        ContributionSourceId::programmer(first_capture.identity.unwrap())
    );
    assert_eq!(
        first_rows[0].programmer_order,
        first_values[0].2.programmer_order
    );
    assert_eq!(
        first_rows[0].stamp.unwrap().programmer_order,
        first_values[0].2.programmer_order
    );

    programmers.apply_dynamic_values(session, &[set(0.7)], None);
    let next_capture = programmers.capture_output_sources();
    let (next_values, next_rows) = engine
        .captured_dynamic_programmer_values_from_sources(next_capture.normal_dynamics.clone());
    assert!(!Arc::ptr_eq(&first_values, &next_values));
    assert!(!Arc::ptr_eq(&first_rows, &next_rows));
    assert_eq!(
        next_rows[0].programmer_order,
        next_values[0].2.programmer_order
    );
    assert!(
        next_rows[0].programmer_order > first_rows[0].programmer_order,
        "the prior capture must retain its original authored row"
    );
}

#[test]
fn restored_release_orders_cannot_suppress_the_next_operator_edit() {
    let (engine, programmers, session, fixture) = source_engine(test_time());
    let tilt = AttributeKey("tilt".into());
    programmers.arm_preload(session, true);
    programmers.apply_release_values(
        session,
        &[light_programmer::ReleaseProgrammerFixtureValue {
            fixture_id: fixture,
            attribute: tilt.clone(),
        }],
        &[],
    );
    programmers.activate_preload(session);
    let saved = serde_json::to_vec(&programmers.get(session).unwrap()).unwrap();
    let restored = ProgrammerRegistry::with_clock(Arc::new(ManualClock::new(test_time())));
    restored.start(session);
    restored.restore(serde_json::from_slice(&saved).unwrap());
    let engine_after_restore = Engine::new(restored.clone());
    engine_after_restore
        .replace_snapshot(engine.snapshot().as_ref().clone())
        .unwrap();
    restored.set_many(session, [(fixture, tilt.clone(), normalized(0.8))]);
    assert_normalized(
        &engine_after_restore.resolved_values(),
        fixture,
        "tilt",
        0.8,
    );
}

#[test]
fn preload_release_keeps_linked_dynamic_on_off_lifetimes_independent() {
    let (engine, programmers, session, fixture) = source_engine(test_time());
    let tilt = AttributeKey("tilt".into());
    programmers.apply_dynamic_values(
        session,
        &[light_programmer::DynamicProgrammerValueMutation::Set {
            fixture_id: fixture,
            attribute: tilt.clone(),
            value: light_dynamics::DynamicSemanticValue::DynamicOff {
                instance_link: uuid::Uuid::new_v4(),
                timing: Default::default(),
            },
        }],
        None,
    );
    programmers.arm_preload(session, true);
    programmers.apply_release_values(
        session,
        &[light_programmer::ReleaseProgrammerFixtureValue {
            fixture_id: fixture,
            attribute: tilt,
        }],
        &[],
    );
    programmers.activate_preload(session);
    let values = engine.dynamic_programmer_values();
    assert_eq!(values.len(), 2);
    assert!(values.iter().any(|(_, _, value)| matches!(
        value.value,
        light_dynamics::DynamicSemanticValue::DynamicOff { .. }
    )));
}
