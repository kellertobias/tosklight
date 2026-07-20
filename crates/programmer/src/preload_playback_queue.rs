use serde::{Deserialize, Serialize};

/// Playback action retained while a Programmer is editing in Preload.
///
/// The serialized names preserve the existing show-file representation. The scoped v2
/// projection translates these to its intentionally cleaner public names.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum PreloadPlaybackQueueAction {
    #[serde(rename = "toggle")]
    Toggle,
    #[serde(rename = "go")]
    Go,
    #[serde(rename = "go-minus", alias = "back")]
    Back,
    #[serde(rename = "off")]
    Off,
    #[serde(rename = "on")]
    On,
    #[serde(rename = "temp-on", alias = "temporary_on")]
    TemporaryOn,
    #[serde(rename = "temp-off", alias = "temporary_off")]
    TemporaryOff,
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
            value => Err(format!("unsupported queued Preload action {value}")),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreloadPlaybackQueueSurface {
    Physical,
    Virtual,
    Osc,
    Matter,
}

impl PreloadPlaybackQueueSurface {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Physical => "physical",
            Self::Virtual => "virtual",
            Self::Osc => "osc",
            Self::Matter => "matter",
        }
    }
}

impl TryFrom<&str> for PreloadPlaybackQueueSurface {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "physical" => Ok(Self::Physical),
            "virtual" => Ok(Self::Virtual),
            "osc" => Ok(Self::Osc),
            "matter" => Ok(Self::Matter),
            value => Err(format!("unsupported queued Preload surface {value}")),
        }
    }
}
