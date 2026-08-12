//! The operator-facing projection of one configured output's canonical DMX map.
//!
//! Addresses and layer repetition belong to this HTTP adapter. Names, defaults, resolution,
//! decoded value sets, and implementation status come from `media-domain`'s channel table, so a
//! route and the frontend never carry a second personality definition.

use media_application::OutputConfiguration;
use media_domain::personality::channels::{
    ChannelSpec, ChannelValueSet, LAYER_CHANNELS, MASTER_CHANNELS, Resolution,
};
use media_domain::personality::{LAYER_SLOTS, LayerPersonality};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::diagnostics::DmxTelemetry;

/// The winning live ingress for one output, pushed with exact footprint bytes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct DmxIngressView {
    pub output_id: String,
    pub protocol: String,
    pub universe: u16,
    pub start_address: u16,
    pub source: String,
    pub frames_per_second: f32,
    #[ts(type = "number")]
    pub age_millis: u64,
    pub active: bool,
    pub slots: Vec<u8>,
}

impl DmxIngressView {
    pub fn of(telemetry: &DmxTelemetry) -> Self {
        Self {
            output_id: telemetry.output.to_string(),
            protocol: telemetry.protocol.clone(),
            universe: telemetry.universe,
            start_address: telemetry.start_address,
            source: telemetry.source.clone(),
            frames_per_second: telemetry.frames_per_second,
            age_millis: telemetry.age_millis,
            active: telemetry.active,
            slots: telemetry.slots.clone(),
        }
    }
}

/// One output's complete, absolute map in wire order: every layer followed by the master.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct DmxMapView {
    pub output_id: String,
    pub output_name: String,
    pub universe: u16,
    /// The configured one-based DMX address of the first layer slot.
    pub start_address: u16,
    pub personality: DmxPersonalityView,
    pub layer_count: u16,
    pub channels: Vec<DmxChannelView>,
}

impl DmxMapView {
    pub fn of(output: &OutputConfiguration) -> Self {
        let mut channels = Vec::with_capacity(usize::from(output.personality.footprint().total()));

        for layer_index in 0..output.personality.layer_count() {
            let block_offset = layer_index * LAYER_SLOTS;
            channels.extend(LAYER_CHANNELS.iter().map(|spec| {
                DmxChannelView::of(
                    spec,
                    output.start_address + block_offset + spec.offset,
                    DmxChannelGroupView::Layer {
                        // The operator-facing layer number is one-based.
                        number: layer_index + 1,
                    },
                )
            }));
        }

        let master_offset = output.personality.footprint().master_offset();
        channels.extend(MASTER_CHANNELS.iter().map(|spec| {
            DmxChannelView::of(
                spec,
                output.start_address + master_offset + spec.offset,
                DmxChannelGroupView::Master,
            )
        }));

        Self {
            output_id: output.id.to_string(),
            output_name: output.name.to_string(),
            universe: output.universe,
            start_address: output.start_address,
            personality: DmxPersonalityView::of(output.personality),
            layer_count: output.personality.layer_count(),
            channels,
        }
    }
}

/// The two supported patch products.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub enum DmxPersonalityView {
    TwoLayers,
    EightLayers,
}

impl DmxPersonalityView {
    const fn of(personality: LayerPersonality) -> Self {
        match personality {
            LayerPersonality::TwoLayers => Self::TwoLayers,
            LayerPersonality::EightLayers => Self::EightLayers,
        }
    }
}

/// Where a slot lives in the repeated personality.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum DmxChannelGroupView {
    Layer { number: u16 },
    Master,
}

/// One physical DMX slot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct DmxChannelView {
    /// The one-based universe address an operator patches.
    pub absolute_channel: u16,
    /// The canonical zero-based offset within a layer or master block.
    pub local_offset: u16,
    pub group: DmxChannelGroupView,
    pub name: String,
    pub resolution: DmxResolutionView,
    /// The complete logical default: 0..=255 for byte controls and 0..=65535 for coarse ones.
    pub default_value: u16,
    pub value_sets: Vec<DmxValueSetView>,
    pub implemented: bool,
    pub implementation_note: Option<String>,
}

impl DmxChannelView {
    fn of(spec: &ChannelSpec, absolute_channel: u16, group: DmxChannelGroupView) -> Self {
        Self {
            absolute_channel,
            local_offset: spec.offset,
            group,
            name: spec.name.to_owned(),
            resolution: DmxResolutionView::of(spec.resolution),
            default_value: spec.default_value,
            value_sets: spec
                .values
                .sets()
                .into_iter()
                .map(DmxValueSetView::of)
                .collect(),
            implemented: spec.implementation.is_implemented(),
            implementation_note: spec.implementation.reason().map(str::to_owned),
        }
    }
}

/// A slot's role in an 8- or 16-bit logical value.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub enum DmxResolutionView {
    Byte,
    Coarse,
    Fine,
}

