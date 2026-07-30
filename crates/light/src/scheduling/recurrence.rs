use chrono::{
    DateTime, Datelike, Days, Duration as ChronoDuration, NaiveDate, NaiveDateTime, NaiveTime,
    Timelike, Utc,
};
use jiff::{
    Timestamp,
    civil::DateTime as JiffDateTime,
    tz::{AmbiguousOffset, TimeZone},
};

use super::{
    CalendarRule, IntervalSkip, MAXIMUM_PLAYBACK_FADE_MILLIS, MAXIMUM_SCHEDULE_PREVIEW,
    MINIMUM_INTERVAL_SECONDS, MonthWeekOrdinal, ScheduleDefinition, ScheduleOccurrence,
    ScheduleOccurrenceIdentity, ScheduleOccurrenceKey, ScheduleTarget, ScheduleTrigger,
    ScheduleValidationError, ScheduleWeekday,
};

const MAX_CALENDAR_SEARCH_DAYS: usize = 366 * 400;

/// Pure recurrence calculator bound to the authoritative server timezone.
#[derive(Clone, Debug)]
pub struct ScheduleRecurrence {
    timezone_name: String,
    timezone: TimeZone,
}

impl ScheduleRecurrence {
    pub fn new(timezone_name: impl Into<String>) -> Result<Self, ScheduleValidationError> {
        let timezone_name = timezone_name.into();
        let timezone = TimeZone::get(&timezone_name).map_err(|error| {
            ScheduleValidationError::new(
                "timezone",
                format!("unknown authoritative timezone {timezone_name:?}: {error}"),
            )
        })?;
        Ok(Self {
            timezone_name,
            timezone,
        })
    }

    pub fn timezone_name(&self) -> &str {
        &self.timezone_name
    }

    /// Validates one definition against product-level invariants known to the pure domain.
    pub fn validate(
        &self,
        schedule: &ScheduleDefinition,
        now: DateTime<Utc>,
        macro_runtime_available: bool,
    ) -> Result<(), ScheduleValidationError> {
        schedule.validate()?;
        self.validate_trigger_context(schedule, now)?;
        if schedule.enabled
            && !macro_runtime_available
            && matches!(&schedule.target, ScheduleTarget::Macro { .. })
        {
            return Err(ScheduleValidationError::new(
                "target",
                "Macro schedules cannot be enabled until the Macro runtime is available",
            ));
        }
        Ok(())
    }

    pub fn preview(
        &self,
        schedule: &ScheduleDefinition,
        after: DateTime<Utc>,
        limit: usize,
    ) -> Result<Vec<ScheduleOccurrence>, ScheduleValidationError> {
        if limit > MAXIMUM_SCHEDULE_PREVIEW {
            return Err(ScheduleValidationError::new(
                "limit",
                format!("preview limit cannot exceed {MAXIMUM_SCHEDULE_PREVIEW}"),
            ));
        }
        if limit == 0 {
            return Ok(Vec::new());
        }
        self.validate(schedule, after, true)?;
        match &schedule.trigger {
            ScheduleTrigger::Interval {
                every_seconds,
                enabled_at,
            } => {
                let mut occurrences =
                    interval_preview(schedule, *every_seconds, *enabled_at, after, limit)?;
                for occurrence in &mut occurrences {
                    occurrence.local = self.local_datetime(occurrence.scheduled_at)?;
                }
                Ok(occurrences)
            }
            ScheduleTrigger::Calendar { rule } => {
                self.calendar_preview(schedule, rule, after, limit)
            }
            ScheduleTrigger::OneTime { at } => {
                let Some(scheduled_at) = self.resolve_local(*at)? else {
                    return Ok(Vec::new());
                };
                Ok((scheduled_at > after)
                    .then(|| one_time_occurrence(schedule, *at, scheduled_at))
                    .into_iter()
                    .collect())
            }
        }
    }

    pub fn next_occurrence(
        &self,
        schedule: &ScheduleDefinition,
        after: DateTime<Utc>,
    ) -> Result<Option<ScheduleOccurrence>, ScheduleValidationError> {
        Ok(self.preview(schedule, after, 1)?.into_iter().next())
    }

