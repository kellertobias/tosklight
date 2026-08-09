//! One output, its stored configuration, its layers, and their intent-shaped updates.

use media_application::configuration::{
    DmxProtocol, MonitorSelector, OutputConfiguration, OutputTarget, Resolution,
};
use media_domain::{LayerPersonality, PresentationMode};
use media_domain::{LayerState, MaskSource, MaskState, MasterState, MediaAddress, OutputState};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{AddressView, SourceStatusView};

/// A layer's mask, as the API reports it.
///
/// Reported even when it is doing nothing, because "a mask is selected but faded out" and "no mask
/// is selected" are different situations an operator needs to tell apart.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct MaskView {
    pub address: AddressView,
    pub scale_x: f32,
    pub scale_y: f32,
    pub invert: bool,
    pub opacity: f32,
    /// `alpha` or `luminance`.
    pub source: String,
    /// Whether it is currently shaping the layer at all.
    pub active: bool,
}

impl MaskView {
    pub fn of(mask: &MaskState) -> Self {
        Self {
            address: AddressView::of(mask.address),
            scale_x: mask.scale_x,
            scale_y: mask.scale_y,
            invert: mask.invert,
            opacity: mask.opacity,
            source: match mask.source {
                MaskSource::Alpha => "alpha",
                MaskSource::Luminance => "luminance",
            }
            .to_owned(),
            active: mask.is_active(),
        }
    }
}

/// One layer, as the API reports it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct LayerView {
    pub index: usize,
    pub address: AddressView,
    pub play_mode: String,
    pub dimmer: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub position_x: f32,
    pub position_y: f32,
    pub rotation: f32,
    pub grayscale: f32,
    pub source_status: SourceStatusView,
    pub mask: MaskView,
    /// Whether this layer contributes pixels right now.
    pub drawing: bool,
}

impl LayerView {
    pub fn of(index: usize, layer: &LayerState) -> Self {
        Self {
            index,
            address: AddressView::of(layer.address),
            play_mode: layer.play_mode.label().to_owned(),
            dimmer: layer.dimmer,
            scale_x: layer.scale_x,
            scale_y: layer.scale_y,
            position_x: layer.position_x,
            position_y: layer.position_y,
            rotation: layer.rotation,
            grayscale: layer.grayscale,
            source_status: SourceStatusView::of(layer.source_status),
            mask: MaskView::of(&layer.mask),
            drawing: layer.draws(),
        }
    }
}

/// The section that applies to the finished composite.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct MasterView {
    pub dimmer: f32,
    pub volume: f32,
    pub tint_red: f32,
    pub tint_green: f32,
    pub tint_blue: f32,
    pub flip_mirror: String,
    pub mask: AddressView,
}

impl MasterView {
    pub fn of(master: MasterState) -> Self {
        Self {
            dimmer: master.dimmer,
            volume: master.volume,
            tint_red: master.tint.red,
            tint_green: master.tint.green,
            tint_blue: master.tint.blue,
            flip_mirror: match master.flip_mirror {
                media_domain::FlipMirror::None => "none",
                media_domain::FlipMirror::Horizontal => "horizontal",
                media_domain::FlipMirror::Vertical => "vertical",
                media_domain::FlipMirror::Both => "both",
            }
            .to_owned(),
            mask: AddressView::of(master.mask),
        }
    }
}

/// One output's whole state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct OutputView {
    pub id: String,
    pub name: String,
    pub layer_count: usize,
    pub layers: Vec<LayerView>,
    pub master: MasterView,
    /// Whether an external desk currently owns this output's continuously controlled values.
    pub dmx_active: bool,
}

impl OutputView {
    pub fn of(output: &OutputState, name: String, dmx_active: bool) -> Self {
        Self {
            id: output.id.to_string(),
            name,
            layer_count: usize::from(output.personality.layer_count()),
            layers: output
                .layers
                .iter()
                .enumerate()
                .map(|(index, layer)| LayerView::of(index, layer))
                .collect(),
            master: MasterView::of(output.master),
            dmx_active,
        }
    }
}

