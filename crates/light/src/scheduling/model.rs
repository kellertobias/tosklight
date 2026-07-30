use std::{collections::BTreeSet, fmt};

use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::CalendarExpression;

pub const MINIMUM_INTERVAL_SECONDS: u64 = 60;
pub const MAXIMUM_PLAYBACK_FADE_MILLIS: u32 = 60_000;
pub const MAXIMUM_SCHEDULE_PREVIEW: usize = 100;

/// Stable identity of one portable Schedule object.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ScheduleId(pub Uuid);

impl ScheduleId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ScheduleId {
    fn default() -> Self {
        Self::new()
    }
}

/// A show-owned Schedule definition. Runtime claims and result history are deliberately separate.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ScheduleDefinition {
    pub id: ScheduleId,
    pub name: String,
    pub enabled: bool,
    pub trigger: ScheduleTrigger,
    pub target: ScheduleTarget,
}

impl ScheduleDefinition {
    /// Validates portable, context-free Schedule invariants.
    ///
    /// Authoritative-timezone resolution, whether a One-time value is in the past, Macro runtime
    /// availability, and current Playback topology require a runtime validation context.
    pub fn validate(&self) -> Result<(), ScheduleValidationError> {
        super::recurrence::validate_definition_structure(self)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ScheduleTrigger {
    Interval {
        every_seconds: u64,
        enabled_at: DateTime<Utc>,
    },
    Calendar {
        rule: CalendarRule,
    },
    OneTime {
        /// Wall-clock date and time interpreted in the authoritative server timezone.
        at: NaiveDateTime,
    },
}

/// Typed calendar rules used by the guided editor plus the documented advanced expression.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CalendarRule {
    Weekly {
        weekdays: BTreeSet<ScheduleWeekday>,
        at: NaiveTime,
    },
    MonthlyDay {
        day: u8,
        at: NaiveTime,
    },
    MonthlyWeekday {
        ordinal: MonthWeekOrdinal,
        weekday: ScheduleWeekday,
        at: NaiveTime,
    },
    EveryNDays {
        every_days: u16,
        anchor: NaiveDate,
        at: NaiveTime,
    },
    Expression {
        expression: CalendarExpression,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleWeekday {
    Sunday,
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
}

impl ScheduleWeekday {
    pub const fn sunday_zero(self) -> u8 {
        match self {
            Self::Sunday => 0,
            Self::Monday => 1,
            Self::Tuesday => 2,
            Self::Wednesday => 3,
            Self::Thursday => 4,
            Self::Friday => 5,
            Self::Saturday => 6,
        }
    }
}

impl From<chrono::Weekday> for ScheduleWeekday {
    fn from(value: chrono::Weekday) -> Self {
        match value {
            chrono::Weekday::Sun => Self::Sunday,
            chrono::Weekday::Mon => Self::Monday,
            chrono::Weekday::Tue => Self::Tuesday,
            chrono::Weekday::Wed => Self::Wednesday,
            chrono::Weekday::Thu => Self::Thursday,
            chrono::Weekday::Fri => Self::Friday,
            chrono::Weekday::Sat => Self::Saturday,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MonthWeekOrdinal {
    First,
    Second,
    Third,
    Fourth,
    Last,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ScheduleTarget {
    Playback {
        page: u16,
        slot: u16,
        playback_number: u16,
        action: ScheduledPlaybackAction,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        master_transition: Option<PlaybackMasterTransition>,
    },
    Macro {
        macro_id: ScheduleMacroId,
    },
}

/// Forward-compatible stable Macro identity. Item 23 does not make this target executable.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ScheduleMacroId(pub String);

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScheduledPlaybackAction {
    Go,
    Pause,
    On,
    Off,
    Release,
    Toggle,
}

/// Runtime Playback family used to validate the initial Schedule action matrix.
///
/// This is resolved from authoritative Playback topology and is not persisted in the Schedule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScheduledPlaybackKind {
    CueList,
    Dynamic,
    Group,
    Unsupported,
}

impl ScheduledPlaybackAction {
    pub const fn supports(self, kind: ScheduledPlaybackKind) -> bool {
        match kind {
            ScheduledPlaybackKind::CueList | ScheduledPlaybackKind::Dynamic => true,
            ScheduledPlaybackKind::Group => {
                matches!(self, Self::On | Self::Off | Self::Release | Self::Toggle)
            }
            ScheduledPlaybackKind::Unsupported => false,
        }
    }

    pub fn validate_for(self, kind: ScheduledPlaybackKind) -> Result<(), ScheduleValidationError> {
        if self.supports(kind) {
            Ok(())
        } else {
            Err(ScheduleValidationError::new(
                "target.action",
                "Playback action is incompatible with the resolved Playback type",
            ))
        }
    }
}

/// One authoritative Playback-master transition; the Playback service performs the fade.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct PlaybackMasterTransition {
    /// Normalized level from 0 through 1.
    pub level: f32,
    pub fade_millis: u32,
}

/// Durable identity used to claim an occurrence before dispatch.
///
/// Calendar identity deliberately excludes the UTC offset so both sides of a DST fold share one
/// identity. A host-zone change likewise cannot re-run the same local occurrence.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ScheduleOccurrenceIdentity {
    pub schedule_id: ScheduleId,
    pub key: ScheduleOccurrenceKey,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ScheduleOccurrenceKey {
    Interval {
        enabled_at: DateTime<Utc>,
        ordinal: u64,
    },
    Calendar {
        local: NaiveDateTime,
    },
    OneTime {
        local: NaiveDateTime,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ScheduleOccurrence {
    pub identity: ScheduleOccurrenceIdentity,
    pub scheduled_at: DateTime<Utc>,
    pub local: NaiveDateTime,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScheduleOccurrenceProjection {
    pub occurrence_id: String,
    pub scheduled_for: String,
    pub local_time: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScheduleOccurrenceStatus {
    Claimed,
    Completed,
    Failed,
    Skipped,
    Interrupted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScheduleOccurrenceResult {
    pub occurrence: ScheduleOccurrenceProjection,
    pub status: ScheduleOccurrenceStatus,
    pub recorded_at: String,
    pub message: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScheduleRuntimeChange {
    pub show_id: light_core::ShowId,
    pub schedule_id: Uuid,
    pub next_occurrence: Option<ScheduleOccurrenceProjection>,
    pub last_result: Option<ScheduleOccurrenceResult>,
    pub validation_error: Option<String>,
}

/// Arithmetic result used when an inactive interval Schedule resumes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntervalSkip {
    pub skipped_occurrences: u64,
    pub next_occurrence: ScheduleOccurrence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScheduleValidationError {
    pub field: String,
    pub message: String,
}

impl ScheduleValidationError {
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for ScheduleValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.field, self.message)
    }
}

impl std::error::Error for ScheduleValidationError {}