impl DmxResolutionView {
    const fn of(resolution: Resolution) -> Self {
        match resolution {
            Resolution::Byte => Self::Byte,
            Resolution::Coarse => Self::Coarse,
            Resolution::Fine => Self::Fine,
        }
    }
}

/// An inclusive raw-value set. `step` is normally one; flip/mirror uses four because every raw
/// byte is decoded modulo four.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct DmxValueSetView {
    pub name: String,
    pub from: u16,
    pub to: u16,
    pub step: u16,
    pub implemented: bool,
}

impl DmxValueSetView {
    fn of(set: ChannelValueSet) -> Self {
        Self {
            name: set.name,
            from: set.from,
            to: set.to,
            step: set.step,
            implemented: set.implemented,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use media_domain::personality::{LAYER_SLOTS, MASTER_SLOTS};

    fn configured_output() -> OutputConfiguration {
        let mut output = OutputConfiguration::new("Stage Left");
        output.personality = LayerPersonality::TwoLayers;
        output.universe = 7;
        output.start_address = 45;
        output
    }

    #[test]
    fn the_projection_repeats_layers_then_places_the_master_absolutely() {
        let view = DmxMapView::of(&configured_output());
        assert_eq!(view.universe, 7);
        assert_eq!(view.start_address, 45);
        assert_eq!(view.personality, DmxPersonalityView::TwoLayers);
        assert_eq!(view.layer_count, 2);
        assert_eq!(
            view.channels.len(),
            usize::from(2 * LAYER_SLOTS + MASTER_SLOTS)
        );

        let first = &view.channels[0];
        assert_eq!(first.absolute_channel, 45);
        assert_eq!(first.local_offset, 0);
        assert_eq!(first.group, DmxChannelGroupView::Layer { number: 1 });
        assert_eq!(first.name, "Folder");

        let second_layer = &view.channels[usize::from(LAYER_SLOTS)];
        assert_eq!(second_layer.absolute_channel, 45 + LAYER_SLOTS);
        assert_eq!(second_layer.group, DmxChannelGroupView::Layer { number: 2 });

        let master = &view.channels[usize::from(2 * LAYER_SLOTS)];
        assert_eq!(master.absolute_channel, 45 + 2 * LAYER_SLOTS);
        assert_eq!(master.local_offset, 0);
        assert_eq!(master.group, DmxChannelGroupView::Master);
        assert_eq!(master.name, "Master dimmer");

        assert_eq!(
            view.channels.last().unwrap().absolute_channel,
            45 + 2 * LAYER_SLOTS + MASTER_SLOTS - 1
        );
    }

    #[test]
    fn canonical_defaults_value_sets_and_resolution_reach_the_wire() {
        let view = DmxMapView::of(&configured_output());
        let first_layer = &view.channels[..usize::from(LAYER_SLOTS)];

        let scale = &first_layer[3];
        assert_eq!(scale.resolution, DmxResolutionView::Coarse);
        assert_eq!(scale.default_value, 32_768);
        assert_eq!(first_layer[4].resolution, DmxResolutionView::Fine);

        let play = &first_layer[2];
        assert_eq!(play.value_sets.first().unwrap().name, "Loop");
        assert_eq!((play.value_sets[0].from, play.value_sets[0].to), (0, 19));
        assert_eq!(play.value_sets.last().unwrap().name, "Pause");

        let scaling = &first_layer[7];
        assert_eq!(
            scaling
                .value_sets
                .iter()
                .map(|set| set.name.as_str())
                .collect::<Vec<_>>(),
            ["Fit", "Fill", "Original", "Stretch"]
        );

        let flip = view
            .channels
            .iter()
            .find(|channel| channel.name == "Flip/mirror")
            .unwrap();
        assert_eq!(flip.value_sets.len(), 4);
        assert!(flip.value_sets.iter().all(|set| set.step == 4));
    }

    #[test]
    fn every_repeated_effect_slot_is_an_implemented_mix_control() {
        let view = DmxMapView::of(&configured_output());
        let effects: Vec<&DmxChannelView> = view
            .channels
            .iter()
            .filter(|channel| channel.name.starts_with("Effect "))
            .collect();
        assert_eq!(effects.len(), 8, "four slots on each of two layers");
        for effect in effects {
            assert!(effect.implemented);
            assert!(effect.implementation_note.is_none());
            assert!(effect.value_sets.is_empty());
        }
    }

    #[test]
    fn the_group_is_a_typed_wire_discriminator() {
        let view = DmxMapView::of(&configured_output());
        let layer = serde_json::to_value(view.channels[0].group).unwrap();
        assert_eq!(layer, serde_json::json!({ "kind": "layer", "number": 1 }));
        let master = serde_json::to_value(DmxChannelGroupView::Master).unwrap();
        assert_eq!(master, serde_json::json!({ "kind": "master" }));
    }
}
