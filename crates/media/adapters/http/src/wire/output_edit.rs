//! Editing one output's stored settings.
//!
//! The intent is a sibling of the view rather than part of it: a view is everything an operator can
//! see about an output, and an edit is only the part of it they are changing.

use media_application::configuration::{
    DmxProtocol, MonitorSelector, OutputConfiguration, OutputTarget, Resolution, SoundOutput,
};
use media_domain::{LayerPersonality, PresentationMode};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// An intent-shaped edit of one output's stored settings.
///
/// Every field is optional except the retry identity. An edit to the DMX address therefore cannot
/// silently move the output to another monitor or change its layer personality.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct UpdateOutputConfiguration {
    pub request_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub monitor_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub monitor_value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fullscreen: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frames_per_second: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound_output_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound_output_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub personality: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub personality_layout: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub universe: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_address: Option<u16>,
    /// The whole pixel map, replaced at once. A zone is meaningless on its own — its address has
    /// to be checked against every other zone's — so the map is edited as a piece.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pixel_map: Option<super::PixelMapView>,
}

/// Why an output settings edit could not describe a usable configuration.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OutputConfigurationEditError {
    #[error("targetKind must be 'monitor' or 'off-screen'")]
    TargetKind,
    #[error("monitorBy must be 'index' or 'name'")]
    MonitorBy,
    #[error("a monitor target needs monitorBy and monitorValue")]
    MonitorMissing,
    #[error("monitorValue must be a non-negative whole number when monitorBy is 'index'")]
    MonitorIndex,
    #[error("monitorValue must name a monitor when monitorBy is 'name'")]
    MonitorName,
    #[error("monitorBy, monitorValue, and fullscreen only apply to a monitor target")]
    MonitorFieldsForOffScreen,
    #[error("presentation must be 'display-synchronized', 'fixed-fps', or 'unlocked'")]
    Presentation,
    #[error("fixed-fps presentation needs framesPerSecond")]
    FixedFpsMissing,
    #[error("framesPerSecond only applies to fixed-fps presentation")]
    FixedFpsForOtherPresentation,
    #[error("soundOutputKind must be 'disabled', 'system-default', or 'device'")]
    SoundOutputKind,
    #[error("a device sound output needs soundOutputName")]
    SoundOutputMissing,
    #[error("the pixel map could not be read: {0}")]
    PixelMap(#[from] super::PixelMapEditError),
    #[error("soundOutputName only applies to a device sound output")]
    SoundOutputNameForOtherKind,
    #[error("personality must be 'two-layers' or 'eight-layers'")]
    Personality,
    #[error("protocol must be 'art-net' or 'sacn'")]
    Protocol,
}

impl UpdateOutputConfiguration {
    /// Applies only the stated intent. Whole-configuration validation remains the route's job,
    /// because it must also check this output against every other output's DMX patch.
    pub fn applied(
        &self,
        current: &OutputConfiguration,
    ) -> Result<OutputConfiguration, OutputConfigurationEditError> {
        let mut next = current.clone();
        next.target = self.target(&current.target)?;
        next.resolution = Resolution {
            width: self.width.unwrap_or(current.resolution.width),
            height: self.height.unwrap_or(current.resolution.height),
        };
        next.presentation = self.presentation(current.presentation)?;
        next.sound_output = self.sound_output(&current.sound_output)?;
        if let Some(map) = self.pixel_map.clone() {
            next.pixel_map = map.into_domain()?;
        }
        if let Some(personality) = self.personality.as_deref() {
            next.personality = match personality.trim() {
                "two-layers" => LayerPersonality::TwoLayers,
                "eight-layers" => LayerPersonality::EightLayers,
                _ => return Err(OutputConfigurationEditError::Personality),
            };
        }
        if let Some(layout) = self.personality_layout.as_deref() {
            next.personality_layout = match layout.trim() {
                "legacy" => media_domain::PersonalityLayout::Legacy,
                "current" => media_domain::PersonalityLayout::Current,
                "extended" => media_domain::PersonalityLayout::Extended,
                _ => return Err(OutputConfigurationEditError::Personality),
            };
        }
        if let Some(protocol) = self.protocol.as_deref() {
            next.protocol = match protocol.trim() {
                "art-net" => DmxProtocol::ArtNet,
                "sacn" => DmxProtocol::Sacn,
                _ => return Err(OutputConfigurationEditError::Protocol),
            };
        }
        if let Some(universe) = self.universe {
            next.universe = universe;
        }
        if let Some(start_address) = self.start_address {
            next.start_address = start_address;
        }
        Ok(next)
    }

