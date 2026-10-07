use super::*;
use light_core::{AttributeKey, AttributeValue, FixtureId, MergeMode, TimedValue};
use light_dynamics::{DynamicControl, ScalarSourceResolver, TimedDynamicControl};
use light_engine::{ContributionSample, Engine};
use light_programmer::ProgrammerRegistry;
use std::time::Duration;

fn transports() -> [DynamicSpeedTransport; 5] {
    std::array::from_fn(|index| DynamicSpeedTransport {
        effective_bpm: 60.0 + index as f64,
        phase_origin_millis: 10,
        phase_reference_millis: 20,
        beat_phase: 0.25,
        phase_advancing: index != 4,
    })
}

fn setup(capacity: usize) -> (Engine, DynamicSnapshotPublication, DynamicRuntime) {
    let engine = Engine::new(ProgrammerRegistry::default());
    let publication = DynamicSnapshotPublication::new(engine.snapshot());
    let mut runtime = DynamicRuntime::default();
    publication
        .begin_retained_history(
            &mut runtime,
            &engine.snapshot(),
            NonZeroUsize::new(capacity).unwrap(),
        )
        .unwrap();
    (engine, publication, runtime)
}

fn select(
    engine: &Engine,
    publication: &DynamicSnapshotPublication,
    now: Instant,
) -> RetainedFrameCapture {
    RetainedFrameCapture::select(
        engine.prepare_output_frame(Default::default()),
        publication,
        now,
    )
}

fn accept(
    publication: &DynamicSnapshotPublication,
    runtime: &DynamicRuntime,
    capture: &RetainedFrameCapture,
) {
    let token = capture.retained().expect("test capture must be selected");
    assert!(token.matches(capture));
    publication.retain_accepted_input(runtime, token, &[], &transports(), 40, None);
}

#[test]
fn disabled_retention_keeps_the_frame_owned() {
    let engine = Engine::new(ProgrammerRegistry::default());
    let publication = DynamicSnapshotPublication::new(engine.snapshot());
    let now = Instant::now();
    assert!(!publication.input_capture_due(now));
    assert_eq!(publication.input_capture_cursor(), None);
    let capture = select(&engine, &publication, now);
    assert!(matches!(&capture, RetainedFrameCapture::Owned(_)));
    assert!(capture.retained().is_none());
    assert!(Arc::ptr_eq(&capture.snapshot(), &engine.snapshot()));
}

#[test]
fn accepted_cadence_selects_zero_and_forty_but_not_thirty_nine_milliseconds() {
    let (engine, publication, runtime) = setup(8);
    let now = Instant::now();
    let seed = publication.input_capture_cursor().unwrap();
    let first = select(&engine, &publication, now);
    accept(&publication, &runtime, &first);
    let first_cursor = publication.input_capture_cursor().unwrap();
    assert_ne!(first_cursor, seed);
    assert!(!publication.input_capture_due(now));
    assert!(matches!(
        select(&engine, &publication, now + Duration::from_millis(39)),
        RetainedFrameCapture::Owned(_)
    ));
    let second = select(&engine, &publication, now + Duration::from_millis(40));
    assert!(second.retained().is_some());
    accept(&publication, &runtime, &second);
    let accepted = publication.input_capture_cursor().unwrap();
    accept(&publication, &runtime, &second);
    assert_eq!(
        publication.input_capture_cursor(),
        Some(accepted),
        "duplicate commit cannot add another attempt"
    );
    let entries = publication.input_captures_since(seed).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].from, seed);
    assert_eq!(entries[0].to, entries[1].from);
    assert_eq!(entries[1].to, accepted);
}

#[test]
fn selecting_then_discarding_a_failed_live_capture_does_not_advance_cadence() {
    let (engine, publication, runtime) = setup(4);
    let now = Instant::now();
    let seed = publication.input_capture_cursor().unwrap();
    let failed = select(&engine, &publication, now);
    assert!(failed.retained().is_some());
    drop(failed);
    assert_eq!(publication.input_capture_cursor(), Some(seed));
    assert!(publication.input_captures_since(seed).unwrap().is_empty());
    let retry = select(&engine, &publication, now + Duration::from_millis(1));
    accept(&publication, &runtime, &retry);
    assert!(!publication.input_capture_due(now + Duration::from_millis(40)));
    assert!(publication.input_capture_due(now + Duration::from_millis(41)));
}

struct NoSources;

impl ScalarSourceResolver for NoSources {
    fn current(&self, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        None
    }
    fn preset(&self, _: &str, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        None
    }
}

fn baseline(value: f32) -> ContributionBatch {
    ContributionBatch::new([ContributionSample::independent(TimedValue {
        fixture_id: FixtureId::new(),
        attribute: AttributeKey::intensity(),
        value: AttributeValue::Normalized(value),
        priority: 10,
        changed_at: chrono::DateTime::from_timestamp_millis(100).unwrap(),
        programmer_order: 2,
        merge_mode: MergeMode::Ltp,
        fade: false,
        fade_millis: None,
        delay_millis: None,
    })])
}

