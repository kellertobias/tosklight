use super::*;

#[test]
fn one_hz_admits_initial_due_frame_and_skips_between_without_consuming_busy_due() {
    let mut gate = TimingAdmission::new(TimingCadence::OneHz, false);
    assert!(gate.begin_at(Duration::ZERO, true, false));
    for millis in 1..1000 {
        assert!(!gate.begin_at(Duration::from_millis(millis), true, false));
    }
    assert!(!gate.begin_at(Duration::from_secs(1), true, true));
    assert!(gate.begin_at(Duration::from_secs(1), true, false));
    let coverage = gate.coverage(1, 1);
    assert_eq!(coverage.frames_admitted, 2);
    assert_eq!(coverage.cadence_skips, 999);
    assert_eq!(coverage.busy_skips, 1);
    assert_eq!(coverage.completed_readbacks, 1);
}

#[test]
fn non_diagnostic_and_explicit_full_keep_every_available_frame_policy() {
    for cadence in [TimingCadence::OneHz, TimingCadence::Full] {
        let mut gate = TimingAdmission::new(cadence, cadence == TimingCadence::Full);
        for millis in 0..100 {
            assert!(gate.begin_at(Duration::from_millis(millis), false, false));
            if cadence == TimingCadence::Full {
                assert!(gate.begin_at(Duration::from_millis(millis), true, false));
            }
        }
        assert!(!gate.begin_at(Duration::from_secs(1), false, true));
    }
}

#[test]
fn quality_changes_do_not_consume_diagnostic_due_or_catch_up_missed_samples() {
    let mut gate = TimingAdmission::new(TimingCadence::OneHz, false);
    assert!(gate.begin_at(Duration::ZERO, true, false));
    assert!(gate.begin_at(Duration::from_millis(500), false, false));
    assert!(!gate.begin_at(Duration::from_millis(501), true, false));
    assert!(gate.begin_at(Duration::from_secs(30), true, false));
    assert!(!gate.begin_at(Duration::from_secs(30), true, false));
}

#[test]
fn cadence_configuration_has_only_two_explicit_values() {
    assert_eq!(TimingCadence::parse("full"), Ok(TimingCadence::Full));
    assert_eq!(TimingCadence::parse("1hz"), Ok(TimingCadence::OneHz));
    for value in ["", "0", "60", "FULL", " 1hz", "off"] {
        assert!(TimingCadence::parse(value).is_err());
    }
}

#[test]
fn explicit_override_requires_benchmark_and_invalid_requested_values_fail() {
    use std::ffi::OsStr;
    assert_eq!(
        configured_cadence(false, Some(OsStr::new("full"))).unwrap(),
        None
    );
    assert_eq!(
        configured_cadence(false, Some(OsStr::new("invalid"))).unwrap(),
        None
    );
    assert_eq!(configured_cadence(true, None).unwrap(), None);
    assert_eq!(
        configured_cadence(true, Some(OsStr::new("full"))).unwrap(),
        Some(TimingCadence::Full)
    );
    assert!(
        configured_cadence(true, Some(OsStr::new("invalid")))
            .unwrap_err()
            .contains(CADENCE_ENV)
    );
}
