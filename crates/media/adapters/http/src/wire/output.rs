//! One output, its stored configuration, its layers, and their intent-shaped updates.

use media_application::configuration::{
    DmxProtocol, MonitorSelector, OutputConfiguration, OutputTarget, Resolution, SoundOutput,
};
use media_domain::{
    ANALOG_TV_EFFECT, AnalogTvParameters, BEAT_FORM_FLASH_EFFECT, BEAT_GRID_WAVE_EFFECT,
    BEAT_MOVE_EFFECT, BEAT_SCALE_TURN_EFFECT, BEAT_SCAN_EFFECT, BLUR_EFFECT,
    BeatFormFlashParameters, BeatGridWaveParameters, BeatMoveParameters, BeatScaleTurnParameters,
    BeatScanParameters, BlurParameters, DIGITAL_TV_EFFECT, DRAWN_IMAGE_EFFECT, DigitalTvParameters,
    DrawnImageParameters, EffectSlot, FEEDBACK_EFFECT, FeedbackParameters, KALEIDOSCOPE_EFFECT,
    KaleidoscopeParameters, LayerState, MaskSource, MaskState, MasterState, MediaAddress,
    OPACITY_CYCLE_EFFECT, OpacityCycleInterval, OutputState, RASTERIZE_EFFECT, RasterizeParameters,
};
use media_domain::{LayerPersonality, PresentationMode};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{AddressView, SourceStatusView, VisualizerParametersView};

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
    pub position_x: f32,
    pub position_y: f32,
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
            position_x: mask.position_x,
            position_y: mask.position_y,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct EffectParameterView {
    pub id: String,
    pub label: String,
    pub value: f32,
    pub default_value: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct EffectSlotView {
    pub index: usize,
    pub effect_type: Option<String>,
    pub label: String,
    pub enabled: bool,
    pub mix: f32,
    pub supported: bool,
    pub capability_detail: Option<String>,
    pub parameters: Vec<EffectParameterView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visualizer_parameters: Option<VisualizerParametersView>,
}

impl EffectSlotView {
    fn of(index: usize, effect: &EffectSlot) -> Self {
        let analog = effect.effect_type.as_deref() == Some(ANALOG_TV_EFFECT);
        let digital = effect.effect_type.as_deref() == Some(DIGITAL_TV_EFFECT);
        let blur = effect.effect_type.as_deref() == Some(BLUR_EFFECT);
        let feedback = effect.effect_type.as_deref() == Some(FEEDBACK_EFFECT);
        let opacity_cycle = effect.effect_type.as_deref() == Some(OPACITY_CYCLE_EFFECT);
        let beat_move = effect.effect_type.as_deref() == Some(BEAT_MOVE_EFFECT);
        let kaleidoscope = effect.effect_type.as_deref() == Some(KALEIDOSCOPE_EFFECT);
        let rasterize = effect.effect_type.as_deref() == Some(RASTERIZE_EFFECT);
        let beat_scan = effect.effect_type.as_deref() == Some(BEAT_SCAN_EFFECT);
        let beat_scale_turn = effect.effect_type.as_deref() == Some(BEAT_SCALE_TURN_EFFECT);
        let beat_grid_wave = effect.effect_type.as_deref() == Some(BEAT_GRID_WAVE_EFFECT);
        let beat_form_flash = effect.effect_type.as_deref() == Some(BEAT_FORM_FLASH_EFFECT);
        let drawn_image = effect.effect_type.as_deref() == Some(DRAWN_IMAGE_EFFECT);
        let parameters = if analog {
            let defaults = AnalogTvParameters::default().as_array();
            let values = AnalogTvParameters::from_normalized(&effect.parameters).as_array();
            parameter_views(
                &AnalogTvParameters::IDS,
                &AnalogTvParameters::LABELS,
                &values,
                &defaults,
            )
        } else if digital {
            let defaults = DigitalTvParameters::default().as_array();
            let values = DigitalTvParameters::from_normalized(&effect.parameters).as_array();
            parameter_views(
                &DigitalTvParameters::IDS,
                &DigitalTvParameters::LABELS,
                &values,
                &defaults,
            )
        } else if blur {
            let defaults = BlurParameters::default().as_array();
            let values = BlurParameters::from_normalized(&effect.parameters).as_array();
            parameter_views(&["blur-amount"], &["Blur amount"], &values, &defaults)
        } else if feedback {
            let defaults = FeedbackParameters::default().as_array();
            let values = FeedbackParameters::from_normalized(&effect.parameters).as_array();
            parameter_views(
                &["feedback-amount", "feedback-motion", "feedback-direction"],
                &["Feedback amount", "Motion speed", "Motion direction"],
                &values,
                &defaults,
            )
        } else if opacity_cycle {
            vec![EffectParameterView {
                id: "cycle-interval".to_owned(),
                label: "Interval".to_owned(),
                value: effect
                    .opacity_cycle_interval()
                    .unwrap_or(OpacityCycleInterval::EveryBeat)
                    .parameter(),
                default_value: OpacityCycleInterval::EveryBeat.parameter(),
            }]
        } else if beat_move {
            let defaults = BeatMoveParameters::default().as_array();
            let values = BeatMoveParameters::from_parameters(&effect.parameters).as_array();
            parameter_views(
                &BeatMoveParameters::IDS,
                &BeatMoveParameters::LABELS,
                &values,
                &defaults,
            )
        } else if kaleidoscope {
            let defaults = KaleidoscopeParameters::default().as_array();
            let values = KaleidoscopeParameters::from_parameters(&effect.parameters).as_array();
            parameter_views(
                &KaleidoscopeParameters::IDS,
                &KaleidoscopeParameters::LABELS,
                &values,
                &defaults,
            )
        } else if rasterize {
            let defaults = RasterizeParameters::default().as_array();
            let values = RasterizeParameters::from_parameters(&effect.parameters).as_array();
            parameter_views(
                &RasterizeParameters::IDS,
                &RasterizeParameters::LABELS,
                &values,
                &defaults,
            )
        } else if beat_scan {
            let defaults = BeatScanParameters::default().as_array();
            let values = BeatScanParameters::from_parameters(&effect.parameters).as_array();
            parameter_views(
                &BeatScanParameters::IDS,
                &BeatScanParameters::LABELS,
                &values,
                &defaults,
            )
        } else if beat_scale_turn {
            let defaults = BeatScaleTurnParameters::default().as_array();
            let values = BeatScaleTurnParameters::from_parameters(&effect.parameters).as_array();
            parameter_views(
                &BeatScaleTurnParameters::IDS,
                &BeatScaleTurnParameters::LABELS,
                &values,
                &defaults,
            )
        } else if beat_grid_wave {
            let defaults = BeatGridWaveParameters::default().as_array();
            let values = BeatGridWaveParameters::from_parameters(&effect.parameters).as_array();
            parameter_views(
                &BeatGridWaveParameters::IDS,
                &BeatGridWaveParameters::LABELS,
                &values,
                &defaults,
            )
        } else if beat_form_flash {
            let defaults = BeatFormFlashParameters::default().as_array();
            let values = BeatFormFlashParameters::from_parameters(&effect.parameters).as_array();
            parameter_views(
                &BeatFormFlashParameters::IDS,
                &BeatFormFlashParameters::LABELS,
                &values,
                &defaults,
            )
        } else if drawn_image {
            let defaults = DrawnImageParameters::default().as_array();
            let values = DrawnImageParameters::from_parameters(&effect.parameters).as_array();
            parameter_views(
                &DrawnImageParameters::IDS,
                &DrawnImageParameters::LABELS,
                &values,
                &defaults,
            )
        } else {
            Vec::new()
        };
        Self {
            index,
            effect_type: effect.effect_type.clone(),
            label: if analog
                || digital
                || blur
                || feedback
                || opacity_cycle
                || beat_move
                || kaleidoscope
                || rasterize
                || beat_scan
                || beat_scale_turn
                || beat_grid_wave
                || beat_form_flash
                || drawn_image
            {
                if analog {
                    "Analog TV"
                } else if digital {
                    "Digital TV"
                } else if blur {
                    "Blur"
                } else if feedback {
                    "Feedback"
                } else if opacity_cycle {
                    "Layer opacity cycle"
                } else if beat_move {
                    "Beat Move"
                } else if kaleidoscope {
                    "Kaleidoscope"
                } else if rasterize {
                    "Rasterized Print"
                } else if beat_scan {
                    "Beat Scan"
                } else if beat_scale_turn {
                    "Beat Scale and Turn"
                } else if beat_form_flash {
                    "Beat Form Flash"
                } else if drawn_image {
                    "Drawn Image"
                } else {
                    "Beat Grid Wave"
                }
                .to_owned()
            } else {
                effect.effect_type.as_deref().unwrap_or("None").to_owned()
            },
            enabled: effect.enabled,
            mix: effect.mix,
            supported: effect.effect_type.is_none()
                || analog
                || digital
                || blur
                || feedback
                || opacity_cycle
                || beat_move
                || kaleidoscope
                || rasterize
                || beat_scan
                || beat_scale_turn
                || beat_grid_wave
                || beat_form_flash
                || drawn_image,
            capability_detail: (!analog
                && !digital
                && !blur
                && !feedback
                && !opacity_cycle
                && !beat_move
                && !kaleidoscope
                && !rasterize
                && !beat_scan
                && !beat_scale_turn
                && !beat_grid_wave
                && !beat_form_flash
                && !drawn_image
                && effect.effect_type.is_some())
            .then(|| "This Media Server build cannot render the selected effect.".to_owned()),
            parameters,
            visualizer_parameters: effect
                .visualizer_parameters
                .as_ref()
                .map(VisualizerParametersView::of),
        }
    }
}

fn parameter_views(
    ids: &[&str],
    labels: &[&str],
    values: &[f32],
    defaults: &[f32],
) -> Vec<EffectParameterView> {
    ids.iter()
        .zip(labels)
        .zip(values.iter().zip(defaults))
        .map(
            |((id, label), (value, default_value))| EffectParameterView {
                id: (*id).to_owned(),
                label: (*label).to_owned(),
                value: *value,
                default_value: *default_value,
            },
        )
        .collect()
}

/// One layer, as the API reports it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct LayerView {
    pub index: usize,
    pub address: AddressView,
    pub play_mode: String,
    pub play_mode_dmx: u8,
    pub dimmer: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub scaling_mode: String,
    pub position_x: f32,
    pub position_y: f32,
    pub rotation: f32,
    pub grayscale: f32,
    pub volume: f32,
    pub tint_red: f32,
    pub tint_green: f32,
    pub tint_blue: f32,
    pub speed_multiplier: String,
    pub speed_multiplier_dmx: u8,
    pub playback_bpm: Option<u8>,
    pub blur: f32,
    pub source_status: SourceStatusView,
    pub mask: MaskView,
    pub effects: Vec<EffectSlotView>,
    /// Whether this layer contributes pixels right now.
    pub drawing: bool,
}

