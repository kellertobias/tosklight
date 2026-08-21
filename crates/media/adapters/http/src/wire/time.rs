//! The server's wall-clock offset from UTC.
//!
//! One offset for the whole server: a clock, or any text derived from the clock, shows the local
//! time of the venue rather than the timezone the machine happens to be set to. A clock may still
//! carry its own offset — a second clock showing another city is a real operator need — and that
//! choice stays with the clock rather than moving here.

use media_application::configuration::{MAXIMUM_UTC_OFFSET_MINUTES, TimeConfiguration};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The time settings, as the API reports them.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct TimeView {
    /// Minutes east of UTC. Negative is west; 0 is UTC itself.
    pub utc_offset_minutes: i16,
    /// The widest offset the server accepts, so a panel can bound its own control.
    pub maximum_utc_offset_minutes: i16,
}

impl TimeView {
    pub fn of(time: &TimeConfiguration) -> Self {
        Self {
            utc_offset_minutes: time.utc_offset_minutes,
            maximum_utc_offset_minutes: MAXIMUM_UTC_OFFSET_MINUTES,
        }
    }
}

/// An intent-shaped time edit.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTime {
    pub request_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub utc_offset_minutes: Option<i16>,
}

/// Why a time edit was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TimeEditError {
    #[error(
        "utcOffsetMinutes must be between -{MAXIMUM_UTC_OFFSET_MINUTES} and {MAXIMUM_UTC_OFFSET_MINUTES}"
    )]
    OffsetOutOfRange,
}

impl UpdateTime {
    /// The settings this edit describes, or why it was refused. Nothing is stored until the whole
    /// edit is accepted.
    pub fn applied(&self, current: &TimeConfiguration) -> Result<TimeConfiguration, TimeEditError> {
        let mut next = *current;
        if let Some(minutes) = self.utc_offset_minutes {
            next.utc_offset_minutes = minutes;
        }
        if !next.is_valid() {
            return Err(TimeEditError::OffsetOutOfRange);
        }
        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_absent_field_keeps_the_stored_offset_and_an_impossible_one_is_refused() {
        let current = TimeConfiguration {
            utc_offset_minutes: 120,
        };
        let unchanged = UpdateTime {
            request_id: "keep".into(),
            utc_offset_minutes: None,
        };
        assert_eq!(unchanged.applied(&current).unwrap(), current);

        let moved = UpdateTime {
            request_id: "move".into(),
            utc_offset_minutes: Some(-345),
        };
        assert_eq!(moved.applied(&current).unwrap().utc_offset_minutes, -345);

        let impossible = UpdateTime {
            request_id: "impossible".into(),
            utc_offset_minutes: Some(MAXIMUM_UTC_OFFSET_MINUTES + 1),
        };
        assert_eq!(
            impossible.applied(&current),
            Err(TimeEditError::OffsetOutOfRange)
        );
    }
}
