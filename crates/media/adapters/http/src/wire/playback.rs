//! Playback settings an operator tunes while a show is running.
//!
//! The clip switch hold and the In/Out point frame rate are exposed. The cache budget decides how much memory the process
//! reserves at start, so it stays a configuration-file value rather than a live control.

use media_application::configuration::{
    MAXIMUM_POINT_FRAME_RATE, MAXIMUM_SWITCH_HOLD_MILLIS, PlaybackConfiguration,
};
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
    /// Frames per second the layer In and Out point channels count in.
    pub frame_rate: u8,
    /// The fastest point rate the server accepts.
    pub maximum_frame_rate: u8,
}

impl PlaybackView {
    pub fn of(playback: &PlaybackConfiguration) -> Self {
        Self {
            switch_hold_millis: playback.switch_hold_millis,
            maximum_switch_hold_millis: MAXIMUM_SWITCH_HOLD_MILLIS,
            frame_rate: playback.frame_rate,
            maximum_frame_rate: MAXIMUM_POINT_FRAME_RATE,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_rate: Option<u8>,
}

/// Why a playback edit was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PlaybackEditError {
    #[error("switchHoldMillis must be between 0 and {MAXIMUM_SWITCH_HOLD_MILLIS}")]
    SwitchHoldOutOfRange,
    #[error("frameRate must be between 1 and {MAXIMUM_POINT_FRAME_RATE} frames per second")]
    FrameRateOutOfRange,
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
        if let Some(rate) = self.frame_rate {
            next.frame_rate = rate;
        }
        if !next.hold_is_valid() {
            return Err(PlaybackEditError::SwitchHoldOutOfRange);
        }
        if !next.frame_rate_is_valid() {
            return Err(PlaybackEditError::FrameRateOutOfRange);
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
            frame_rate: None,
        };
        assert_eq!(unchanged.applied(&current).unwrap(), current);

        let off = UpdatePlayback {
            request_id: "off".into(),
            switch_hold_millis: Some(0),
            frame_rate: None,
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
            frame_rate: None,
        };
        assert_eq!(
            overlong.applied(&current),
            Err(PlaybackEditError::SwitchHoldOutOfRange)
        );
    }

    #[test]
    fn a_frame_rate_edit_keeps_the_hold_and_an_out_of_range_rate_is_refused() {
        let current = PlaybackConfiguration::default();
        let thirty = UpdatePlayback {
            request_id: "thirty".into(),
            frame_rate: Some(30),
            ..UpdatePlayback::default()
        };
        let applied = thirty.applied(&current).unwrap();
        assert_eq!(applied.frame_rate, 30);
        assert_eq!(applied.switch_hold_millis, current.switch_hold_millis);
        for rate in [0, MAXIMUM_POINT_FRAME_RATE + 1] {
            let refused = UpdatePlayback {
                request_id: "bad".into(),
                frame_rate: Some(rate),
                ..UpdatePlayback::default()
            };
            assert_eq!(
                refused.applied(&current),
                Err(PlaybackEditError::FrameRateOutOfRange)
            );
        }
    }
}
