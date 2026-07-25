use std::{num::NonZeroU16, sync::Mutex, time::Duration};

use chrono::{DateTime, NaiveTime, TimeZone, Utc};
use uuid::Uuid;

use super::*;
use crate::MacroId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FakeMissedRunPolicy {
    Skip,
    CatchUp { limit: NonZeroU16 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum FakeWallClockSchedule {
    OneTime { at: DateTime<Utc> },
    Daily { at: NaiveTime },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FakeTimezoneId(String);

#[derive(Clone, Debug, Eq, PartialEq)]
struct FakeMacroSchedule {
    id: Uuid,
    macro_id: MacroId,
    timezone: FakeTimezoneId,
    timing: FakeWallClockSchedule,
    missed_run_policy: FakeMissedRunPolicy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FakeSchedulePlan {
    due: Vec<DateTime<Utc>>,
    skipped: usize,
}

fn plan_fake_recovery(
    schedule: &FakeMacroSchedule,
    previous: DateTime<Utc>,
    current: DateTime<Utc>,
) -> FakeSchedulePlan {
    let mut due = fake_occurrences(schedule, previous, current);
    match schedule.missed_run_policy {
        FakeMissedRunPolicy::Skip => FakeSchedulePlan {
            skipped: due.len(),
            due: Vec::new(),
        },
        FakeMissedRunPolicy::CatchUp { limit } => {
            let retained = usize::from(limit.get()).min(due.len());
            let skipped = due.len() - retained;
            let due = due.split_off(skipped);
            FakeSchedulePlan { due, skipped }
        }
    }
}

fn fake_occurrences(
    schedule: &FakeMacroSchedule,
    previous: DateTime<Utc>,
    current: DateTime<Utc>,
) -> Vec<DateTime<Utc>> {
    match schedule.timing {
        FakeWallClockSchedule::OneTime { at } => {
            if at > previous && at <= current {
                vec![at]
            } else {
                Vec::new()
            }
        }
        FakeWallClockSchedule::Daily { at } => {
            let mut date = previous.date_naive();
            let final_date = current.date_naive();
            let mut due = Vec::new();
            while date <= final_date {
                let occurrence = Utc.from_utc_datetime(&date.and_time(at));
                if occurrence > previous && occurrence <= current {
                    due.push(occurrence);
                }
                let Some(next) = date.succ_opt() else {
                    break;
                };
                date = next;
            }
            due
        }
    }
}

fn daily(policy: FakeMissedRunPolicy) -> FakeMacroSchedule {
    FakeMacroSchedule {
        id: Uuid::from_u128(1),
        macro_id: MacroId("morning".into()),
        timezone: FakeTimezoneId("Etc/UTC".into()),
        timing: FakeWallClockSchedule::Daily {
            at: NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
        },
        missed_run_policy: policy,
    }
}

fn utc(year: i32, month: u32, day: u32, hour: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(year, month, day, hour, 0, 0)
        .single()
        .unwrap()
}

#[test]
fn fake_daily_macro_schedule_proves_skip_and_bounded_catch_up() {
    let previous = utc(2026, 4, 1, 10);
    let current = utc(2026, 4, 4, 10);
    let skip_schedule = daily(FakeMissedRunPolicy::Skip);
    assert_eq!(skip_schedule.id, Uuid::from_u128(1));
    assert_eq!(skip_schedule.macro_id, MacroId("morning".into()));
    assert_eq!(skip_schedule.timezone, FakeTimezoneId("Etc/UTC".into()));
    let skipped = plan_fake_recovery(&skip_schedule, previous, current);
    assert!(skipped.due.is_empty());
    assert_eq!(skipped.skipped, 3);

    let caught_up = plan_fake_recovery(
        &daily(FakeMissedRunPolicy::CatchUp {
            limit: NonZeroU16::new(2).unwrap(),
        }),
        previous,
        current,
    );
    assert_eq!(caught_up.skipped, 1);
    assert_eq!(caught_up.due, [utc(2026, 4, 3, 9), utc(2026, 4, 4, 9)]);
}

#[test]
fn fake_one_time_macro_schedule_obeys_each_missed_run_policy() {
    let previous = utc(2026, 5, 2, 11);
    let current = utc(2026, 5, 2, 13);
    let mut schedule = FakeMacroSchedule {
        id: Uuid::from_u128(2),
        macro_id: MacroId("once".into()),
        timezone: FakeTimezoneId("Etc/UTC".into()),
        timing: FakeWallClockSchedule::OneTime {
            at: utc(2026, 5, 2, 12),
        },
        missed_run_policy: FakeMissedRunPolicy::Skip,
    };
    assert_eq!(
        plan_fake_recovery(&schedule, previous, current),
        FakeSchedulePlan {
            due: Vec::new(),
            skipped: 1,
        }
    );

    schedule.missed_run_policy = FakeMissedRunPolicy::CatchUp {
        limit: NonZeroU16::new(1).unwrap(),
    };
    assert_eq!(
        plan_fake_recovery(&schedule, previous, current).due,
        [utc(2026, 5, 2, 12)]
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
    let wall = FakeWallClock(Mutex::new(utc(2026, 1, 1, 0)));
    let monotonic = FakeMonotonicClock(Mutex::new(Duration::from_secs(10)));
    let deadline = deadline_after(&monotonic, Duration::from_secs(5)).unwrap();
    let scheduler = FakeScheduler::default();

    *wall.0.lock().unwrap() = utc(2030, 1, 1, 0);
    scheduler
        .wait_until(deadline, &TestCancellation::default())
        .unwrap();
    *wall.0.lock().unwrap() = utc(2020, 1, 1, 0);

    assert_eq!(scheduler.0.lock().unwrap().as_slice(), [deadline]);
    assert_eq!(deadline, MonotonicMoment(Duration::from_secs(15)));
    assert_eq!(wall.now_wall(), utc(2020, 1, 1, 0));
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
