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
                .map(|parameter| {
                    serde_json::to_value(parameter)
                        .ok()
                        .and_then(|value| value.as_str().map(str::to_owned))
                        .unwrap_or_default()
                })
                .collect(),
            parameters: VisualizerParametersView::of(&configuration.parameters),
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<VisualizerParametersView>,
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
