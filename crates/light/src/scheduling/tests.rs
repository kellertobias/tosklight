use std::{collections::BTreeSet, sync::Mutex, time::Duration};

use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};

use super::*;

fn utc(value: &str) -> DateTime<Utc> {
    value.parse().unwrap()
}

fn local(value: &str) -> NaiveDateTime {
    value.parse().unwrap()
}

fn time(hour: u32, minute: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(hour, minute, 0).unwrap()
}

fn playback_target() -> ScheduleTarget {
    ScheduleTarget::Playback {
        page: 1,
        slot: 2,
        playback_number: 17,
        action: ScheduledPlaybackAction::Go,
        master_transition: None,
    }
}

fn schedule(trigger: ScheduleTrigger) -> ScheduleDefinition {
    ScheduleDefinition {
        id: ScheduleId(uuid::Uuid::from_u128(7)),
        name: "House open".into(),
        enabled: true,
        trigger,
        target: playback_target(),
    }
}

fn calendar(rule: CalendarRule) -> ScheduleDefinition {
    schedule(ScheduleTrigger::Calendar { rule })
}

#[test]
fn interval_preview_uses_exact_anchor_multiples_without_drift() {
    let recurrence = ScheduleRecurrence::new("UTC").unwrap();
    let enabled_at = utc("2026-01-01T10:00:00Z");
    let definition = schedule(ScheduleTrigger::Interval {
        every_seconds: 300,
        enabled_at,
    });
    let preview = recurrence
        .preview(&definition, utc("2026-01-01T10:04:59Z"), 3)
        .unwrap();
    assert_eq!(
        preview
            .iter()
            .map(|occurrence| occurrence.scheduled_at)
            .collect::<Vec<_>>(),
        [
            utc("2026-01-01T10:05:00Z"),
            utc("2026-01-01T10:10:00Z"),
            utc("2026-01-01T10:15:00Z"),
        ]
    );
    assert_eq!(
        preview[0].identity.key,
        ScheduleOccurrenceKey::Interval {
            enabled_at,
            ordinal: 1
        }
    );
    assert_eq!(
        recurrence
            .next_occurrence(&definition, utc("2026-01-01T10:05:00Z"))
            .unwrap()
            .unwrap()
            .scheduled_at,
        utc("2026-01-01T10:10:00Z"),
        "an occurrence duration or failure cannot move the persisted anchor"
    );
}

#[test]
fn interval_skip_is_bounded_arithmetic_and_never_enumerates_a_backlog() {
    let recurrence = ScheduleRecurrence::new("UTC").unwrap();
    let definition = schedule(ScheduleTrigger::Interval {
        every_seconds: 60,
        enabled_at: utc("2020-01-01T00:00:00Z"),
    });
    let skipped = recurrence
        .skip_interval_to_future(&definition, utc("2026-01-01T00:00:00Z"))
        .unwrap();
    assert_eq!(skipped.skipped_occurrences, 3_156_480);
    assert_eq!(
        skipped.next_occurrence.scheduled_at,
        utc("2026-01-01T00:01:00Z")
    );
    assert_eq!(
        skipped.next_occurrence.identity.key,
        ScheduleOccurrenceKey::Interval {
            enabled_at: utc("2020-01-01T00:00:00Z"),
            ordinal: 3_156_481,
        }
    );
}

#[test]
fn interval_rejects_flooding_and_preview_limits() {
    let recurrence = ScheduleRecurrence::new("UTC").unwrap();
    let too_fast = schedule(ScheduleTrigger::Interval {
        every_seconds: 59,
        enabled_at: utc("2026-01-01T00:00:00Z"),
    });
    assert_eq!(
        recurrence
            .validate(&too_fast, utc("2026-01-01T00:00:00Z"), false)
            .unwrap_err()
            .field,
        "trigger.every_seconds"
    );
    let valid = schedule(ScheduleTrigger::Interval {
        every_seconds: 60,
        enabled_at: utc("2026-01-01T00:00:00Z"),
    });
    assert_eq!(
        recurrence
            .preview(&valid, utc("2026-01-01T00:00:00Z"), 101)
            .unwrap_err()
            .field,
        "limit"
    );
}