impl LayerView {
    pub fn of(index: usize, layer: &LayerState) -> Self {
        Self {
            index,
            address: AddressView::of(layer.address),
            play_mode: layer.play_mode.label().to_owned(),
            play_mode_dmx: layer.play_mode.dmx_range().0,
            dimmer: layer.dimmer,
            scale_x: layer.scale_x,
            scale_y: layer.scale_y,
            scaling_mode: match layer.scaling_mode {
                media_domain::ScalingMode::Fit => "fit",
                media_domain::ScalingMode::Fill => "fill",
                media_domain::ScalingMode::Original => "original",
                media_domain::ScalingMode::Stretch => "stretch",
            }
            .to_owned(),
            position_x: layer.position_x,
            position_y: layer.position_y,
            rotation: layer.rotation,
            grayscale: layer.grayscale,
            volume: layer.volume,
            tint_red: layer.tint.red,
            tint_green: layer.tint.green,
            tint_blue: layer.tint.blue,
            speed_multiplier: layer.speed_multiplier.label(),
            speed_multiplier_dmx: (0..=u8::MAX)
                .find(|value| {
                    media_domain::SpeedMultiplier::from_dmx(*value) == layer.speed_multiplier
                })
                .unwrap_or(127),
            playback_bpm: layer.playback_bpm,
            blur: layer.blur,
            source_status: SourceStatusView::of(layer.source_status),
            mask: MaskView::of(&layer.mask),
            effects: layer
                .effects
                .iter()
                .enumerate()
                .map(|(index, effect)| EffectSlotView::of(index, effect))
                .collect(),
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
    pub mask_position_x: f32,
    pub mask_position_y: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub scaling_mode: String,
    pub position_x: f32,
    pub position_y: f32,
    pub rotation: f32,
    pub shaper_left: f32,
    pub shaper_right: f32,
    pub shaper_top: f32,
    pub shaper_bottom: f32,
    pub shaper_left_rotation: f32,
    pub shaper_right_rotation: f32,
    pub shaper_top_rotation: f32,
    pub shaper_bottom_rotation: f32,
    pub shaper_rotation: f32,
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
            mask_position_x: master.mask_position_x,
            mask_position_y: master.mask_position_y,
            scale_x: master.scale_x,
            scale_y: master.scale_y,
            scaling_mode: match master.scaling_mode {
                media_domain::ScalingMode::Fit => "fit",
                media_domain::ScalingMode::Fill => "fill",
                media_domain::ScalingMode::Original => "original",
                media_domain::ScalingMode::Stretch => "stretch",
            }
            .to_owned(),
            position_x: master.position_x,
            position_y: master.position_y,
            rotation: master.rotation,
            shaper_left: master.shaper.left,
            shaper_right: master.shaper.right,
            shaper_top: master.shaper.top,
            shaper_bottom: master.shaper.bottom,
            shaper_left_rotation: master.shaper.left_rotation,
            shaper_right_rotation: master.shaper.right_rotation,
            shaper_top_rotation: master.shaper.top_rotation,
            shaper_bottom_rotation: master.shaper.bottom_rotation,
            shaper_rotation: master.shaper.rotation,
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
    /// Whether this server explicitly ignores network playback control in favour of the web UI.
    pub playback_takeover: bool,
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
            playback_takeover: output.ownership.web_takeover,
        }
    }
}

