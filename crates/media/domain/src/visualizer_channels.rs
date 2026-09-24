//! A layer's dedicated visualizer parameter channels, as the active visualizer defines them.
//!
//! The layer personality carries four Visualizer Parameter bytes of its own, separate from the two
//! effect banks. What each byte means depends on the visualizer the layer is showing: its kind
//! names the parameter, the range the byte spans, and the value the byte's zero keeps. A layer
//! showing ordinary media, and any byte past the parameters a kind offers, is inert.
//!
//! The desk's Media pane, the web media controls, and DMX input all read this one definition, so
//! a fader labelled "Size" on any of them moves the same value.

use serde::{Deserialize, Serialize};

use crate::address::MediaAddress;
use crate::personality::VISUALIZER_PARAMETERS;
use crate::visualizer::{Parameter, VisualizerKind, VisualizerParameters};

/// A layer's own tuning of the visualizer at one address.
///
/// It lives on the layer, never in an effect slot, so the effect banks stay ordinary effects while
/// a visualizer is shown. It applies only while the layer still shows that address.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VisualizerTuning {
    pub address: MediaAddress,
    pub parameters: VisualizerParameters,
}

impl VisualizerTuning {
    /// The parameter block a layer showing `address` draws with, before its channel bytes.
    pub fn resolve<'a>(
        tuning: Option<&'a Self>,
        address: MediaAddress,
        configured: &'a VisualizerParameters,
    ) -> &'a VisualizerParameters {
        tuning
            .filter(|tuning| tuning.address == address)
            .map_or(configured, |tuning| &tuning.parameters)
    }

    /// The complete parameter block a layer showing a visualizer draws with: its tuning (or the
    /// configured block), then its Visualizer Parameter bytes.
    pub fn effective(
        tuning: Option<&Self>,
        address: MediaAddress,
        kind: VisualizerKind,
        configured: &VisualizerParameters,
        bytes: &[u8],
    ) -> VisualizerParameters {
        Self::resolve(tuning, address, configured).with_dmx(kind, bytes)
    }
}

/// One Visualizer Parameter channel as the active visualizer defines it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VisualizerChannel {
    /// Zero-based position among the layer's visualizer parameter bytes.
    pub index: usize,
    pub parameter: Parameter,
    /// The operator-facing name the active visualizer gives this channel.
    pub label: &'static str,
    /// The value byte 1 selects.
    pub minimum: f32,
    /// The value byte 255 selects.
    pub maximum: f32,
    /// One or more marks a whole-number, switch, or hue value.
    pub step: f32,
}

impl VisualizerChannel {
    /// The value a raw byte selects, or `None` for zero, which keeps the default.
    pub fn value_of(self, raw: u8) -> Option<f32> {
        if raw == 0 {
            return None;
        }
        let fraction = f32::from(raw - 1) / 254.0;
        Some(match self.parameter {
            Parameter::Mirror | Parameter::Filled | Parameter::Wireframe | Parameter::OnBeat => {
                f32::from(u8::from(raw >= 128))
            }
            Parameter::Mode => f32::from(raw - 1),
            Parameter::Count | Parameter::Iterations | Parameter::Burst => {
                (self.minimum + fraction * (self.maximum - self.minimum)).round()
            }
            _ => self.minimum + fraction * (self.maximum - self.minimum),
        })
    }

    /// The value byte zero keeps: the layer's own tuning, else the configured visualizer's.
    ///
    /// Colours report their hue in degrees and switches report zero or one.
    pub fn default_value(self, parameters: &VisualizerParameters) -> f32 {
        read(self.parameter, parameters)
    }

    /// Writes a value this channel selected into a parameter block.
    pub fn apply(self, parameters: &mut VisualizerParameters, value: f32) {
        write(self.parameter, parameters, value);
    }
}

/// The most props Flying Props draws, whatever its Density says.
pub const FLYING_PROPS_MOST: u32 = 48;

/// The flight patterns Flying Props cycles through as its Flight pattern value counts up.
pub const FLYING_PROPS_PATTERNS: [&str; 4] = ["Fly-through", "Drift", "Orbit", "Rise"];