#[test]
fn weekly_monday_at_fourteen_uses_the_authoritative_zone() {
    let recurrence = ScheduleRecurrence::new("Europe/Berlin").unwrap();
    let definition = calendar(CalendarRule::Weekly {
        weekdays: BTreeSet::from([ScheduleWeekday::Monday]),
        at: time(14, 0),
    });
    let preview = recurrence
        .preview(&definition, utc("2026-07-24T12:00:00Z"), 2)
        .unwrap();
    assert_eq!(
        preview
            .iter()
            .map(|occurrence| occurrence.local)
            .collect::<Vec<_>>(),
        [local("2026-07-27T14:00:00"), local("2026-08-03T14:00:00")]
    );
    assert_eq!(preview[0].scheduled_at, utc("2026-07-27T12:00:00Z"));
}

#[test]
fn monthly_first_day_and_first_monday_cross_calendar_boundaries() {
    let recurrence = ScheduleRecurrence::new("UTC").unwrap();
    let first_day = calendar(CalendarRule::MonthlyDay {
        day: 1,
        at: time(9, 0),
    });
    assert_eq!(
        recurrence
            .preview(&first_day, utc("2026-01-31T23:00:00Z"), 3)
            .unwrap()
            .iter()
            .map(|value| value.local)
            .collect::<Vec<_>>(),
        [
            local("2026-02-01T09:00:00"),
            local("2026-03-01T09:00:00"),
            local("2026-04-01T09:00:00"),
        ]
    );

    let first_monday = calendar(CalendarRule::MonthlyWeekday {
        ordinal: MonthWeekOrdinal::First,
        weekday: ScheduleWeekday::Monday,
        at: time(14, 0),
    });
    assert_eq!(
        recurrence
            .preview(&first_monday, utc("2026-01-01T00:00:00Z"), 3)
            .unwrap()
            .iter()
            .map(|value| value.local)
            .collect::<Vec<_>>(),
        [
            local("2026-01-05T14:00:00"),
            local("2026-02-02T14:00:00"),
            local("2026-03-02T14:00:00"),
        ]
    );
}

#[test]
fn monthly_last_weekday_and_leap_day_are_supported() {
    let recurrence = ScheduleRecurrence::new("UTC").unwrap();
    let last_monday = calendar(CalendarRule::MonthlyWeekday {
        ordinal: MonthWeekOrdinal::Last,
        weekday: ScheduleWeekday::Monday,
        at: time(8, 0),
    });
    assert_eq!(
        recurrence
            .preview(&last_monday, utc("2024-01-01T00:00:00Z"), 2)
            .unwrap()
            .iter()
            .map(|value| value.local)
            .collect::<Vec<_>>(),
        [local("2024-01-29T08:00:00"), local("2024-02-26T08:00:00")]
    );

    let leap_day = calendar(CalendarRule::Expression {
        expression: CalendarExpression::parse("0 12 29 2 *").unwrap(),
    });
    assert_eq!(
        recurrence
            .next_occurrence(&leap_day, utc("2025-01-01T00:00:00Z"))
            .unwrap()
            .unwrap()
            .local,
        local("2028-02-29T12:00:00")
    );
}

#[test]
fn every_second_day_uses_its_typed_anchor_across_months_and_leap_years() {
    let recurrence = ScheduleRecurrence::new("UTC").unwrap();
    let definition = calendar(CalendarRule::EveryNDays {
        every_days: 2,
        anchor: NaiveDate::from_ymd_opt(2024, 2, 27).unwrap(),
        at: time(10, 0),
    });
    assert_eq!(
        recurrence
            .preview(&definition, utc("2024-02-27T10:00:00Z"), 4)
            .unwrap()
            .iter()
            .map(|value| value.local)
            .collect::<Vec<_>>(),
        [
            local("2024-02-29T10:00:00"),
            local("2024-03-02T10:00:00"),
            local("2024-03-04T10:00:00"),
            local("2024-03-06T10:00:00"),
        ]
    );
}