    fn sound_output(
        &self,
        current: &SoundOutput,
    ) -> Result<SoundOutput, OutputConfigurationEditError> {
        match self.sound_output_kind.as_deref().map(str::trim) {
            None => {
                if self.sound_output_name.is_some() {
                    return Err(OutputConfigurationEditError::SoundOutputNameForOtherKind);
                }
                Ok(current.clone())
            }
            Some("disabled") => {
                if self.sound_output_name.is_some() {
                    return Err(OutputConfigurationEditError::SoundOutputNameForOtherKind);
                }
                Ok(SoundOutput::Disabled)
            }
            Some("system-default") => {
                if self.sound_output_name.is_some() {
                    return Err(OutputConfigurationEditError::SoundOutputNameForOtherKind);
                }
                Ok(SoundOutput::SystemDefault)
            }
            Some("device") => {
                let name = self
                    .sound_output_name
                    .as_deref()
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                    .ok_or(OutputConfigurationEditError::SoundOutputMissing)?;
                Ok(SoundOutput::Device {
                    name: name.to_owned(),
                })
            }
            Some(_) => Err(OutputConfigurationEditError::SoundOutputKind),
        }
    }

    fn target(&self, current: &OutputTarget) -> Result<OutputTarget, OutputConfigurationEditError> {
        let target_kind = self.target_kind.as_deref().map(str::trim);
        let wants_monitor = match target_kind {
            Some("monitor") => true,
            Some("off-screen") => false,
            Some(_) => return Err(OutputConfigurationEditError::TargetKind),
            None => matches!(current, OutputTarget::Monitor { .. }),
        };

        if !wants_monitor {
            if self.monitor_by.is_some()
                || self.monitor_value.is_some()
                || self.fullscreen.is_some()
            {
                return Err(OutputConfigurationEditError::MonitorFieldsForOffScreen);
            }
            return Ok(OutputTarget::OffScreen);
        }

        let (current_monitor, current_fullscreen) = match current {
            OutputTarget::Monitor {
                monitor,
                fullscreen,
            } => (Some(monitor), *fullscreen),
            OutputTarget::OffScreen => (None, false),
        };
        let monitor_by = self.monitor_by.as_deref().map(str::trim).or_else(|| {
            current_monitor.map(|monitor| match monitor {
                MonitorSelector::Index(_) => "index",
                MonitorSelector::Name(_) => "name",
            })
        });
        let monitor_value = self.monitor_value.as_deref().or_else(|| {
            if self.monitor_by.is_some() {
                None
            } else {
                current_monitor.map(|monitor| match monitor {
                    MonitorSelector::Index(_) => "",
                    MonitorSelector::Name(name) => name.as_str(),
                })
            }
        });

        let monitor = match (monitor_by, monitor_value, current_monitor) {
            (Some("index"), Some(value), _) if !value.trim().is_empty() => MonitorSelector::Index(
                value
                    .trim()
                    .parse()
                    .map_err(|_| OutputConfigurationEditError::MonitorIndex)?,
            ),
            (Some("index"), _, Some(MonitorSelector::Index(index)))
                if self.monitor_by.is_none() && self.monitor_value.is_none() =>
            {
                MonitorSelector::Index(*index)
            }
            (Some("index"), _, _) => return Err(OutputConfigurationEditError::MonitorMissing),
            (Some("name"), Some(value), _) if !value.trim().is_empty() => {
                MonitorSelector::Name(value.trim().to_owned())
            }
            (Some("name"), Some(_), _) => {
                return Err(OutputConfigurationEditError::MonitorName);
            }
            (Some("name"), _, Some(MonitorSelector::Name(name)))
                if self.monitor_by.is_none() && self.monitor_value.is_none() =>
            {
                MonitorSelector::Name(name.clone())
            }
            (Some("name"), _, _) => return Err(OutputConfigurationEditError::MonitorMissing),
            (Some(_), _, _) => return Err(OutputConfigurationEditError::MonitorBy),
            (None, _, _) => return Err(OutputConfigurationEditError::MonitorMissing),
        };

        Ok(OutputTarget::Monitor {
            monitor,
            fullscreen: self.fullscreen.unwrap_or(current_fullscreen),
        })
    }

    fn presentation(
        &self,
        current: PresentationMode,
    ) -> Result<PresentationMode, OutputConfigurationEditError> {
        let kind = self.presentation.as_deref().map(str::trim);
        let fixed_fps = match (kind, current) {
            (Some("display-synchronized"), _) => {
                if self.frames_per_second.is_some() {
                    return Err(OutputConfigurationEditError::FixedFpsForOtherPresentation);
                }
                return Ok(PresentationMode::DisplaySynchronized);
            }
            (Some("unlocked"), _) => {
                if self.frames_per_second.is_some() {
                    return Err(OutputConfigurationEditError::FixedFpsForOtherPresentation);
                }
                return Ok(PresentationMode::Unlocked);
            }
            (Some("fixed-fps"), PresentationMode::FixedFps { frames_per_second }) => {
                Some(frames_per_second)
            }
            (Some("fixed-fps"), _) => None,
            (Some(_), _) => return Err(OutputConfigurationEditError::Presentation),
            (None, PresentationMode::FixedFps { frames_per_second }) => Some(frames_per_second),
            (None, _) if self.frames_per_second.is_some() => {
                return Err(OutputConfigurationEditError::FixedFpsForOtherPresentation);
            }
            (None, _) => return Ok(current),
        };
        Ok(PresentationMode::FixedFps {
            frames_per_second: self
                .frames_per_second
                .or(fixed_fps)
                .ok_or(OutputConfigurationEditError::FixedFpsMissing)?,
        })
    }
}
