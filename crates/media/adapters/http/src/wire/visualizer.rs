//! The generated visualizers and the edit that tunes one.

use media_domain::MediaAddress;
use media_domain::Tint;
use media_domain::visualizer::{GeneratedCatalog, VisualizerConfiguration, VisualizerParameters};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::AddressView;

/// One configured generated visualizer at the address a desk reaches it by.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct VisualizerView {
    pub address: AddressView,
    /// Stable across releases and across a reassignment of the address.
    pub type_id: u16,
    /// The kind's own name, which is what documentation and a cue sheet call it.
    pub kind: String,
    /// What this configuration is called, which an operator may change.
    pub name: String,
    /// Which of the shared parameters this kind reads. The rest are present and ignored, so an
    /// editor can show only the controls that do something.
    pub uses: Vec<String>,
    pub parameters: VisualizerParametersView,
    /// The layer's four Visualizer Parameter channels as this visualizer defines them, in byte
    /// order. A byte past the last entry is inert while this visualizer is shown.
    pub channels: Vec<VisualizerChannelView>,
}

/// One dedicated Visualizer Parameter channel as the active visualizer defines it.
///
/// The desk, the web media controls, and DMX input all address this same byte.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct VisualizerChannelView {
    /// Zero-based byte position, `0..=3`.
    pub index: u8,
    /// The shared parameter this byte moves, such as `size` or `primary`.
    pub parameter: String,
    /// What the active visualizer calls it.
    pub label: String,
    /// The value byte 1 selects. Colours are hue degrees; switches are 0 or 1.
    pub minimum: f32,
    /// The value byte 255 selects.
    pub maximum: f32,
    /// One or more marks a whole-number, switch, or hue value.
    pub step: f32,
    /// The value byte zero keeps: the layer's tuning, else the configured visualizer's.
    pub default_value: f32,
    /// The raw byte currently received or set; zero keeps the default.
    pub raw: u8,
    /// The value in effect on this channel.
    pub value: f32,
}

impl VisualizerChannelView {
    /// The channels `kind` defines, with defaults from `parameters` and the current bytes.
    pub fn all(
        kind: media_domain::VisualizerKind,
        parameters: &VisualizerParameters,
        bytes: &[u8],
    ) -> Vec<Self> {
        kind.channels()
            .map(|channel| {
                let raw = bytes.get(channel.index).copied().unwrap_or(0);
                let default_value = channel.default_value(parameters);
                Self {
                    index: channel.index as u8,
                    parameter: parameter_name(channel.parameter),
                    label: channel.label.to_owned(),
                    minimum: channel.minimum,
                    maximum: channel.maximum,
                    step: channel.step,
                    default_value,
                    raw,
                    value: channel.value_of(raw).unwrap_or(default_value),
                }
            })
            .collect()
    }
}

fn parameter_name(parameter: media_domain::Parameter) -> String {
    serde_json::to_value(parameter)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// The shared parameter block, as the API reports it.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct VisualizerParametersView {
    pub count: u32,
    pub size: f32,
    pub speed: f32,
    pub amount: f32,
    pub radius: f32,
    pub thickness: f32,
    pub reactivity: f32,
    pub decay: f32,
    pub zoom: f32,
    pub iterations: u32,
    pub threshold: f32,
    pub smoothing: f32,
    pub gravity: f32,
    pub lifetime: f32,
    pub curvature: f32,
    pub primary_red: f32,
    pub primary_green: f32,
    pub primary_blue: f32,
    pub secondary_red: f32,
    pub secondary_green: f32,
    pub secondary_blue: f32,
    pub mirror: bool,
    pub filled: bool,
    pub wireframe: bool,
    pub mode: u8,
}

impl VisualizerParametersView {
    pub fn of(parameters: &VisualizerParameters) -> Self {
        Self {
            count: parameters.count,
            size: parameters.size,
            speed: parameters.speed,
            amount: parameters.amount,
            radius: parameters.radius,
            thickness: parameters.thickness,
            reactivity: parameters.reactivity,
            decay: parameters.decay,
            zoom: parameters.zoom,
            iterations: parameters.iterations,
            threshold: parameters.threshold,
            smoothing: parameters.smoothing,
            gravity: parameters.gravity,
            lifetime: parameters.lifetime,
            curvature: parameters.curvature,
            primary_red: parameters.primary.red,
            primary_green: parameters.primary.green,
            primary_blue: parameters.primary.blue,
            secondary_red: parameters.secondary.red,
            secondary_green: parameters.secondary.green,
            secondary_blue: parameters.secondary.blue,
            mirror: parameters.mirror,
            filled: parameters.filled,
            wireframe: parameters.wireframe,
            mode: parameters.mode,
        }
    }
}

impl VisualizerView {
    pub fn of(address: MediaAddress, configuration: &VisualizerConfiguration) -> Self {
        Self {
            address: AddressView::of(address),
            type_id: configuration.kind.type_id(),
            kind: configuration.kind.label().to_owned(),
            name: configuration.name.clone(),
            uses: configuration
                .kind
                .parameters()
                .iter()
                .copied()
                .map(parameter_name)
                .collect(),
            parameters: VisualizerParametersView::of(&configuration.parameters),
            channels: VisualizerChannelView::all(
                configuration.kind,
                &configuration.parameters,
                &[],
            ),
        }
    }

    /// Every assignment, in address order.
    pub fn all(catalog: &GeneratedCatalog) -> Vec<Self> {
        catalog
            .entries
            .iter()
            .map(|entry| Self::of(entry.address, &entry.configuration))
            .collect()
    }
}

/// An intent-shaped visualizer edit: only the fields being changed.
///
/// This edits stored configuration rather than live state, so it carries a request id: a dropped
/// response must never become a second edit.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct UpdateVisualizer {
    /// Client-generated. A resend with the same id returns the first outcome.
    pub request_id: String,
    /// Changing kind resets the instance to that built-in's safe default parameters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_id: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<VisualizerParametersView>,
}

/// Creates another independently tunable instance of one shipped visualizer kind.
///
/// The address is explicit because an empty pool slot is an operator-selected stable identity.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct CreateVisualizer {
    /// Client-generated. A resend with the same id returns the first outcome.
    pub request_id: String,
    pub folder: u8,
    pub file: u8,
    /// The stable built-in kind id published by [`VisualizerView::type_id`].
    pub type_id: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl VisualizerParametersView {
    /// The domain parameters this view describes, clamped to what a renderer can use.
    pub fn into_parameters(self) -> VisualizerParameters {
        VisualizerParameters {
            count: self.count,
            size: self.size,
            speed: self.speed,
            amount: self.amount,
            radius: self.radius,
            thickness: self.thickness,
            reactivity: self.reactivity,
            decay: self.decay,
            zoom: self.zoom,
            iterations: self.iterations,
            threshold: self.threshold,
            smoothing: self.smoothing,
            gravity: self.gravity,
            lifetime: self.lifetime,
            curvature: self.curvature,
            primary: Tint::new(self.primary_red, self.primary_green, self.primary_blue),
            secondary: Tint::new(
                self.secondary_red,
                self.secondary_green,
                self.secondary_blue,
            ),
            mirror: self.mirror,
            filled: self.filled,
            wireframe: self.wireframe,
            mode: self.mode,
        }
        .clamped()
    }
}