#[test]
fn retained_capture_owns_exact_baseline_transports_rate_and_committed_cursors() {
    let (engine, publication, mut runtime) = setup(4);
    let seed = publication.input_capture_cursor().unwrap();
    let capture = select(&engine, &publication, Instant::now());
    let other_frame = engine.prepare_output_frame(Default::default());
    assert!(!capture.retained().unwrap().matches(&other_frame));
    let mut baseline = vec![baseline(0.25)];
    let mut speed = transports();
    let expected_speed = speed;
    let expected_value = baseline[0].samples()[0].value().clone();
    runtime.sample_all_addressed(100, 25, &speed, &NoSources, None);
    let expected_sample = runtime.committed_sample_boundary();
    assert!(expected_sample.is_some());
    let expected_controls = runtime.control_cursor().unwrap();
    let expected_cold = publication.cold_boundary(&runtime).unwrap().from;
    publication.retain_accepted_input(
        &runtime,
        capture.retained().unwrap(),
        &baseline,
        &speed,
        25,
        expected_sample,
    );
    baseline.clear();
    speed[0].effective_bpm = 999.0;
    speed[1].phase_advancing = false;
    runtime
        .apply_recorded_control(TimedDynamicControl {
            at_millis: 101,
            control: DynamicControl::GlobalPause(true),
        })
        .unwrap();
    runtime.sample_all_addressed(102, 5, &speed, &NoSources, None);
    let entry = publication.input_captures_since(seed).unwrap().remove(0);
    assert_eq!(entry.baseline.len(), 1);
    assert_eq!(entry.baseline[0].samples()[0].value(), &expected_value);
    assert_eq!(entry.speed_transports, expected_speed);
    assert_eq!(entry.rate, 25);
    assert_eq!(entry.controls, expected_controls);
    assert_ne!(Some(entry.controls), runtime.control_cursor());
    assert_eq!(entry.cold, expected_cold);
    assert_eq!(entry.live_sample, expected_sample);
    assert_ne!(entry.live_sample, runtime.committed_sample_boundary());
    assert!(std::ptr::eq(entry.frame.as_ref(), &*capture));
    drop(capture);
    assert!(Arc::ptr_eq(&entry.frame.snapshot(), &engine.snapshot()));
}

#[test]
fn eviction_reports_history_loss_without_invalidating_a_retained_arc() {
    let (engine, publication, runtime) = setup(2);
    let seed = publication.input_capture_cursor().unwrap();
    let now = Instant::now();
    let first = select(&engine, &publication, now);
    accept(&publication, &runtime, &first);
    let retained = publication.input_captures_since(seed).unwrap().remove(0);
    for offset in [40, 80] {
        let next = select(&engine, &publication, now + Duration::from_millis(offset));
        accept(&publication, &runtime, &next);
    }
    assert!(matches!(
        publication.input_captures_since(seed),
        Err(ColdGenerationReadError::HistoryLost)
    ));
    let remaining = publication.input_captures_since(retained.to).unwrap();
    assert_eq!(
        remaining.len(),
        2,
        "selected input attempts are not coalesced"
    );
    assert_eq!(remaining[0].from, retained.to);
    assert_eq!(remaining[0].to, remaining[1].from);
    assert!(std::ptr::eq(retained.frame.as_ref(), &*first));
    assert!(Arc::ptr_eq(&retained.frame.snapshot(), &engine.snapshot()));
}

#[test]
fn ordinary_install_resets_input_epoch_and_rejects_old_selection_tokens() {
    let (engine, publication, runtime) = setup(4);
    let now = Instant::now();
    let seed = publication.input_capture_cursor().unwrap();
    let stale = select(&engine, &publication, now);
    // Same snapshot identity is deliberate: the selection token's epoch must detect reset.
    publication.installed(engine.snapshot());
    let restarted = publication.input_capture_cursor().unwrap();
    assert_ne!(restarted.epoch, seed.epoch);
    assert!(matches!(
        publication.input_captures_since(seed),
        Err(ColdGenerationReadError::WrongEpoch)
    ));
    accept(&publication, &runtime, &stale);
    assert_eq!(publication.input_capture_cursor(), Some(restarted));
    assert!(
        publication
            .input_captures_since(restarted)
            .unwrap()
            .is_empty()
    );
    let fresh = select(&engine, &publication, now);
    accept(&publication, &runtime, &fresh);
    assert_eq!(
        publication.input_captures_since(restarted).unwrap().len(),
        1
    );
}

#[test]
fn cold_generation_barrier_is_retained_without_resetting_input_history() {
    let (engine, publication, runtime) = setup(4);
    let now = Instant::now();
    let seed = publication.input_capture_cursor().unwrap();
    let first = select(&engine, &publication, now);
    accept(&publication, &runtime, &first);
    let first_entry = publication.input_captures_since(seed).unwrap().remove(0);
    let stale = select(&engine, &publication, now + Duration::from_millis(40));
    let previous = engine.snapshot();
    let boundary = publication.cold_boundary(&runtime).unwrap();
    let mut destination = (*previous).clone();
    destination.revision += 1;
    let prepared = engine.prepare_snapshot(destination).unwrap();
    let destination = prepared.snapshot_arc();
    let cold_event = boundary.prepare(previous, destination.clone(), &runtime);
    engine.install_prepared_snapshot(prepared);
    publication.installed_with_cold_event(destination.clone(), Some(cold_event));
    accept(&publication, &runtime, &stale);
    assert_eq!(publication.input_capture_cursor(), Some(first_entry.to));
    let second = select(&engine, &publication, now + Duration::from_millis(40));
    accept(&publication, &runtime, &second);
    let entries = publication.input_captures_since(seed).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[1].from, entries[0].to);
    assert_eq!(entries[1].to.epoch, seed.epoch);
    assert_ne!(entries[0].cold, entries[1].cold);
    assert!(Arc::ptr_eq(&entries[1].frame.snapshot(), &destination));
    let events = publication.cold_generations_since(entries[0].cold).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].to, entries[1].cold);
}
