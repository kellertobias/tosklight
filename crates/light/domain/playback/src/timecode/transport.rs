use serde::{Deserialize, Serialize};

use super::TimecodeFrame;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimecodeTransportState {
    #[default]
    Stopped,
    Playing,
    Paused,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct TimecodeTransport {
    pub state: TimecodeTransportState,
    pub frame: TimecodeFrame,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TimecodeTransportAction {
    Go,
    Pause,
    Stop,
    Rewind,
    Seek { frame: TimecodeFrame },
}

impl TimecodeTransport {
    /// Applies the settled manual transport semantics without consulting a clock or performing I/O.
    pub fn apply(self, action: TimecodeTransportAction, duration: TimecodeFrame) -> Self {
        match action {
            TimecodeTransportAction::Go | TimecodeTransportAction::Rewind => Self {
                state: TimecodeTransportState::Playing,
                frame: TimecodeFrame::ZERO,
            },
            TimecodeTransportAction::Pause => Self {
                state: match self.state {
                    TimecodeTransportState::Playing => TimecodeTransportState::Paused,
                    TimecodeTransportState::Paused => TimecodeTransportState::Playing,
                    TimecodeTransportState::Stopped => TimecodeTransportState::Stopped,
                },
                ..self
            },
            TimecodeTransportAction::Stop => Self::default(),
            TimecodeTransportAction::Seek { frame } => Self {
                frame: TimecodeFrame(frame.0.min(duration.0)),
                ..self
            },
        }
    }
}
