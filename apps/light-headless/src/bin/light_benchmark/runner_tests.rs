use super::*;

#[test]
fn rational_tick_schedule_does_not_accumulate_120_hz_rounding_error() {
    assert_eq!(scheduled_offset(120, 120), Duration::from_secs(1));
    assert_eq!(scheduled_offset(600, 120), Duration::from_secs(5));
    assert_eq!(tick_at(Duration::from_secs(5), 120), 600);
}

#[test]
fn checksum_is_order_independent_for_universe_map_iteration() {
    let frames_a = HashMap::from([(2, [2; 512]), (1, [1; 512])]);
    let frames_b = HashMap::from([(1, [1; 512]), (2, [2; 512])]);
    assert_eq!(checksum(&frames_a, &[]), checksum(&frames_b, &[]));
}

#[test]
fn required_floor_status_distinguishes_failure_from_not_run() {
    assert_eq!(
        required_floor_result_for([(Expectation::LowPowerGoal, false)]),
        None
    );
    assert_eq!(
        required_floor_result_for([(Expectation::RequiredFloor, false)]),
        Some(false)
    );
    assert_eq!(
        required_floor_result_for([(Expectation::RequiredFloor, true)]),
        Some(true)
    );
    assert_eq!(
        required_floor_result_for([(Expectation::InformationalCapacity, false)]),
        None
    );
}

#[test]
fn frame_rate_report_requires_every_measured_second_to_meet_the_floor() {
    let passing = frame_rate_report(&[100, 100], 2, 100, 200, Duration::from_secs_f64(1.991));
    assert_eq!(passing.average_completed_hz, 100.0);
    assert_eq!(passing.minimum_one_second_completed_hz, 100.0);
    assert_eq!(passing.p95_one_second_completed_hz, 100.0);
    assert_eq!(passing.maximum_one_second_completed_hz, 100.0);
    assert_eq!(passing.windows_below_minimum, 0);
    assert_eq!(passing.reporting_target_hz, 44);
    assert_eq!(passing.windows_below_reporting_target, 0);
    assert!(passing.gate_met);

    let dropped = frame_rate_report(&[100, 99], 2, 100, 199, Duration::from_secs_f64(1.991));
    assert_eq!(dropped.average_completed_hz, 99.5);
    assert_eq!(dropped.minimum_one_second_completed_hz, 99.0);
    assert_eq!(dropped.p95_one_second_completed_hz, 100.0);
    assert_eq!(dropped.maximum_one_second_completed_hz, 100.0);
    assert_eq!(dropped.windows_below_minimum, 1);
    assert_eq!(dropped.windows_below_reporting_target, 0);
    assert!(!dropped.gate_met);
}

#[test]
fn frame_rate_report_uses_nearest_rank_for_p95() {
    let windows = [
        40, 50, 60, 70, 80, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101, 102, 103, 104,
    ];
    let report = frame_rate_report(&windows, 20, 40, 1_865, Duration::from_secs(20));
    assert_eq!(report.p95_one_second_completed_hz, 103.0);
    assert_eq!(report.maximum_one_second_completed_hz, 104.0);
}