impl VisualizerKind {
    /// The layer's visualizer parameter channels this kind uses, in byte order.
    ///
    /// The bytes follow the kind's own parameter order, so a cue that stored a byte keeps the
    /// parameter it moved. A kind with fewer parameters than bytes leaves the rest inert.
    pub fn channels(self) -> impl Iterator<Item = VisualizerChannel> {
        self.parameters()
            .iter()
            .take(VISUALIZER_PARAMETERS)
            .enumerate()
            .map(move |(index, &parameter)| {
                let (minimum, maximum) = self.range(parameter);
                VisualizerChannel {
                    index,
                    parameter,
                    label: self.parameter_label(parameter),
                    minimum,
                    maximum,
                    step: parameter.step(),
                }
            })
    }

    /// The channel at a byte position, or `None` where this kind leaves the byte inert.
    pub fn channel(self, index: usize) -> Option<VisualizerChannel> {
        self.channels().nth(index)
    }

    /// The name this kind gives a parameter. Equalizer Bars calls its Amount "Bloom".
    pub const fn parameter_label(self, parameter: Parameter) -> &'static str {
        match (self, parameter) {
            (Self::EqualizerBars, Parameter::Amount) => "Bloom",
            (Self::FlyingProps, Parameter::Mode) => "Flight pattern",
            (Self::FlyingProps, Parameter::Count) => "Density",
            _ => parameter.label(),
        }
    }

    /// The range this kind gives a parameter. A waveform wider than a tenth of the picture is
    /// no longer a line, so Waveform Oscilloscope narrows Size.
    pub const fn range(self, parameter: Parameter) -> (f32, f32) {
        match (self, parameter) {
            (Self::WaveformOscilloscope, Parameter::Size) => (0.005, 0.1),
            // More props than this crowd the picture into noise and cost every pixel a test each.
            (Self::FlyingProps, Parameter::Count) => (1.0, FLYING_PROPS_MOST as f32),
            (Self::FlyingProps, Parameter::Size) => (0.02, 0.5),
            _ => parameter.range(),
        }
    }
}

impl Parameter {
    /// The generic operator-facing name.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Count => "Count",
            Self::Size => "Size",
            Self::Speed => "Speed",
            Self::Amount => "Amount",
            Self::Radius => "Radius",
            Self::Thickness => "Thickness",
            Self::Reactivity => "Reactivity",
            Self::Decay => "Decay",
            Self::Zoom => "Zoom",
            Self::Iterations => "Iterations",
            Self::Threshold => "Threshold",
            Self::Smoothing => "Smoothing",
            Self::Gravity => "Gravity",
            Self::Lifetime => "Lifetime",
            Self::Curvature => "Curvature",
            Self::Primary => "Colour",
            Self::Secondary => "Second colour",
            Self::Mirror => "Mirror",
            Self::Filled => "Filled",
            Self::Wireframe => "Wireframe",
            Self::Mode => "Variant",
            Self::OnBeat => "React to beat",
            Self::Burst => "Per beat",
        }
    }

    /// The accepted range, matching [`VisualizerParameters::clamped`]. Colours span the hue
    /// wheel in degrees, switches are off or on, and Variant counts 0–254.
    pub const fn range(self) -> (f32, f32) {
        match self {
            Self::Count => (1.0, 512.0),
            Self::Size => (0.001, 1.0),
            Self::Speed | Self::Reactivity => (0.0, 8.0),
            Self::Amount
            | Self::Radius
            | Self::Decay
            | Self::Threshold
            | Self::Smoothing
            | Self::Curvature => (0.0, 1.0),
            Self::Thickness => (0.0005, 0.5),
            Self::Zoom => (0.05, 16.0),
            Self::Iterations => (1.0, 256.0),
            Self::Gravity => (-4.0, 4.0),
            Self::Lifetime => (0.05, 60.0),
            Self::Primary | Self::Secondary => (0.0, 360.0),
            Self::Mirror | Self::Filled | Self::Wireframe | Self::OnBeat => (0.0, 1.0),
            Self::Mode => (0.0, 254.0),
            Self::Burst => (0.0, crate::visualizer::MAXIMUM_BURST as f32),
        }
    }

    const fn step(self) -> f32 {
        match self {
            Self::Count
            | Self::Iterations
            | Self::Mode
            | Self::Primary
            | Self::Secondary
            | Self::Mirror
            | Self::Filled
            | Self::Wireframe
            | Self::OnBeat
            | Self::Burst => 1.0,
            _ => 0.001,
        }
    }
}

