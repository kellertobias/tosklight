use chrono::{DateTime, Utc};
use light_core::CueListId;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SmpteTimecode {
    pub hours: u8,
    pub minutes: u8,
    pub seconds: u8,
    pub frames: u8,
    pub rate: FrameRate,
    pub source: String,
    pub received_at: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameRate {
    Fps24,
    Fps25,
    Fps2997Drop,
    Fps30,
}

impl FrameRate {
    pub fn nominal_frames(self) -> u8 {
        match self {
            Self::Fps24 => 24,
            Self::Fps25 => 25,
            Self::Fps2997Drop | Self::Fps30 => 30,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OscArgument {
    Int(i32),
    Float(f32),
    String(String),
    Bool(bool),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ControlEvent {
    Midi {
        status: u8,
        data: Vec<u8>,
    },
    Osc {
        address: String,
        arguments: Vec<OscArgument>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<String>,
    },
    Timecode(SmpteTimecode),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ControlTrigger {
    Osc { address: String },
    Midi { status: u8, data1: Option<u8> },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ControlAction {
    CueGo {
        cue_list_id: CueListId,
    },
    CueBack {
        cue_list_id: CueListId,
    },
    CuePause {
        cue_list_id: CueListId,
    },
    CueRelease {
        cue_list_id: CueListId,
    },
    Blackout {
        enabled: bool,
    },
    GrandMaster {
        level: f32,
    },
    /// Routes the desk's global SET key to connected operator surfaces.
    DeskSet,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ControlMapping {
    pub name: String,
    pub enabled: bool,
    pub trigger: ControlTrigger,
    pub action: ControlAction,
}

impl ControlMapping {
    pub fn matches(&self, event: &ControlEvent) -> bool {
        if !self.enabled {
            return false;
        }
        match (&self.trigger, event) {
            (ControlTrigger::Osc { address: expected }, ControlEvent::Osc { address, .. }) => {
                expected == address
            }
            (
                ControlTrigger::Midi {
                    status: expected,
                    data1,
                },
                ControlEvent::Midi { status, data },
            ) => expected == status && data1.is_none_or(|expected| data.first() == Some(&expected)),
            _ => false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseError(pub &'static str);
impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}
impl std::error::Error for ParseError {}
