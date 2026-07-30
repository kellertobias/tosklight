//! Schedule definitions, wall-clock recurrence policy, and neutral scheduler ports.

use std::time::Duration;

use chrono::{DateTime, Utc};

mod expression;
mod model;
mod recurrence;

pub use expression::CalendarExpression;
pub use model::{
    CalendarRule, IntervalSkip, MAXIMUM_PLAYBACK_FADE_MILLIS, MAXIMUM_SCHEDULE_PREVIEW,
    MINIMUM_INTERVAL_SECONDS, MonthWeekOrdinal, PlaybackMasterTransition, ScheduleDefinition,
    ScheduleId, ScheduleMacroId, ScheduleOccurrence, ScheduleOccurrenceIdentity,
    ScheduleOccurrenceKey, ScheduleOccurrenceProjection, ScheduleOccurrenceResult,
    ScheduleOccurrenceStatus, ScheduleRuntimeChange, ScheduleTarget, ScheduleTrigger,
    ScheduleValidationError, ScheduleWeekday, ScheduledPlaybackAction, ScheduledPlaybackKind,
};
pub use recurrence::ScheduleRecurrence;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MonotonicMoment(pub Duration);

impl MonotonicMoment {
    pub fn checked_after(self, delay: Duration) -> Result<Self, SchedulerError> {
        self.0
            .checked_add(delay)
            .map(Self)
            .ok_or_else(|| SchedulerError::new(SchedulerErrorKind::Overflow, "timer overflow"))
    }
}

pub trait WallClock: Send + Sync {
    fn now_wall(&self) -> DateTime<Utc>;
}

pub trait MonotonicClock: Send + Sync {
    fn now_monotonic(&self) -> MonotonicMoment;
}

pub trait CancellationSignal: Send + Sync {
    fn is_cancelled(&self) -> bool;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchedulerErrorKind {
    Cancelled,
    Overflow,
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchedulerError {
    pub kind: SchedulerErrorKind,
    pub message: String,
}

impl SchedulerError {
    pub fn new(kind: SchedulerErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

/// Cancellation-aware wait boundary for application services.
///
/// Implementations may park an application worker or use an async runtime internally. The output
/// render path must never call this boundary.
pub trait MonotonicScheduler: Send + Sync {
    fn wait_until(
        &self,
        deadline: MonotonicMoment,
        cancellation: &dyn CancellationSignal,
    ) -> Result<(), SchedulerError>;
}

pub fn deadline_after(
    clock: &dyn MonotonicClock,
    delay: Duration,
) -> Result<MonotonicMoment, SchedulerError> {
    clock.now_monotonic().checked_after(delay)
}

#[cfg(test)]
mod tests;
