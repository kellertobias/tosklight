//! Typed v2 Scheduler definitions, projections, previews, and object intents.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ScheduleDefinition {
    pub id: Uuid,
    pub name: String,
    pub enabled: bool,
    pub trigger: ScheduleTrigger,
    pub target: ScheduleTarget,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ScheduleTrigger {
    Interval {
        #[ts(type = "number")]
        every_seconds: u64,
        /// RFC 3339 instant established when the Schedule became enabled.
        enabled_at: String,
    },
    Calendar {
        rule: ScheduleCalendarRule,
    },
    OneTime {
        /// Server-local civil timestamp without a per-Schedule timezone.
        at: String,
    },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ScheduleCalendarRule {
    Weekly {
        weekdays: Vec<ScheduleWeekday>,
        at: String,
    },
    MonthlyDay {
        day: u8,
        at: String,
    },
    MonthlyWeekday {
        ordinal: ScheduleMonthWeekOrdinal,
        weekday: ScheduleWeekday,
        at: String,
    },
    Expression {
        expression: String,
    },
    EveryNDays {
        every_days: u16,
        anchor: String,
        at: String,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
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

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleMonthWeekOrdinal {
    First,
    Second,
    Third,
    Fourth,
    Last,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ScheduleTarget {
    Playback {
        page: u16,
        slot: u16,
        playback_number: u16,
        action: SchedulePlaybackAction,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional = nullable)]
        master_transition: Option<ScheduleMasterTransition>,
    },
    Macro {
        macro_id: String,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SchedulePlaybackAction {
    Go,
    Pause,
    On,
    Off,
    Release,
    Toggle,
}

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ScheduleMasterTransition {
    pub level: f32,
    #[ts(type = "number")]
    pub fade_millis: u32,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ScheduleOccurrenceProjection {
    pub occurrence_id: String,
    /// RFC 3339 instant used for ordering and exactly-once claims.
    pub scheduled_for: String,
    /// Civil timestamp formatted in the authoritative server timezone.
    pub local_time: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleOccurrenceStatus {
    Claimed,
    Completed,
    Failed,
    Skipped,
    Interrupted,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ScheduleOccurrenceResult {
    pub occurrence: ScheduleOccurrenceProjection,
    pub status: ScheduleOccurrenceStatus,
    pub recorded_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub message: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ScheduleProjection {
    pub definition: ScheduleDefinition,
    #[ts(type = "number")]
    pub object_revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub next_occurrence: Option<ScheduleOccurrenceProjection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub last_result: Option<ScheduleOccurrenceResult>,
    pub history: Vec<ScheduleOccurrenceResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub validation_error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ScheduleSnapshot {
    pub show_id: Uuid,
    #[ts(type = "number")]
    pub show_revision: u64,
    /// IANA name of the server/desk timezone.
    pub timezone: String,
    /// RFC 3339 server instant used to make loading and stale previews explicit.
    pub server_now: String,
    #[ts(type = "number")]
    pub event_sequence: u64,
    pub schedules: Vec<ScheduleProjection>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct SchedulePreviewRequest {
    pub trigger: ScheduleTrigger,
    #[serde(default = "default_preview_count")]
    pub count: u8,
}

const fn default_preview_count() -> u8 {
    5
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct SchedulePreview {
    pub timezone: String,
    pub server_now: String,
    pub occurrences: Vec<ScheduleOccurrenceProjection>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ScheduleCreateRequest {
    pub request_id: String,
    pub definition: ScheduleCreateDefinition,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ScheduleCreateDefinition {
    pub name: String,
    pub enabled: bool,
    pub trigger: ScheduleTrigger,
    pub target: ScheduleTarget,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ScheduleUpdateRequest {
    pub request_id: String,
    #[ts(type = "number")]
    pub expected_revision: u64,
    pub patch: SchedulePatch,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct SchedulePatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub trigger: Option<ScheduleTrigger>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub target: Option<ScheduleTarget>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ScheduleDuplicateRequest {
    pub request_id: String,
    #[ts(type = "number")]
    pub expected_revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub name: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ScheduleDeleteRequest {
    pub request_id: String,
    #[ts(type = "number")]
    pub expected_revision: u64,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ScheduleMutationOutcome {
    pub request_id: String,
    pub replayed: bool,
    pub show_id: Uuid,
    #[ts(type = "number")]
    pub show_revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub schedule: Option<ScheduleProjection>,
    #[ts(type = "number")]
    pub event_sequence: u64,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct ScheduleRuntimeChange {
    pub show_id: Uuid,
    pub schedule_id: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub next_occurrence: Option<ScheduleOccurrenceProjection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub last_result: Option<ScheduleOccurrenceResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub validation_error: Option<String>,
}