/// The output settings an operator can inspect and edit.
///
/// This deliberately does not expose the retired status overlay. Library transcoding is not an
/// output setting either, so its target codec does not belong here. Every field this view does
/// expose is settled when the output and its ingress are created, and therefore takes effect on
/// restart.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
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
    pub frames_per_second: Option<f64>,
    /// `disabled`, `system-default`, or `device`.
    pub sound_output_kind: String,
    /// The exact operating-system device name when `soundOutputKind` is `device`.
    pub sound_output_name: Option<String>,
    /// Monitors the presentation event loop currently sees.
    pub available_monitors: Vec<AvailableMonitorView>,
    /// Audio outputs the operating system currently reports.
    pub available_sound_outputs: Vec<String>,
    /// `two-layers` or `eight-layers`.
    pub personality: String,
    /// `legacy` preserves the original 35/7-slot blocks; `current` includes mask positioning.
    pub personality_layout: String,
    /// `art-net` or `sacn`.
    pub protocol: String,
    pub universe: u16,
    pub start_address: u16,
    /// Exact startup values, used to restore what the running process is using now.
    pub active: OutputConfigurationValuesView,
    pub picture_pending_restart: bool,
    pub sound_pending_restart: bool,
    pub dmx_pending_restart: bool,
    /// Output surfaces, clocks, personalities, and DMX ingress are created once at startup.
    pub takes_effect_on_restart: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct OutputConfigurationValuesView {
    pub target_kind: String,
    pub monitor_by: Option<String>,
    pub monitor_value: Option<String>,
    pub fullscreen: bool,
    pub width: u32,
    pub height: u32,
    pub presentation: String,
    pub frames_per_second: Option<f64>,
    pub sound_output_kind: String,
    pub sound_output_name: Option<String>,
    pub personality: String,
    pub personality_layout: String,
    pub protocol: String,
    pub universe: u16,
    pub start_address: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct AvailableMonitorView {
    pub index: u32,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub refresh_millihertz: Option<u32>,
}

impl OutputConfigurationView {
    pub fn of(
        output: &OutputConfiguration,
        active: &OutputConfiguration,
        available_monitors: Vec<crate::diagnostics::MonitorDevice>,
        available_sound_outputs: Vec<String>,
    ) -> Self {
        let values = OutputConfigurationValuesView::of(output);
        let active = OutputConfigurationValuesView::of(active);
        let picture_pending_restart = values.picture_differs(&active);
        let sound_pending_restart = values.sound_differs(&active);
        let dmx_pending_restart = values.dmx_differs(&active);

        Self {
            id: output.id.to_string(),
            name: output.name.to_string(),
            target_kind: values.target_kind.clone(),
            monitor_by: values.monitor_by.clone(),
            monitor_value: values.monitor_value.clone(),
            fullscreen: values.fullscreen,
            width: values.width,
            height: values.height,
            presentation: values.presentation.clone(),
            frames_per_second: values.frames_per_second,
            sound_output_kind: values.sound_output_kind.clone(),
            sound_output_name: values.sound_output_name.clone(),
            available_monitors: available_monitors
                .into_iter()
                .map(|monitor| AvailableMonitorView {
                    index: monitor.index,
                    name: monitor.name,
                    width: monitor.width,
                    height: monitor.height,
                    refresh_millihertz: monitor.refresh_millihertz,
                })
                .collect(),
            available_sound_outputs,
            personality: values.personality.clone(),
            personality_layout: values.personality_layout.clone(),
            protocol: values.protocol.clone(),
            universe: values.universe,
            start_address: values.start_address,
            active,
            picture_pending_restart,
            sound_pending_restart,
            dmx_pending_restart,
            takes_effect_on_restart: true,
        }
    }
}

impl OutputConfigurationValuesView {
    fn of(output: &OutputConfiguration) -> Self {
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
            target_kind: target_kind.to_owned(),
            monitor_by,
            monitor_value,
            fullscreen,
            width: output.resolution.width,
            height: output.resolution.height,
            presentation: presentation.to_owned(),
            frames_per_second,
            sound_output_kind: match &output.sound_output {
                SoundOutput::Disabled => "disabled",
                SoundOutput::SystemDefault => "system-default",
                SoundOutput::Device { .. } => "device",
            }
            .to_owned(),
            sound_output_name: match &output.sound_output {
                SoundOutput::Device { name } => Some(name.clone()),
                SoundOutput::Disabled | SoundOutput::SystemDefault => None,
            },
            personality: match output.personality {
                LayerPersonality::TwoLayers => "two-layers",
                LayerPersonality::EightLayers => "eight-layers",
            }
            .to_owned(),
            personality_layout: match output.personality_layout {
                media_domain::PersonalityLayout::Legacy => "legacy",
                media_domain::PersonalityLayout::Current => "current",
                media_domain::PersonalityLayout::Extended => "extended",
            }
            .to_owned(),
            protocol: match output.protocol {
                DmxProtocol::ArtNet => "art-net",
                DmxProtocol::Sacn => "sacn",
            }
            .to_owned(),
            universe: output.universe,
            start_address: output.start_address,
        }
    }

    fn picture_differs(&self, active: &Self) -> bool {
        self.target_kind != active.target_kind
            || self.monitor_by != active.monitor_by
            || self.monitor_value != active.monitor_value
            || self.fullscreen != active.fullscreen
            || self.width != active.width
            || self.height != active.height
            || self.presentation != active.presentation
            || self.frames_per_second != active.frames_per_second
    }

    fn sound_differs(&self, active: &Self) -> bool {
        self.sound_output_kind != active.sound_output_kind
            || self.sound_output_name != active.sound_output_name
    }

    fn dmx_differs(&self, active: &Self) -> bool {
        self.personality != active.personality
            || self.personality_layout != active.personality_layout
            || self.protocol != active.protocol
            || self.universe != active.universe
            || self.start_address != active.start_address
    }
}

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub play_mode_dmx: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale_x: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale_y: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scaling_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position_x: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position_y: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tint_red: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tint_green: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tint_blue: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grayscale: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask_folder: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask_file: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask_scale_x: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask_scale_y: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask_position_x: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask_position_y: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask_invert: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask_opacity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed_multiplier_dmx: Option<u8>,
    /// Zero disables the per-layer BPM target; 1..=255 selects a target.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub playback_bpm: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blur: Option<f32>,
    /// The ordered slot changed by the following typed effect fields, `0..=3`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect_slot: Option<u8>,
    /// `analog-tv` or `digital-tv` selects the effect; `none` clears the slot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect_mix: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tv_curvature: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect_distortion: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_grain: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compression_damage: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_size: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tile_displacement: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chroma_damage: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect_glitching: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blur_amount: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedback_amount: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedback_motion: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedback_direction: Option<String>,
    /// `every-beat`, `every-half-beat`, or `every-second` for the opacity cycle effect.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cycle_interval: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beat_move_amount: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beat_move_direction: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beat_move_decay: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kaleidoscope_repetitions: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kaleidoscope_angle: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rasterize_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rasterize_dot_size: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beat_scan_width: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beat_scan_edge: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beat_scan_falloff: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beat_scan_duration: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beat_scale_amount: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beat_turn_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beat_turn_rotation: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beat_scale_decay: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beat_grid_density: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beat_grid_height: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beat_grid_duration: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beat_grid_origin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beat_grid_hue: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beat_grid_brightness: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beat_form_enlargement: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beat_form_lifetime: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beat_form_density: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beat_form_variation: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drawn_strength: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drawn_line_detail: Option<f32>,
    /// Complete per-layer visualizer settings routed through effect slot one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visualizer_parameters: Option<VisualizerParametersView>,
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

    pub const fn changes_effect(&self) -> bool {
        self.effect_type.is_some()
            || self.effect_enabled.is_some()
            || self.effect_mix.is_some()
            || self.tv_curvature.is_some()
            || self.effect_distortion.is_some()
            || self.image_grain.is_some()
            || self.compression_damage.is_some()
            || self.block_size.is_some()
            || self.tile_displacement.is_some()
            || self.chroma_damage.is_some()
            || self.effect_glitching.is_some()
            || self.blur_amount.is_some()
            || self.feedback_amount.is_some()
            || self.feedback_motion.is_some()
            || self.feedback_direction.is_some()
            || self.cycle_interval.is_some()
            || self.beat_move_amount.is_some()
            || self.beat_move_direction.is_some()
            || self.beat_move_decay.is_some()
            || self.kaleidoscope_repetitions.is_some()
            || self.kaleidoscope_angle.is_some()
            || self.rasterize_mode.is_some()
            || self.rasterize_dot_size.is_some()
            || self.beat_scan_width.is_some()
            || self.beat_scan_edge.is_some()
            || self.beat_scan_falloff.is_some()
            || self.beat_scan_duration.is_some()
            || self.beat_scale_amount.is_some()
            || self.beat_turn_enabled.is_some()
            || self.beat_turn_rotation.is_some()
            || self.beat_scale_decay.is_some()
            || self.beat_grid_density.is_some()
            || self.beat_grid_height.is_some()
            || self.beat_grid_duration.is_some()
            || self.beat_grid_origin.is_some()
            || self.beat_grid_hue.is_some()
            || self.beat_grid_brightness.is_some()
            || self.beat_form_enlargement.is_some()
            || self.beat_form_lifetime.is_some()
            || self.beat_form_density.is_some()
            || self.beat_form_variation.is_some()
            || self.drawn_strength.is_some()
            || self.drawn_line_detail.is_some()
            || self.visualizer_parameters.is_some()
    }

    pub const fn changes_non_effect(&self) -> bool {
        self.folder.is_some()
            || self.file.is_some()
            || self.dimmer.is_some()
            || self.play_mode_dmx.is_some()
            || self.scale_x.is_some()
            || self.scale_y.is_some()
            || self.scaling_mode.is_some()
            || self.position_x.is_some()
            || self.position_y.is_some()
            || self.rotation.is_some()
            || self.volume.is_some()
            || self.tint_red.is_some()
            || self.tint_green.is_some()
            || self.tint_blue.is_some()
            || self.grayscale.is_some()
            || self.mask_folder.is_some()
            || self.mask_file.is_some()
            || self.mask_scale_x.is_some()
            || self.mask_scale_y.is_some()
            || self.mask_position_x.is_some()
            || self.mask_position_y.is_some()
            || self.mask_invert.is_some()
            || self.mask_opacity.is_some()
            || self.speed_multiplier_dmx.is_some()
            || self.playback_bpm.is_some()
            || self.blur.is_some()
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMaster {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dimmer: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tint_red: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tint_green: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tint_blue: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flip_mirror: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask_folder: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask_file: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask_position_x: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask_position_y: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale_x: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale_y: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scaling_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position_x: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position_y: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shaper_left: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shaper_right: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shaper_top: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shaper_bottom: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shaper_left_rotation: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shaper_right_rotation: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shaper_top_rotation: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shaper_bottom_rotation: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shaper_rotation: Option<f32>,
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

    #[test]
    fn u8_playback_fields_require_json_integers_inside_the_byte_range() {
        let boundaries: UpdateLayer = serde_json::from_str(
			r#"{"folder":0,"file":255,"playModeDmx":216,"speedMultiplierDmx":255,"playbackBpm":120,"effectSlot":3,"volume":0.375}"#,
		)
		.unwrap();
        assert_eq!(boundaries.folder, Some(0));
        assert_eq!(boundaries.file, Some(255));
        assert_eq!(boundaries.play_mode_dmx, Some(216));
        assert_eq!(boundaries.speed_multiplier_dmx, Some(255));
        assert_eq!(boundaries.playback_bpm, Some(120));
        assert_eq!(boundaries.effect_slot, Some(3));
        assert_eq!(boundaries.volume, Some(0.375));

        for body in [
            r#"{"playbackBpm":120.1}"#,
            r#"{"speedMultiplierDmx":-1}"#,
            r#"{"folder":256}"#,
        ] {
            assert!(
                serde_json::from_str::<UpdateLayer>(body).is_err(),
                "{body} must not deserialize as a byte-backed update"
            );
        }
    }

    #[test]
    fn an_unknown_persisted_effect_reports_an_actionable_capability_error() {
        let effect = EffectSlot {
            effect_type: Some("future-effect".to_owned()),
            enabled: true,
            mix: 1.0,
            parameters: vec![0.4],
            seed: 7,
            visualizer_parameters: None,
        };
        let view = EffectSlotView::of(2, &effect);

        assert_eq!(view.label, "future-effect");
        assert!(!view.supported);
        assert_eq!(
            view.capability_detail.as_deref(),
            Some("This Media Server build cannot render the selected effect.")
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
            frames_per_second: 50.0,
        };
        output.sound_output = SoundOutput::Device {
            name: "Display 2".to_owned(),
        };
        let view = OutputConfigurationView::of(
            &output,
            &OutputConfiguration::new("Main"),
            vec![crate::diagnostics::MonitorDevice {
                index: 1,
                name: "Stage Right".to_owned(),
                width: 3840,
                height: 2160,
                refresh_millihertz: Some(59_940),
            }],
            vec!["Display 2".to_owned()],
        );

        assert_eq!(view.target_kind, "monitor");
        assert_eq!(view.monitor_by.as_deref(), Some("name"));
        assert_eq!(view.monitor_value.as_deref(), Some("Stage Right"));
        assert!(view.fullscreen);
        assert_eq!(view.presentation, "fixed-fps");
        assert_eq!(view.frames_per_second, Some(50.0));
        assert_eq!(view.available_monitors[0].width, 3840);
        assert_eq!(view.available_monitors[0].height, 2160);
        assert_eq!(view.sound_output_kind, "device");
        assert_eq!(view.sound_output_name.as_deref(), Some("Display 2"));
        assert_eq!(view.available_monitors[0].name, "Stage Right");
        assert_eq!(view.available_sound_outputs, vec!["Display 2"]);
        assert!(view.takes_effect_on_restart);
        assert!(view.picture_pending_restart);
        assert!(view.sound_pending_restart);
        assert!(!view.dmx_pending_restart);
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
            r#"{"requestId":"a","presentation":"fixed-fps","framesPerSecond":29.97}"#,
        )
        .applied(&current)
        .expect("accepted");
        assert_eq!(
            fixed.presentation,
            PresentationMode::FixedFps {
                frames_per_second: 29.97
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

    #[test]
    fn sound_output_edits_require_a_truthful_device_name() {
        let current = OutputConfiguration::new("Main");
        let device = configuration_edit(
            r#"{"requestId":"a","soundOutputKind":"device","soundOutputName":"Display 2"}"#,
        )
        .applied(&current)
        .expect("accepted");
        assert_eq!(
            device.sound_output,
            SoundOutput::Device {
                name: "Display 2".to_owned()
            }
        );

        let error = configuration_edit(r#"{"requestId":"b","soundOutputKind":"device"}"#)
            .applied(&current)
            .unwrap_err();
        assert_eq!(error, OutputConfigurationEditError::SoundOutputMissing);
    }
}