fn read(parameter: Parameter, parameters: &VisualizerParameters) -> f32 {
    match parameter {
        Parameter::Count => parameters.count as f32,
        Parameter::Size => parameters.size,
        Parameter::Speed => parameters.speed,
        Parameter::Amount => parameters.amount,
        Parameter::Radius => parameters.radius,
        Parameter::Thickness => parameters.thickness,
        Parameter::Reactivity => parameters.reactivity,
        Parameter::Decay => parameters.decay,
        Parameter::Zoom => parameters.zoom,
        Parameter::Iterations => parameters.iterations as f32,
        Parameter::Threshold => parameters.threshold,
        Parameter::Smoothing => parameters.smoothing,
        Parameter::Gravity => parameters.gravity,
        Parameter::Lifetime => parameters.lifetime,
        Parameter::Curvature => parameters.curvature,
        Parameter::Primary => parameters.primary.hue_degrees(),
        Parameter::Secondary => parameters.secondary.hue_degrees(),
        Parameter::Mirror => f32::from(u8::from(parameters.mirror)),
        Parameter::Filled => f32::from(u8::from(parameters.filled)),
        Parameter::Wireframe => f32::from(u8::from(parameters.wireframe)),
        Parameter::Mode => f32::from(parameters.mode),
        Parameter::OnBeat => f32::from(u8::from(parameters.on_beat)),
        Parameter::Burst => parameters.burst as f32,
    }
}

