//! Playback settings an operator tunes while a show is running.
//!
//! Only the clip switch hold is exposed. The cache budget decides how much memory the process
//! reserves at start, so it stays a configuration-file value rather than a live control.

use media_application::configuration::{MAXIMUM_SWITCH_HOLD_MILLIS, PlaybackConfiguration};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The playback settings, as the API reports them.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackView {
    /// How long a layer keeps showing its previous clip while a newly selected one loads. Zero
    /// means the layer is empty until the new clip's first frame is ready.
    pub switch_hold_millis: u32,
    /// The longest hold the server accepts, so a panel can bound its own control.
    pub maximum_switch_hold_millis: u32,
}

impl PlaybackView {
    pub fn of(playback: &PlaybackConfiguration) -> Self {
        Self {
            switch_hold_millis: playback.switch_hold_millis,
            maximum_switch_hold_millis: MAXIMUM_SWITCH_HOLD_MILLIS,
        }
    }
}

/// An intent-shaped playback edit.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePlayback {
    pub request_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub switch_hold_millis: Option<u32>,
}

/// Why a playback edit was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PlaybackEditError {
    #[error("switchHoldMillis must be between 0 and {MAXIMUM_SWITCH_HOLD_MILLIS}")]
    SwitchHoldOutOfRange,
}

impl UpdatePlayback {
    /// The settings this edit describes, or why it was refused. Nothing is stored until the whole
    /// edit is accepted.
    pub fn applied(
        &self,
        current: &PlaybackConfiguration,
    ) -> Result<PlaybackConfiguration, PlaybackEditError> {
        let mut next = *current;
        if let Some(millis) = self.switch_hold_millis {
            next.switch_hold_millis = millis;
        }
        if !next.is_valid() {
            return Err(PlaybackEditError::SwitchHoldOutOfRange);
        }
        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_absent_field_keeps_the_stored_hold_and_an_overlong_one_is_refused() {
        let current = PlaybackConfiguration {
            switch_hold_millis: 250,
            ..PlaybackConfiguration::default()
        };
        let unchanged = UpdatePlayback {
            request_id: "keep".into(),
            switch_hold_millis: None,
        };
        assert_eq!(unchanged.applied(&current).unwrap(), current);

        let off = UpdatePlayback {
            request_id: "off".into(),
            switch_hold_millis: Some(0),
        };
        let applied = off.applied(&current).unwrap();
        assert_eq!(applied.switch_hold_millis, 0);
        assert_eq!(
            applied.cache_budget_bytes, current.cache_budget_bytes,
            "the edit touches only the hold"
        );

        let overlong = UpdatePlayback {
            request_id: "overlong".into(),
            switch_hold_millis: Some(MAXIMUM_SWITCH_HOLD_MILLIS + 1),
        };
        assert_eq!(
            overlong.applied(&current),
            Err(PlaybackEditError::SwitchHoldOutOfRange)
        );
    }
}