    /// Advances an interval arithmetically to the first future multiple.
    ///
    /// Every occurrence at or before `now` is summarized as skipped. No backlog is enumerated.
    pub fn skip_interval_to_future(
        &self,
        schedule: &ScheduleDefinition,
        now: DateTime<Utc>,
    ) -> Result<IntervalSkip, ScheduleValidationError> {
        let ScheduleTrigger::Interval {
            every_seconds,
            enabled_at,
        } = &schedule.trigger
        else {
            return Err(ScheduleValidationError::new(
                "trigger",
                "skip arithmetic requires an Interval Schedule",
            ));
        };
        validate_interval(*every_seconds)?;
        let first = add_interval(*enabled_at, *every_seconds, 1)?;
        let skipped_occurrences = if now < first {
            0
        } else {
            let elapsed = now.signed_duration_since(first).num_seconds();
            u64::try_from(elapsed)
                .unwrap_or(u64::MAX)
                .checked_div(*every_seconds)
                .unwrap_or(0)
                .saturating_add(1)
        };
        let ordinal = skipped_occurrences.saturating_add(1);
        let scheduled_at = add_interval(*enabled_at, *every_seconds, ordinal)?;
        let local = self.local_datetime(scheduled_at)?;
        Ok(IntervalSkip {
            skipped_occurrences,
            next_occurrence: interval_occurrence(
                schedule,
                *enabled_at,
                ordinal,
                scheduled_at,
                local,
            ),
        })
    }

    fn validate_trigger_context(
        &self,
        schedule: &ScheduleDefinition,
        now: DateTime<Utc>,
    ) -> Result<(), ScheduleValidationError> {
        match &schedule.trigger {
            ScheduleTrigger::Interval { .. } | ScheduleTrigger::Calendar { .. } => Ok(()),
            ScheduleTrigger::OneTime { at } => {
                let scheduled_at = self.resolve_local(*at)?.ok_or_else(|| {
                    ScheduleValidationError::new(
                        "trigger.at",
                        "one-time local date and time does not exist in the authoritative timezone",
                    )
                })?;
                if schedule.enabled && scheduled_at <= now {
                    return Err(ScheduleValidationError::new(
                        "trigger.at",
                        "an enabled One-time Schedule must be in the future",
                    ));
                }
                Ok(())
            }
        }
    }

    fn calendar_preview(
        &self,
        schedule: &ScheduleDefinition,
        rule: &CalendarRule,
        after: DateTime<Utc>,
        limit: usize,
    ) -> Result<Vec<ScheduleOccurrence>, ScheduleValidationError> {
        validate_calendar_rule(rule)?;
        let after_local = self.local_datetime(after)?;
        let mut date = after_local.date();
        let mut occurrences = Vec::with_capacity(limit);
        for _ in 0..MAX_CALENDAR_SEARCH_DAYS {
            for time in matching_times(rule, date) {
                let local = date.and_time(time);
                let Some(scheduled_at) = self.resolve_local(local)? else {
                    // A wall-clock occurrence inside a DST gap is deliberately skipped.
                    continue;
                };
                if scheduled_at <= after {
                    continue;
                }
                occurrences.push(calendar_occurrence(schedule, local, scheduled_at));
                if occurrences.len() == limit {
                    return Ok(occurrences);
                }
            }
            date = date.checked_add_days(Days::new(1)).ok_or_else(|| {
                ScheduleValidationError::new("trigger", "calendar recurrence exceeded date range")
            })?;
        }
        Err(ScheduleValidationError::new(
            "trigger",
            "calendar rule produced no occurrence within the supported 400-year search window",
        ))
    }

    fn local_datetime(
        &self,
        instant: DateTime<Utc>,
    ) -> Result<NaiveDateTime, ScheduleValidationError> {
        let timestamp = chrono_to_jiff(instant)?;
        let local = self.timezone.to_datetime(timestamp);
        chrono_from_jiff_civil(local)
    }