/// The output settings an operator can inspect and edit.
///
/// This deliberately does not expose the retired status overlay. Library transcoding is not an
/// output setting either, so its target codec does not belong here. Every field this view does
/// expose is settled when the output and its ingress are created, and therefore takes effect on
/// restart.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct OutputConfigurationView {
    pub id: String,
    pub name: String,
    /// `monitor` or `off-screen`.
    pub target_kind: String,
    /// `index` or `name` when the target is a monitor.
    pub monitor_by: Option<String>,
    /// A decimal index or the literal monitor name, depending on `monitorBy`.
    pub monitor_value: Option<String>,
    pub fullscreen: bool,
    pub width: u32,
    pub height: u32,
    /// `display-synchronized`, `fixed-fps`, or `unlocked`.
    pub presentation: String,
    pub frames_per_second: Option<u16>,
    /// `two-layers` or `eight-layers`.
    pub personality: String,
    /// `art-net` or `sacn`.
    pub protocol: String,
    pub universe: u16,
    pub start_address: u16,
    /// Output surfaces, clocks, personalities, and DMX ingress are created once at startup.
    pub takes_effect_on_restart: bool,
}

impl OutputConfigurationView {
    pub fn of(output: &OutputConfiguration) -> Self {
        let (target_kind, monitor_by, monitor_value, fullscreen) = match &output.target {
            OutputTarget::OffScreen => ("off-screen", None, None, false),
            OutputTarget::Monitor {
                monitor,
                fullscreen,
            } => {
                let (by, value) = match monitor {
                    MonitorSelector::Index(index) => ("index", index.to_string()),
                    MonitorSelector::Name(name) => ("name", name.clone()),
                };
                ("monitor", Some(by.to_owned()), Some(value), *fullscreen)
            }
        };
        let (presentation, frames_per_second) = match output.presentation {
            PresentationMode::DisplaySynchronized => ("display-synchronized", None),
            PresentationMode::FixedFps { frames_per_second } => {
                ("fixed-fps", Some(frames_per_second))
            }
            PresentationMode::Unlocked => ("unlocked", None),
        };

        Self {
            id: output.id.to_string(),
            name: output.name.to_string(),
            target_kind: target_kind.to_owned(),
            monitor_by,
            monitor_value,
            fullscreen,
            width: output.resolution.width,
            height: output.resolution.height,
            presentation: presentation.to_owned(),
            frames_per_second,
            personality: match output.personality {
                LayerPersonality::TwoLayers => "two-layers",
                LayerPersonality::EightLayers => "eight-layers",
            }
            .to_owned(),
            protocol: match output.protocol {
                DmxProtocol::ArtNet => "art-net",
                DmxProtocol::Sacn => "sacn",
            }
            .to_owned(),
            universe: output.universe,
            start_address: output.start_address,
            takes_effect_on_restart: true,
        }
    }
}