#[test]
fn five_field_expression_accepts_lists_ranges_steps_and_ordinal_weekdays() {
    let recurrence = ScheduleRecurrence::new("UTC").unwrap();
    let stepped = calendar(CalendarRule::Expression {
        expression: CalendarExpression::parse("0,30 8-10/2 * 1,2 1-5").unwrap(),
    });
    assert_eq!(
        recurrence
            .preview(&stepped, utc("2026-01-04T00:00:00Z"), 5)
            .unwrap()
            .iter()
            .map(|value| value.local)
            .collect::<Vec<_>>(),
        [
            local("2026-01-05T08:00:00"),
            local("2026-01-05T08:30:00"),
            local("2026-01-05T10:00:00"),
            local("2026-01-05T10:30:00"),
            local("2026-01-06T08:00:00"),
        ]
    );

    let first_monday = calendar(CalendarRule::Expression {
        expression: CalendarExpression::parse("0 14 * * 1#1").unwrap(),
    });
    assert_eq!(
        recurrence
            .next_occurrence(&first_monday, utc("2026-01-06T00:00:00Z"))
            .unwrap()
            .unwrap()
            .local,
        local("2026-02-02T14:00:00")
    );
}

#[test]
fn expression_rejects_unsupported_ambiguous_and_impossible_forms_by_field() {
    for (value, field) in [
        ("0 14 * *", "expression"),
        ("0 0 0 * * *", "expression"),
        ("0 14 1 * 1", "weekday"),
        ("0 14 * JAN *", "month"),
        ("0 24 * * *", "hour"),
        ("0 0 31 2 *", "day-of-month"),
        ("0 0 * * 1#5", "weekday"),
        ("0 0 * * 1W", "weekday"),
        ("*/0 * * * *", "minute"),
        ("0 0 * * 1,2#1", "weekday"),
        ("0 0 * * ?", "weekday"),
    ] {
        assert_eq!(
            CalendarExpression::parse(value).unwrap_err().field,
            field,
            "{value}"
        );
    }
}

#[test]
fn one_time_includes_seconds_disables_past_activation_and_allows_disabled_history() {
    let recurrence = ScheduleRecurrence::new("Europe/Berlin").unwrap();
    let mut definition = schedule(ScheduleTrigger::OneTime {
        at: local("2026-12-31T23:59:59"),
    });
    let occurrence = recurrence
        .next_occurrence(&definition, utc("2026-12-31T22:59:58Z"))
        .unwrap()
        .unwrap();
    assert_eq!(occurrence.scheduled_at, utc("2026-12-31T22:59:59Z"));
    assert_eq!(
        occurrence.identity.key,
        ScheduleOccurrenceKey::OneTime {
            local: local("2026-12-31T23:59:59")
        }
    );

    assert_eq!(
        recurrence
            .validate(&definition, utc("2026-12-31T23:00:00Z"), false)
            .unwrap_err()
            .field,
        "trigger.at"
    );
    definition.enabled = false;
    recurrence
        .validate(&definition, utc("2027-01-01T00:00:00Z"), false)
        .unwrap();
}

#[test]
fn nonexistent_one_time_is_invalid_even_when_disabled() {
    let recurrence = ScheduleRecurrence::new("Europe/Berlin").unwrap();
    let mut definition = schedule(ScheduleTrigger::OneTime {
        at: local("2026-03-29T02:30:00"),
    });
    definition.enabled = false;
    assert_eq!(
        recurrence
            .validate(&definition, utc("2026-01-01T00:00:00Z"), false)
            .unwrap_err()
            .field,
        "trigger.at"
    );
}

#[test]
fn daylight_saving_gap_is_skipped_not_shifted_or_replayed() {
    let recurrence = ScheduleRecurrence::new("Europe/Berlin").unwrap();
    let definition = calendar(CalendarRule::Expression {
        expression: CalendarExpression::parse("30 2 * * *").unwrap(),
    });
    let preview = recurrence
        .preview(&definition, utc("2026-03-28T22:00:00Z"), 2)
        .unwrap();
    assert_eq!(
        preview.iter().map(|value| value.local).collect::<Vec<_>>(),
        [local("2026-03-30T02:30:00"), local("2026-03-31T02:30:00")]
    );
    assert_eq!(preview[0].scheduled_at, utc("2026-03-30T00:30:00Z"));
}

