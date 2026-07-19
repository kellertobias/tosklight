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
}
