use super::*;
use chrono::{DateTime, Duration as ChronoDuration, Utc};

#[derive(Clone)]
struct ProjectedAssignment {
    value: TimedValue,
    source: ContributionSourceId,
    sequence_master: Option<ContributionSequenceMaster>,
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
                Some(master) => {
                    ContributionSample::replacing_playback(sampled, master.source(), master.scale())
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
    programmers.start(session, UserId::new());
    let (fixture, fixture_id) = animated_fixture();
    let engine = Engine::new(programmers.clone());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture],
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    engine.set_control_timing([120.0, 90.0, 60.0, 30.0, 15.0], 1_000, 0);
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
fn playback_sample_applies_its_master_to_intensity_and_non_intensity_output() {
    let started = test_time();
    let clock: SharedClock = Arc::new(ManualClock::new(started));
    let programmers = ProgrammerRegistry::with_clock(clock);
    let (fixture, fixture_id) = schema_v2_fixture(&[
        ("intensity", false, false, false, false, false),
        ("tilt", false, false, true, false, false),
    ]);
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
            fixtures: vec![fixture],
            cue_lists: vec![cue_list],
            playbacks: vec![playback],
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    engine.playback().write().go_playback(1).unwrap();

    for (master, expected_intensity, expected_tilt) in [(0.5, 0.1, 0.4), (0.0, 0.0, 0.0)] {
        engine.playback().write().set_master(1, master).unwrap();
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
        assert_dmx(
            frame.universes[&1][1],
            expected_tilt,
            "Playback non-Intensity master",
        );
    }
}

#[test]
fn sampled_playback_intensity_is_mastered_before_htp_arbitration() {
    let started = test_time();
    let clock: SharedClock = Arc::new(ManualClock::new(started));
    let programmers = ProgrammerRegistry::with_clock(clock);
    let (fixture, fixture_id) =
        schema_v2_fixture(&[("intensity", false, false, false, false, false)]);
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
            fixtures: vec![fixture],
            cue_lists: vec![sampled_list, competing_list],
            playbacks: vec![sampled_playback, competing_playback],
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    engine.playback().write().go_playback(1).unwrap();
    engine.playback().write().go_playback(2).unwrap();
    engine.playback().write().set_master(1, 0.5).unwrap();
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
    let clock = Arc::new(ManualClock::new(started));
    let shared_clock: SharedClock = clock.clone();
    let programmers = ProgrammerRegistry::with_clock(shared_clock);
    let (fixture, fixture_id) = schema_v2_fixture(&[("tilt", false, false, false, false, false)]);
    let mut cue_list = test_cue_list(
        "Animated",
        vec![CueChange::set(
            fixture_id,
            AttributeKey("tilt".into()),
            AttributeValue::Normalized(0.0),
        )],
    );
    let mut independent = Cue::new(2.0);
    independent.changes = vec![CueChange::set(
        fixture_id,
        AttributeKey("tilt".into()),
        AttributeValue::Normalized(0.4),
    )];
    cue_list.cues.push(independent);
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture],
            cue_lists: vec![cue_list.clone()],
            playbacks: vec![test_playback(1, cue_list.id), test_playback(2, cue_list.id)],
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    engine.playback().write().go_playback(1).unwrap();
    engine.playback().write().go_playback(2).unwrap();
    clock.advance_millis(1);
    engine.playback().write().go_playback(2).unwrap();
    let assignments = playback_assignments(&engine, started, Some(1));
    let mut animated = FakeAnimatedSource { phase: 0.1 };
    let sampled = animated.sample(&assignments);

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
    programmers.start(session, UserId::new());
    let (fixture, fixture_id) = schema_v2_fixture(&[("tilt", false, false, false, false, false)]);
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
    let live = state.values.into_iter().map(|value| ProjectedAssignment {
        value,
        source: ContributionSourceId::programmer(state.id),
        sequence_master: None,
    });
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture],
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
    programmers.start(session, UserId::new());
    let (fixture, fixture_id) =
        schema_v2_fixture(&[("intensity", false, false, false, false, false)]);
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
    let live = state.values.into_iter().map(|value| ProjectedAssignment {
        value,
        source: ContributionSourceId::programmer(state.id),
        sequence_master: None,
    });
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture],
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    let sampled = FakeAnimatedSource::default().sample(&live.collect::<Vec<_>>());

    let resolved = engine.resolved_values_with_contribution_batches(std::slice::from_ref(&sampled));
    assert_normalized(&resolved, fixture_id, "intensity", 0.9);
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
    engine.playback().write().go_playback(1).unwrap();
    engine.playback().write().go_playback(1).unwrap();
    clock.set(started + ChronoDuration::milliseconds(5_000));
    let sampled = independent_batch(timed_value(fixture_id, "intensity", 0.2, 100, clock.now()));

    engine.resolved_values_with_contribution_batches(std::slice::from_ref(&sampled));
    assert_eq!(mib_state(&engine, fixture_id), MoveInBlackState::Blocked);
    engine.resolved_values();
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
        .into_iter()
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
        .into_iter()
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
            fixtures: vec![fixture],
            cue_lists: vec![cue_list.clone()],
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    engine
        .playback()
        .write()
        .go_at(cue_list.id, started)
        .unwrap();
    let assignments = playback_assignments(&engine, started, None);
    ("Playback/Cue", engine, fixture_id, assignments)
}

fn playback_assignments(
    engine: &Engine,
    at: DateTime<Utc>,
    playback_number: Option<u16>,
) -> Vec<ProjectedAssignment> {
    engine
        .playback()
        .read()
        .contributions_with_context_at(at, |_, _| false)
        .into_iter()
        .filter(|contribution| contribution.source.playback_number == playback_number)
        .map(|contribution| ProjectedAssignment {
            source: ContributionSourceId::playback(contribution.source),
            sequence_master: Some(ContributionSequenceMaster::new(
                contribution.source,
                contribution.sequence_master,
            )),
            value: contribution.value,
        })
        .collect()
}

fn source_engine(started: DateTime<Utc>) -> (Engine, ProgrammerRegistry, SessionId, FixtureId) {
    let clock: SharedClock = Arc::new(ManualClock::new(started));
    let programmers = ProgrammerRegistry::with_clock(clock);
    let session = SessionId::new();
    programmers.start(session, UserId::new());
    let (fixture, fixture_id) = animated_fixture();
    let engine = Engine::new(programmers.clone());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture],
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    (engine, programmers, session, fixture_id)
}

fn animated_fixture() -> (PatchedFixture, FixtureId) {
    schema_v2_fixture(&[
        ("intensity", false, false, false, false, false),
        ("tilt", false, false, false, false, false),
    ])
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
    values: &HashMap<(FixtureId, AttributeKey), AttributeValue>,
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