    /// Resolves unambiguous civil time, skips gaps, and picks the earlier side of a fold.
    fn resolve_local(
        &self,
        local: NaiveDateTime,
    ) -> Result<Option<DateTime<Utc>>, ScheduleValidationError> {
        let civil = jiff_from_chrono_civil(local)?;
        let ambiguous = self.timezone.to_ambiguous_timestamp(civil);
        let timestamp = match ambiguous.offset() {
            AmbiguousOffset::Unambiguous { offset } => offset.to_timestamp(civil),
            AmbiguousOffset::Gap { .. } => return Ok(None),
            AmbiguousOffset::Fold { before, .. } => before.to_timestamp(civil),
        }
        .map_err(|error| {
            ScheduleValidationError::new(
                "trigger",
                format!("local occurrence is outside the supported timestamp range: {error}"),
            )
        })?;
        Ok(Some(chrono_from_jiff_timestamp(timestamp)?))
    }
}

pub(super) fn validate_definition_structure(
    schedule: &ScheduleDefinition,
) -> Result<(), ScheduleValidationError> {
    if schedule.name.trim().is_empty() {
        return Err(ScheduleValidationError::new(
            "name",
            "Schedule name cannot be empty",
        ));
    }
    match &schedule.trigger {
        ScheduleTrigger::Interval { every_seconds, .. } => validate_interval(*every_seconds)?,
        ScheduleTrigger::Calendar { rule } => validate_calendar_rule(rule)?,
        ScheduleTrigger::OneTime { .. } => {}
    }
    validate_target(&schedule.target)
}

fn validate_interval(every_seconds: u64) -> Result<(), ScheduleValidationError> {
    if every_seconds < MINIMUM_INTERVAL_SECONDS {
        return Err(ScheduleValidationError::new(
            "trigger.every_seconds",
            format!("interval must be at least {MINIMUM_INTERVAL_SECONDS} seconds"),
        ));
    }
    if every_seconds > i64::MAX as u64 {
        return Err(ScheduleValidationError::new(
            "trigger.every_seconds",
            "interval exceeds the supported timestamp range",
        ));
    }
    Ok(())
}

fn validate_calendar_rule(rule: &CalendarRule) -> Result<(), ScheduleValidationError> {
    match rule {
        CalendarRule::Weekly { weekdays, at } => {
            if weekdays.is_empty() {
                return Err(ScheduleValidationError::new(
                    "trigger.rule.weekdays",
                    "at least one weekday is required",
                ));
            }
            validate_recurring_time(*at)
        }
        CalendarRule::MonthlyDay { day, at } => {
            if !(1..=31).contains(day) {
                return Err(ScheduleValidationError::new(
                    "trigger.rule.day",
                    "monthly day must be within 1-31",
                ));
            }
            validate_recurring_time(*at)
        }
        CalendarRule::MonthlyWeekday { at, .. } => validate_recurring_time(*at),
        CalendarRule::EveryNDays { every_days, at, .. } => {
            if *every_days == 0 {
                return Err(ScheduleValidationError::new(
                    "trigger.rule.every_days",
                    "calendar day interval must be greater than zero",
                ));
            }
            validate_recurring_time(*at)
        }
        CalendarRule::Expression { expression } => {
            expression.parsed();
            Ok(())
        }
    }
}

fn validate_recurring_time(at: NaiveTime) -> Result<(), ScheduleValidationError> {
    if at.second() != 0 || at.nanosecond() != 0 {
        return Err(ScheduleValidationError::new(
            "trigger.rule.at",
            "recurring rules use minute precision; seconds are unsupported",
        ));
    }
    Ok(())
}

