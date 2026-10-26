//! CPU-only deterministic clocks: no event loop, sleep, display, renderer or driver.
use super::*;

#[test]
fn repeated_late_delivery_preserves_sixty_hz_phase_instead_of_accumulating_jitter() {
    let start = Instant::now();
    let mut schedule = LowGpuSchedule::default();
    let cpu = Duration::from_millis(1);
    let adaptive = Duration::from_millis(2);
    let mut due = schedule.succeeded(start, start + cpu, adaptive);
    for frame in 1..=600_u32 {
        let delivered = due + Duration::from_micros(500);
        due = schedule.succeeded(delivered, delivered + cpu, adaptive);
        assert_eq!(due, start + LOW_GPU_PERIOD * (frame + 1));
    }
    // One scheduled submission per grid slot: normal delivery latency never creates extra slots.
    assert_eq!(due.duration_since(start), LOW_GPU_PERIOD * 601);
}

#[test]
fn long_nonintegral_stall_reanchors_and_never_owes_a_catchup_pair() {
    let start = Instant::now();
    let mut schedule = LowGpuSchedule::default();
    schedule.succeeded(start, start, Duration::from_millis(2));
    let stalled = start + Duration::from_millis(5016);
    let due = schedule.succeeded(stalled, stalled, Duration::from_millis(2));
    assert_eq!(due, stalled + LOW_GPU_PERIOD);
    assert!(stalled + LOW_GPU_PERIOD - Duration::from_nanos(1) < due);
    assert_eq!(
        schedule.succeeded(due, due, Duration::from_millis(2)),
        due + LOW_GPU_PERIOD
    );
}

#[test]
fn pending_requests_do_not_advance_phase_or_duplicate_requests() {
    let start = Instant::now();
    let mut schedule = LowGpuSchedule::default();
    let due = schedule.succeeded(start, start, Duration::from_millis(2));
    assert!(schedule.request());
    for _ in 0..20 {
        assert!(!schedule.request());
    }
    assert_eq!(schedule.due, Some(due));
    schedule.delivered();
    assert!(schedule.request());
}

#[test]
fn hidden_and_static_resume_reset_phase_without_losing_an_outstanding_request() {
    let start = Instant::now();
    let mut schedule = LowGpuSchedule::default();
    schedule.succeeded(start, start, Duration::from_millis(2));
    assert!(schedule.request());
    schedule.suspend();
    assert!(!schedule.request());
    schedule.delivered();
    let resumed = start + Duration::from_secs(10);
    assert_eq!(
        schedule.succeeded(resumed, resumed, Duration::from_millis(2)),
        resumed + LOW_GPU_PERIOD
    );
}

#[test]
fn skip_and_error_attempts_preserve_backoff_and_do_not_advance_submission_phase() {
    let start = Instant::now();
    for existing_delay in [Duration::from_millis(2), Duration::from_millis(50)] {
        let mut schedule = LowGpuSchedule::default();
        schedule.succeeded(start, start, Duration::from_millis(2));
        let retried = schedule.retry(start, start + existing_delay);
        assert_eq!(retried, start + existing_delay.max(LOW_GPU_PERIOD));
        assert_eq!(schedule.due, None);
        assert_eq!(
            schedule.succeeded(retried, retried, Duration::from_millis(2)),
            retried + LOW_GPU_PERIOD
        );
    }
}

#[test]
fn slow_render_or_slower_adaptation_cannot_trigger_immediate_catchup() {
    let start = Instant::now();
    let mut schedule = LowGpuSchedule::default();
    let completed = start + Duration::from_millis(40);
    assert_eq!(
        schedule.succeeded(start, completed, Duration::from_millis(2)),
        completed + LOW_GPU_PERIOD
    );
    schedule.suspend();
    assert_eq!(
        schedule.succeeded(start, completed, Duration::from_millis(30)),
        completed + Duration::from_millis(30)
    );
}

#[test]
fn immediate_input_exception_reanchors_the_next_scheduled_frame() {
    let start = Instant::now();
    let mut schedule = LowGpuSchedule::default();
    schedule.succeeded(start, start, Duration::from_millis(2));
    let input = start + Duration::from_millis(3);
    assert_eq!(
        schedule.succeeded(input, input, Duration::from_millis(2)),
        input + LOW_GPU_PERIOD
    );
}

#[test]
fn diagnostics_distinguish_overlapping_reason_predicates_and_reset_without_touching_phase() {
    let start = Instant::now();
    let mut schedule = LowGpuSchedule::default();
    schedule.succeeded(start, start, Duration::from_millis(2));
    let due = schedule.due;
    let first = schedule.take_diagnostics();
    assert_eq!(first.successful_low_gpu_frames, 1);
    assert_eq!(schedule.due, due);
    let late = start + Duration::from_millis(100);
    let completed = late + Duration::from_millis(20);
    schedule.succeeded(late, completed, Duration::from_millis(25));
    let observed = schedule.take_diagnostics();
    assert_eq!(observed.late_admission_reanchors, 1);
    assert_eq!(observed.render_crossed_deadline_reanchors, 1);
    assert_eq!(observed.adaptive_above_period_frames, 1);
    assert_eq!(observed.adaptive_deadline_extensions, 1);
    let mut merged = first;
    merged.merge(observed);
    assert_eq!(merged.successful_low_gpu_frames, 2);
    assert_eq!(
        merged.adaptive_interval_nanos_range,
        Some((2_000_000, 25_000_000))
    );
    assert_eq!(schedule.take_diagnostics().successful_low_gpu_frames, 0);
}

#[test]
fn acquisition_controller_backoff_is_respected_even_when_current_frame_is_fast() {
    let start = Instant::now();
    let mut schedule = LowGpuSchedule::default();
    let completed = start + Duration::from_millis(9);
    assert_eq!(
        schedule.succeeded(start, completed, Duration::from_millis(26)),
        completed + Duration::from_millis(26)
    );
    let counters = schedule.take_diagnostics();
    assert_eq!(counters.adaptive_above_period_frames, 1);
    assert_eq!(counters.adaptive_deadline_extensions, 1);
    assert_eq!(counters.render_crossed_deadline_reanchors, 0);
}

#[test]
fn exact_period_work_keeps_genuine_adaptive_backpressure() {
    let start = Instant::now();
    let mut schedule = LowGpuSchedule::default();
    let completed = start + LOW_GPU_PERIOD;
    assert_eq!(
        schedule.succeeded(start, completed, Duration::from_millis(30)),
        completed + Duration::from_millis(30)
    );
    let counters = schedule.take_diagnostics();
    assert_eq!(counters.adaptive_above_period_frames, 1);
    assert_eq!(counters.adaptive_deadline_extensions, 1);
}