#[test]
fn daylight_saving_fold_uses_one_local_identity_and_never_runs_twice() {
    let recurrence = ScheduleRecurrence::new("Europe/Berlin").unwrap();
    let definition = calendar(CalendarRule::Expression {
        expression: CalendarExpression::parse("30 2 * * *").unwrap(),
    });
    let occurrence = recurrence
        .next_occurrence(&definition, utc("2026-10-24T23:00:00Z"))
        .unwrap()
        .unwrap();
    assert_eq!(occurrence.local, local("2026-10-25T02:30:00"));
    assert_eq!(occurrence.scheduled_at, utc("2026-10-25T00:30:00Z"));
    assert_eq!(
        occurrence.identity.key,
        ScheduleOccurrenceKey::Calendar {
            local: local("2026-10-25T02:30:00")
        }
    );
    assert_eq!(
        recurrence
            .next_occurrence(&definition, utc("2026-10-25T00:45:00Z"))
            .unwrap()
            .unwrap()
            .local,
        local("2026-10-26T02:30:00"),
        "the later side of the repeated local time is not a second occurrence"
    );
}

#[test]
fn timezone_changes_reinterpret_future_instants_but_retain_local_identity() {
    let utc_recurrence = ScheduleRecurrence::new("UTC").unwrap();
    let berlin_recurrence = ScheduleRecurrence::new("Europe/Berlin").unwrap();
    let definition = calendar(CalendarRule::Weekly {
        weekdays: BTreeSet::from([ScheduleWeekday::Monday]),
        at: time(14, 0),
    });
    let after = utc("2026-07-26T00:00:00Z");
    let in_utc = utc_recurrence
        .next_occurrence(&definition, after)
        .unwrap()
        .unwrap();
    let in_berlin = berlin_recurrence
        .next_occurrence(&definition, after)
        .unwrap()
        .unwrap();
    assert_eq!(in_utc.identity, in_berlin.identity);
    assert_eq!(in_utc.scheduled_at, utc("2026-07-27T14:00:00Z"));
    assert_eq!(in_berlin.scheduled_at, utc("2026-07-27T12:00:00Z"));
    assert_eq!(berlin_recurrence.timezone_name(), "Europe/Berlin");
}

#[test]
fn macro_target_is_forward_compatible_but_cannot_activate_without_runtime() {
    let recurrence = ScheduleRecurrence::new("UTC").unwrap();
    let mut definition = schedule(ScheduleTrigger::Interval {
        every_seconds: 300,
        enabled_at: utc("2026-01-01T00:00:00Z"),
    });
    definition.target = ScheduleTarget::Macro {
        macro_id: ScheduleMacroId("venue-open".into()),
    };
    assert_eq!(
        recurrence
            .validate(&definition, utc("2026-01-01T00:00:00Z"), false)
            .unwrap_err()
            .field,
        "target"
    );
    definition.enabled = false;
    recurrence
        .validate(&definition, utc("2026-01-01T00:00:00Z"), false)
        .unwrap();
}

#[test]
fn playback_target_and_master_transition_are_bounded() {
    let mut definition = schedule(ScheduleTrigger::Interval {
        every_seconds: 300,
        enabled_at: utc("2026-01-01T00:00:00Z"),
    });
    definition.target = ScheduleTarget::Playback {
        page: 1,
        slot: 0,
        playback_number: 17,
        action: ScheduledPlaybackAction::Toggle,
        master_transition: None,
    };
    assert_eq!(definition.validate().unwrap_err().field, "target.slot");
    definition.target = ScheduleTarget::Playback {
        page: 1,
        slot: 2,
        playback_number: 17,
        action: ScheduledPlaybackAction::Toggle,
        master_transition: Some(PlaybackMasterTransition {
            level: 0.5,
            fade_millis: 60_001,
        }),
    };
    assert_eq!(
        definition.validate().unwrap_err().field,
        "target.master_transition.fade_millis"
    );
}

#[test]
fn playback_action_matrix_is_typed_without_persisting_runtime_kind() {
    for kind in [
        ScheduledPlaybackKind::CueList,
        ScheduledPlaybackKind::Dynamic,
    ] {
        for action in [
            ScheduledPlaybackAction::Go,
            ScheduledPlaybackAction::Pause,
            ScheduledPlaybackAction::On,
            ScheduledPlaybackAction::Off,
            ScheduledPlaybackAction::Release,
            ScheduledPlaybackAction::Toggle,
        ] {
            action.validate_for(kind).unwrap();
        }
    }
    for action in [
        ScheduledPlaybackAction::On,
        ScheduledPlaybackAction::Off,
        ScheduledPlaybackAction::Release,
        ScheduledPlaybackAction::Toggle,
    ] {
        action.validate_for(ScheduledPlaybackKind::Group).unwrap();
    }
    for action in [ScheduledPlaybackAction::Go, ScheduledPlaybackAction::Pause] {
        assert_eq!(
            action
                .validate_for(ScheduledPlaybackKind::Group)
                .unwrap_err()
                .field,
            "target.action"
        );
    }
    assert!(
        ScheduledPlaybackAction::On
            .validate_for(ScheduledPlaybackKind::Unsupported)
            .is_err()
    );
}