fn validate_target(target: &ScheduleTarget) -> Result<(), ScheduleValidationError> {
    match target {
        ScheduleTarget::Playback {
            page,
            slot,
            playback_number,
            master_transition,
            ..
        } => {
            for (field, value) in [
                ("target.page", page),
                ("target.slot", slot),
                ("target.playback_number", playback_number),
            ] {
                if *value == 0 {
                    return Err(ScheduleValidationError::new(
                        field,
                        "Playback identity values start at 1",
                    ));
                }
            }
            if let Some(transition) = master_transition {
                if !transition.level.is_finite() || !(0.0..=1.0).contains(&transition.level) {
                    return Err(ScheduleValidationError::new(
                        "target.master_transition.level",
                        "master level must be finite and within 0-1",
                    ));
                }
                if transition.fade_millis > MAXIMUM_PLAYBACK_FADE_MILLIS {
                    return Err(ScheduleValidationError::new(
                        "target.master_transition.fade_millis",
                        format!(
                            "master fade cannot exceed {MAXIMUM_PLAYBACK_FADE_MILLIS} milliseconds"
                        ),
                    ));
                }
            }
            Ok(())
        }
        ScheduleTarget::Macro { macro_id } => {
            if macro_id.0.trim().is_empty() {
                return Err(ScheduleValidationError::new(
                    "target.macro_id",
                    "Macro identity cannot be empty",
                ));
            }
            Ok(())
        }
    }
}

fn interval_preview(
    schedule: &ScheduleDefinition,
    every_seconds: u64,
    enabled_at: DateTime<Utc>,
    after: DateTime<Utc>,
    limit: usize,
) -> Result<Vec<ScheduleOccurrence>, ScheduleValidationError> {
    validate_interval(every_seconds)?;
    let ordinal = if after < enabled_at {
        1
    } else {
        let elapsed = after.signed_duration_since(enabled_at).num_seconds();
        u64::try_from(elapsed)
            .unwrap_or(u64::MAX)
            .checked_div(every_seconds)
            .unwrap_or(0)
            .saturating_add(1)
            .max(1)
    };
    (0..limit)
        .map(|offset| {
            let ordinal = ordinal
                .checked_add(offset as u64)
                .ok_or_else(|| ScheduleValidationError::new("trigger", "interval overflow"))?;
            let scheduled_at = add_interval(enabled_at, every_seconds, ordinal)?;
            Ok(interval_occurrence(
                schedule,
                enabled_at,
                ordinal,
                scheduled_at,
                scheduled_at.naive_utc(),
            ))
        })
        .collect()
}

fn add_interval(
    enabled_at: DateTime<Utc>,
    every_seconds: u64,
    ordinal: u64,
) -> Result<DateTime<Utc>, ScheduleValidationError> {
    let seconds = every_seconds
        .checked_mul(ordinal)
        .and_then(|value| i64::try_from(value).ok())
        .ok_or_else(|| ScheduleValidationError::new("trigger", "interval occurrence overflow"))?;
    enabled_at
        .checked_add_signed(ChronoDuration::seconds(seconds))
        .ok_or_else(|| ScheduleValidationError::new("trigger", "interval timestamp overflow"))
}

fn matching_times(rule: &CalendarRule, date: NaiveDate) -> Vec<NaiveTime> {
    match rule {
        CalendarRule::Weekly { weekdays, at } => weekdays
            .contains(&ScheduleWeekday::from(date.weekday()))
            .then_some(*at)
            .into_iter()
            .collect(),
        CalendarRule::MonthlyDay { day, at } => (date.day() == u32::from(*day))
            .then_some(*at)
            .into_iter()
            .collect(),
        CalendarRule::MonthlyWeekday {
            ordinal,
            weekday,
            at,
        } => {
            let weekday_matches = ScheduleWeekday::from(date.weekday()) == *weekday;
            let ordinal_matches = match ordinal {
                MonthWeekOrdinal::First => date.day().div_ceil(7) == 1,
                MonthWeekOrdinal::Second => date.day().div_ceil(7) == 2,
                MonthWeekOrdinal::Third => date.day().div_ceil(7) == 3,
                MonthWeekOrdinal::Fourth => date.day().div_ceil(7) == 4,
                MonthWeekOrdinal::Last => date
                    .checked_add_days(Days::new(7))
                    .is_some_and(|next| next.month() != date.month()),
            };
            (weekday_matches && ordinal_matches)
                .then_some(*at)
                .into_iter()
                .collect()
        }
        CalendarRule::EveryNDays {
            every_days,
            anchor,
            at,
        } => {
            let days = date.signed_duration_since(*anchor).num_days();
            (days >= 0 && days % i64::from(*every_days) == 0)
                .then_some(*at)
                .into_iter()
                .collect()
        }
        CalendarRule::Expression { expression } => {
            let parsed = expression.parsed();
            if !parsed.matches_date(date) {
                return Vec::new();
            }
            parsed
                .times()
                .map(|(hour, minute)| {
                    NaiveTime::from_hms_opt(u32::from(hour), u32::from(minute), 0)
                        .expect("validated expression time")
                })
                .collect()
        }
    }
}