/// An intent-shaped edit of one output's stored settings.
///
/// Every field is optional except the retry identity. An edit to the DMX address therefore cannot
/// silently move the output to another monitor or change its layer personality.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq, TS)]
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
    pub frames_per_second: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub personality: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub universe: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_address: Option<u16>,
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
        if let Some(personality) = self.personality.as_deref() {
            next.personality = match personality.trim() {
                "two-layers" => LayerPersonality::TwoLayers,
                "eight-layers" => LayerPersonality::EightLayers,
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

/// An intent-shaped layer update: only the fields being changed.
///
/// Absent means "leave alone", which is why every field is optional. Sending a dimmer must never
/// rewrite the layer's media selection.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLayer {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dimmer: Option<f32>,
}

impl UpdateLayer {
    /// The address this update selects, given what the layer already points at.
    ///
    /// Either component may be changed on its own, which is how a desk-style folder-then-file
    /// selection works.
    pub const fn address(&self, current: MediaAddress) -> MediaAddress {
        MediaAddress::new(
            match self.folder {
                Some(folder) => folder,
                None => current.folder,
            },
            match self.file {
                Some(file) => file,
                None => current.file,
            },
        )
    }

    pub const fn changes_address(&self) -> bool {
        self.folder.is_some() || self.file.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_update_leaves_out_what_it_does_not_change() {
        let body: UpdateLayer = serde_json::from_str(r#"{"dimmer":0.5}"#).unwrap();
        assert_eq!(body.dimmer, Some(0.5));
        assert_eq!(body.folder, None);
        assert!(
            !body.changes_address(),
            "a dimmer change must not touch the selection"
        );
    }

    #[test]
    fn either_half_of_an_address_can_change_on_its_own() {
        let current = MediaAddress::new(3, 7);
        let folder_only: UpdateLayer = serde_json::from_str(r#"{"folder":5}"#).unwrap();
        assert_eq!(folder_only.address(current), MediaAddress::new(5, 7));

        let file_only: UpdateLayer = serde_json::from_str(r#"{"file":9}"#).unwrap();
        assert_eq!(file_only.address(current), MediaAddress::new(3, 9));

        let both: UpdateLayer = serde_json::from_str(r#"{"folder":1,"file":2}"#).unwrap();
        assert_eq!(both.address(current), MediaAddress::new(1, 2));
    }

    #[test]
    fn an_empty_update_changes_nothing() {
        let body: UpdateLayer = serde_json::from_str("{}").unwrap();
        assert_eq!(body, UpdateLayer::default());
        assert_eq!(
            body.address(MediaAddress::new(4, 4)),
            MediaAddress::new(4, 4)
        );
    }

    fn configuration_edit(body: &str) -> UpdateOutputConfiguration {
        serde_json::from_str(body).expect("an output configuration edit")
    }

    #[test]
    fn output_configuration_reports_monitor_and_restart_semantics() {
        let mut output = OutputConfiguration::new("Main");
        output.target = OutputTarget::Monitor {
            monitor: MonitorSelector::Name("Stage Right".to_owned()),
            fullscreen: true,
        };
        output.presentation = PresentationMode::FixedFps {
            frames_per_second: 50,
        };
        let view = OutputConfigurationView::of(&output);

        assert_eq!(view.target_kind, "monitor");
        assert_eq!(view.monitor_by.as_deref(), Some("name"));
        assert_eq!(view.monitor_value.as_deref(), Some("Stage Right"));
        assert!(view.fullscreen);
        assert_eq!(view.presentation, "fixed-fps");
        assert_eq!(view.frames_per_second, Some(50));
        assert!(view.takes_effect_on_restart);
    }

    #[test]
    fn output_edits_are_intent_shaped_and_tolerate_unknown_fields() {
        let current = OutputConfiguration::new("Main");
        let next = configuration_edit(
            r#"{"requestId":"a","universe":7,"aFieldFromTheFuture":{"nested":true}}"#,
        )
        .applied(&current)
        .expect("accepted");

        assert_eq!(next.universe, 7);
        assert_eq!(next.target, current.target);
        assert_eq!(next.resolution, current.resolution);
        assert_eq!(next.presentation, current.presentation);
        assert_eq!(next.personality, current.personality);
        assert_eq!(next.protocol, current.protocol);
        assert_eq!(next.start_address, current.start_address);
    }

    #[test]
    fn a_monitor_target_requires_a_truthful_selector() {
        let current = OutputConfiguration::new("Main");
        let error = configuration_edit(r#"{"requestId":"a","targetKind":"monitor"}"#)
            .applied(&current)
            .unwrap_err();
        assert_eq!(error, OutputConfigurationEditError::MonitorMissing);

        let next = configuration_edit(
            r#"{"requestId":"b","targetKind":"monitor","monitorBy":"index","monitorValue":"2","fullscreen":true}"#,
        )
        .applied(&current)
        .expect("accepted");
        assert_eq!(
            next.target,
            OutputTarget::Monitor {
                monitor: MonitorSelector::Index(2),
                fullscreen: true,
            }
        );
    }

    #[test]
    fn fixed_rate_edits_carry_the_rate_and_other_modes_refuse_it() {
        let current = OutputConfiguration::new("Main");
        let fixed = configuration_edit(
            r#"{"requestId":"a","presentation":"fixed-fps","framesPerSecond":30}"#,
        )
        .applied(&current)
        .expect("accepted");
        assert_eq!(
            fixed.presentation,
            PresentationMode::FixedFps {
                frames_per_second: 30
            }
        );

        let error = configuration_edit(
            r#"{"requestId":"b","presentation":"unlocked","framesPerSecond":30}"#,
        )
        .applied(&current)
        .unwrap_err();
        assert_eq!(
            error,
            OutputConfigurationEditError::FixedFpsForOtherPresentation
        );
    }
}