#[test]
fn schedule_and_occurrence_identity_round_trip_without_a_timezone_field() {
    let definition = calendar(CalendarRule::Expression {
        expression: CalendarExpression::parse("0 14 * * 1").unwrap(),
    });
    let json = serde_json::to_value(&definition).unwrap();
    assert!(json.get("timezone").is_none());
    assert_eq!(
        serde_json::from_value::<ScheduleDefinition>(json).unwrap(),
        definition
    );

    let occurrence = ScheduleRecurrence::new("UTC")
        .unwrap()
        .next_occurrence(&definition, utc("2026-01-01T00:00:00Z"))
        .unwrap()
        .unwrap();
    let encoded = serde_json::to_string(&occurrence.identity).unwrap();
    assert_eq!(
        serde_json::from_str::<ScheduleOccurrenceIdentity>(&encoded).unwrap(),
        occurrence.identity
    );
}

struct FakeWallClock(Mutex<DateTime<Utc>>);

impl WallClock for FakeWallClock {
    fn now_wall(&self) -> DateTime<Utc> {
        *self.0.lock().unwrap()
    }
}

struct FakeMonotonicClock(Mutex<Duration>);

impl MonotonicClock for FakeMonotonicClock {
    fn now_monotonic(&self) -> MonotonicMoment {
        MonotonicMoment(*self.0.lock().unwrap())
    }
}

#[derive(Default)]
struct TestCancellation(bool);

impl CancellationSignal for TestCancellation {
    fn is_cancelled(&self) -> bool {
        self.0
    }
}

#[derive(Default)]
struct FakeScheduler(Mutex<Vec<MonotonicMoment>>);

impl MonotonicScheduler for FakeScheduler {
    fn wait_until(
        &self,
        deadline: MonotonicMoment,
        cancellation: &dyn CancellationSignal,
    ) -> Result<(), SchedulerError> {
        if cancellation.is_cancelled() {
            return Err(SchedulerError::new(
                SchedulerErrorKind::Cancelled,
                "cancelled",
            ));
        }
        self.0.lock().unwrap().push(deadline);
        Ok(())
    }
}

#[test]
fn wall_clock_jumps_do_not_move_a_monotonic_deadline() {
    let wall = FakeWallClock(Mutex::new(utc("2026-01-01T00:00:00Z")));
    let monotonic = FakeMonotonicClock(Mutex::new(Duration::from_secs(10)));
    let deadline = deadline_after(&monotonic, Duration::from_secs(5)).unwrap();
    let scheduler = FakeScheduler::default();

    *wall.0.lock().unwrap() = utc("2030-01-01T00:00:00Z");
    scheduler
        .wait_until(deadline, &TestCancellation::default())
        .unwrap();
    *wall.0.lock().unwrap() = utc("2020-01-01T00:00:00Z");

    assert_eq!(scheduler.0.lock().unwrap().as_slice(), [deadline]);
    assert_eq!(deadline, MonotonicMoment(Duration::from_secs(15)));
    assert_eq!(wall.now_wall(), utc("2020-01-01T00:00:00Z"));
}

#[test]
fn monotonic_scheduler_exposes_cancellation_and_overflow() {
    let scheduler = FakeScheduler::default();
    assert_eq!(
        scheduler
            .wait_until(
                MonotonicMoment(Duration::from_secs(1)),
                &TestCancellation(true),
            )
            .unwrap_err()
            .kind,
        SchedulerErrorKind::Cancelled
    );
    assert_eq!(
        MonotonicMoment(Duration::MAX)
            .checked_after(Duration::from_nanos(1))
            .unwrap_err()
            .kind,
        SchedulerErrorKind::Overflow
    );
}