fn interval_occurrence(
    schedule: &ScheduleDefinition,
    enabled_at: DateTime<Utc>,
    ordinal: u64,
    scheduled_at: DateTime<Utc>,
    local: NaiveDateTime,
) -> ScheduleOccurrence {
    ScheduleOccurrence {
        identity: ScheduleOccurrenceIdentity {
            schedule_id: schedule.id,
            key: ScheduleOccurrenceKey::Interval {
                enabled_at,
                ordinal,
            },
        },
        scheduled_at,
        local,
    }
}

fn calendar_occurrence(
    schedule: &ScheduleDefinition,
    local: NaiveDateTime,
    scheduled_at: DateTime<Utc>,
) -> ScheduleOccurrence {
    ScheduleOccurrence {
        identity: ScheduleOccurrenceIdentity {
            schedule_id: schedule.id,
            key: ScheduleOccurrenceKey::Calendar { local },
        },
        scheduled_at,
        local,
    }
}

fn one_time_occurrence(
    schedule: &ScheduleDefinition,
    local: NaiveDateTime,
    scheduled_at: DateTime<Utc>,
) -> ScheduleOccurrence {
    ScheduleOccurrence {
        identity: ScheduleOccurrenceIdentity {
            schedule_id: schedule.id,
            key: ScheduleOccurrenceKey::OneTime { local },
        },
        scheduled_at,
        local,
    }
}

fn chrono_to_jiff(value: DateTime<Utc>) -> Result<Timestamp, ScheduleValidationError> {
    Timestamp::new(value.timestamp(), value.timestamp_subsec_nanos() as i32).map_err(|error| {
        ScheduleValidationError::new(
            "timestamp",
            format!("instant is outside the supported timestamp range: {error}"),
        )
    })
}

fn chrono_from_jiff_timestamp(value: Timestamp) -> Result<DateTime<Utc>, ScheduleValidationError> {
    DateTime::from_timestamp(value.as_second(), value.subsec_nanosecond() as u32).ok_or_else(|| {
        ScheduleValidationError::new(
            "timestamp",
            "instant is outside the supported Chrono timestamp range",
        )
    })
}

fn jiff_from_chrono_civil(value: NaiveDateTime) -> Result<JiffDateTime, ScheduleValidationError> {
    JiffDateTime::new(
        value.year() as i16,
        value.month() as i8,
        value.day() as i8,
        value.hour() as i8,
        value.minute() as i8,
        value.second() as i8,
        value.nanosecond() as i32,
    )
    .map_err(|error| {
        ScheduleValidationError::new(
            "trigger",
            format!("local date and time is outside the supported range: {error}"),
        )
    })
}

fn chrono_from_jiff_civil(value: JiffDateTime) -> Result<NaiveDateTime, ScheduleValidationError> {
    let date = NaiveDate::from_ymd_opt(
        i32::from(value.year()),
        value.month() as u32,
        value.day() as u32,
    );
    let time = NaiveTime::from_hms_nano_opt(
        value.hour() as u32,
        value.minute() as u32,
        value.second() as u32,
        value.subsec_nanosecond() as u32,
    );
    date.zip(time)
        .map(|(date, time)| date.and_time(time))
        .ok_or_else(|| {
            ScheduleValidationError::new(
                "timestamp",
                "local date and time is outside the supported Chrono range",
            )
        })
}
