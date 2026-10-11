use serde::{Deserialize, Serialize};

/// Playback action retained while a Programmer is editing in Preload.
///
/// Only discrete Playback actions enter capture; continuous faders remain live.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum PreloadPlaybackQueueAction {
    #[serde(rename = "toggle")]
    Toggle,
    #[serde(rename = "go")]
    Go,
    #[serde(rename = "go-minus")]
    Back,
    #[serde(rename = "off")]
    Off,
    #[serde(rename = "on")]
    On,
    #[serde(rename = "temp-on")]
    TemporaryOn,
    #[serde(rename = "temp-off")]
    TemporaryOff,
    #[serde(rename = "dynamic-pause")]
    DynamicPause,
    #[serde(rename = "dynamic-restart")]
    DynamicRestart,
    #[serde(rename = "dynamic-double-speed")]
    DynamicDoubleSpeed,
    #[serde(rename = "dynamic-half-speed")]
    DynamicHalfSpeed,
    #[serde(rename = "dynamic-learn-speed")]
    DynamicLearnSpeed,
}

impl PreloadPlaybackQueueAction {
    pub const fn legacy_name(self) -> &'static str {
        match self {
            Self::Toggle => "toggle",
            Self::Go => "go",
            Self::Back => "go-minus",
            Self::Off => "off",
            Self::On => "on",
            Self::TemporaryOn => "temp-on",
            Self::TemporaryOff => "temp-off",
            Self::DynamicPause => "dynamic-pause",
            Self::DynamicRestart => "dynamic-restart",
            Self::DynamicDoubleSpeed => "dynamic-double-speed",
            Self::DynamicHalfSpeed => "dynamic-half-speed",
            Self::DynamicLearnSpeed => "dynamic-learn-speed",
        }
    }
}

impl TryFrom<&str> for PreloadPlaybackQueueAction {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "toggle" => Ok(Self::Toggle),
            "go" => Ok(Self::Go),
            "go-minus" | "back" => Ok(Self::Back),
            "off" => Ok(Self::Off),
            "on" => Ok(Self::On),
            "temp-on" | "temporary_on" => Ok(Self::TemporaryOn),
            "temp-off" | "temporary_off" => Ok(Self::TemporaryOff),
            "dynamic-pause" => Ok(Self::DynamicPause),
            "dynamic-restart" => Ok(Self::DynamicRestart),
            "dynamic-double-speed" => Ok(Self::DynamicDoubleSpeed),
            "dynamic-half-speed" => Ok(Self::DynamicHalfSpeed),
            "dynamic-learn-speed" => Ok(Self::DynamicLearnSpeed),
            value => Err(format!("unsupported queued Preload action {value}")),
        }
    }
}