fn write(parameter: Parameter, parameters: &mut VisualizerParameters, value: f32) {
    match parameter {
        Parameter::Count => parameters.count = value.round().max(0.0) as u32,
        Parameter::Size => parameters.size = value,
        Parameter::Speed => parameters.speed = value,
        Parameter::Amount => parameters.amount = value,
        Parameter::Radius => parameters.radius = value,
        Parameter::Thickness => parameters.thickness = value,
        Parameter::Reactivity => parameters.reactivity = value,
        Parameter::Decay => parameters.decay = value,
        Parameter::Zoom => parameters.zoom = value,
        Parameter::Iterations => parameters.iterations = value.round().max(0.0) as u32,
        Parameter::Threshold => parameters.threshold = value,
        Parameter::Smoothing => parameters.smoothing = value,
        Parameter::Gravity => parameters.gravity = value,
        Parameter::Lifetime => parameters.lifetime = value,
        Parameter::Curvature => parameters.curvature = value,
        Parameter::Primary => parameters.primary = crate::color::Tint::from_hue(value),
        Parameter::Secondary => parameters.secondary = crate::color::Tint::from_hue(value),
        Parameter::Mirror => parameters.mirror = value >= 0.5,
        Parameter::Filled => parameters.filled = value >= 0.5,
        Parameter::Wireframe => parameters.wireframe = value >= 0.5,
        Parameter::Mode => parameters.mode = value.round().clamp(0.0, 255.0) as u8,
        Parameter::OnBeat => parameters.on_beat = value >= 0.5,
        Parameter::Burst => parameters.burst = value.round().max(0.0) as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::visualizer::{ALL_KINDS, VisualizerConfiguration};

    #[test]
    fn every_kind_names_at_most_four_channels_and_leaves_the_rest_inert() {
        for kind in ALL_KINDS {
            let channels: Vec<_> = kind.channels().collect();
            assert_eq!(
                channels.len(),
                kind.parameters().len().min(VISUALIZER_PARAMETERS)
            );
            for (index, channel) in channels.iter().enumerate() {
                assert_eq!(channel.index, index);
                assert_eq!(channel.parameter, kind.parameters()[index]);
                assert!(channel.minimum < channel.maximum, "{}", channel.label);
            }
            for index in channels.len()..VISUALIZER_PARAMETERS {
                assert_eq!(kind.channel(index), None, "{} byte {index}", kind.label());
            }
        }
        // Color Cycling offers two parameters, so its last two bytes do nothing.
        assert_eq!(VisualizerKind::ColorCycling.channels().count(), 2);
        let configured = VisualizerParameters::default();
        assert_eq!(
            configured.with_dmx(VisualizerKind::ColorCycling, &[0, 0, 255, 255]),
            configured.clamped()
        );
    }

    #[test]
    fn the_active_kind_names_and_ranges_its_channels() {
        let waveform = VisualizerKind::WaveformOscilloscope.channel(0).unwrap();
        assert_eq!(
            (waveform.label, waveform.minimum, waveform.maximum),
            ("Size", 0.005, 0.1)
        );
        let bars = VisualizerKind::EqualizerBars;
        assert_eq!(bars.parameter_label(Parameter::Amount), "Bloom");
        assert_eq!(bars.channel(0).unwrap().label, "Count");
        assert_eq!(bars.channel(2).unwrap().label, "Colour");
    }

    #[test]
    fn flying_props_puts_pattern_speed_density_and_size_on_its_four_channels() {
        let kind = VisualizerKind::FlyingProps;
        let labels: Vec<_> = kind.channels().map(|channel| channel.label).collect();
        assert_eq!(labels, ["Flight pattern", "Speed", "Density", "Size"]);

        let density = kind.channel(2).unwrap();
        assert_eq!((density.minimum, density.maximum), (1.0, 48.0));
        let driven = VisualizerConfiguration::new(kind)
            .parameters
            .with_dmx(kind, &[3, 0, 255, 1]);
        assert_eq!(driven.mode, 2, "byte 3 selects the third pattern, Orbit");
        assert_eq!(FLYING_PROPS_PATTERNS[usize::from(driven.mode)], "Orbit");
        assert_eq!(driven.count, 48);
        assert!((driven.size - 0.02).abs() < 1e-6);
        assert_eq!(driven.speed, 1.0, "a zero byte keeps the configured speed");
    }

    #[test]
    fn byte_zero_keeps_the_default_and_the_bytes_span_the_channel_range() {
        let channel = VisualizerKind::WaveformOscilloscope.channel(0).unwrap();
        assert_eq!(channel.value_of(0), None);
        assert_eq!(channel.value_of(1), Some(0.005));
        assert_eq!(channel.value_of(255), Some(0.1));
        let driven = VisualizerParameters::default()
            .with_dmx(VisualizerKind::WaveformOscilloscope, &[255, 0, 0, 0]);
        assert!((driven.size - 0.1).abs() < 1e-6);

        let configuration = VisualizerConfiguration::new(VisualizerKind::MatrixDigitalRain);
        let count = VisualizerKind::MatrixDigitalRain.channel(0).unwrap();
        assert_eq!(count.default_value(&configuration.parameters), 48.0);
    }

    #[test]
    fn a_tuning_applies_only_to_the_address_it_was_made_for() {
        let configured = VisualizerParameters::default();
        let here = MediaAddress::new(250, 1);
        let tuning = VisualizerTuning {
            address: here,
            parameters: VisualizerParameters {
                size: 0.2,
                ..configured
            },
        };
        assert_eq!(
            VisualizerTuning::resolve(Some(&tuning), here, &configured).size,
            0.2
        );
        assert_eq!(
            VisualizerTuning::resolve(Some(&tuning), MediaAddress::new(250, 2), &configured),
            &configured
        );
        assert_eq!(
            VisualizerTuning::resolve(None, here, &configured),
            &configured
        );
        let driven = VisualizerTuning::effective(
            Some(&tuning),
            here,
            VisualizerKind::EqualizerBars,
            &configured,
            &[0, 0, 0, 0],
        );
        assert_eq!(driven.size, 0.2, "zero bytes keep the tuned value");
    }

    #[test]
    fn a_channel_value_written_back_reads_the_same() {
        for kind in ALL_KINDS {
            for channel in kind.channels() {
                let mut parameters = VisualizerParameters::default();
                let value = channel.value_of(200).unwrap();
                channel.apply(&mut parameters, value);
                let read = channel.default_value(&parameters.clamped());
                assert!(
                    (read - value).abs() < 0.01 * (channel.maximum - channel.minimum).max(1.0),
                    "{} {}: wrote {value}, read {read}",
                    kind.label(),
                    channel.label
                );
            }
        }
    }
}
