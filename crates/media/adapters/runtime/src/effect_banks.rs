use media_application::MediaConfiguration;
use media_domain::{EffectBankState, EffectLibrary, LayerState, OutputState};

/// Resolves the two lightweight effect-bank selectors into the typed renderer effect chain.
///
/// A layout that carries the banks on the wire always plays them. An older layout keeps its four
/// directly configured slots, unless the operator selected a bank from the web surface: that
/// selection is shown as active there, so it must reach the output rather than be ignored.
pub(crate) fn resolve_output(
    output: &OutputState,
    configuration: &MediaConfiguration,
) -> OutputState {
    let Some(configured) = configuration.output(output.id) else {
        return output.clone();
    };
    let wire_banks = configured.personality_layout.carries_effect_banks();
    let mut resolved = output.clone();
    for (layer_index, layer) in resolved.layers.iter_mut().enumerate() {
        if wire_banks || layer.effect_banks.iter().any(|bank| bank.select != 0) {
            resolve_layer(layer, &configuration.effects, layer_index);
        }
    }
    resolved
}

fn resolve_layer(layer: &mut LayerState, library: &EffectLibrary, layer_index: usize) {
    layer.effects = Default::default();
    for (bank_index, bank) in layer.effect_banks.iter().enumerate() {
        let Some(preset) = library.resolve(bank.select) else {
            continue;
        };
        let mut effect = preset.effect.clone();
        effect.enabled = true;
        effect.mix = bank.strength.clamp(0.0, 1.0);
        effect.seed = ((layer_index as u32) << 8) | bank_index as u32;
        apply_parameters(&mut effect, bank);
        effect.normalize();
        layer.effects[bank_index] = effect;
    }
}

/// Overrides the preset's stored parameters with every non-zero bank parameter byte.
fn apply_parameters(effect: &mut media_domain::EffectSlot, bank: &EffectBankState) {
    let Some(effect_type) = effect.effect_type.as_deref() else {
        return;
    };
    let ids = media_domain::effect_parameter_ids(effect_type);
    for (index, (id, raw)) in ids.iter().zip(bank.parameters).enumerate() {
        let Some(value) = media_domain::effect_parameter_bounds(id).from_dmx(raw) else {
            continue;
        };
        if effect.parameters.len() <= index {
            // A preset stored before this parameter existed normalizes the gap to its default.
            let mut filled = effect.clone();
            filled.normalize();
            effect.parameters = filled.parameters;
        }
        if let Some(stored) = effect.parameters.get_mut(index) {
            *stored = value;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use media_domain::{LayerPersonality, PersonalityLayout};

    #[test]
    fn two_banks_resolve_in_order_and_zero_or_missing_is_off() {
        let mut configuration = MediaConfiguration::default();
        let id = configuration.outputs[0].id;
        let mut output = OutputState::new(id, LayerPersonality::TwoLayers);
        output.layers[0].effect_banks = [
            EffectBankState {
                select: 2,
                strength: 0.4,
                ..Default::default()
            },
            EffectBankState {
                select: 255,
                strength: 1.0,
                ..Default::default()
            },
        ];
        let resolved = resolve_output(&output, &configuration);
        assert_eq!(
            resolved.layers[0].effects[0].effect_type.as_deref(),
            Some("digital-tv")
        );
        assert_eq!(resolved.layers[0].effects[0].mix, 0.4);
        assert!(resolved.layers[0].effects[1].effect_type.is_none());

        configuration.outputs[0].personality_layout = PersonalityLayout::EffectBanks;
        assert_eq!(
            resolve_output(&output, &configuration).layers[0].effects[0]
                .effect_type
                .as_deref(),
            Some("digital-tv")
        );
    }

    /// The development seed and every configuration written before effect banks decode the
    /// legacy layout. An operator's bank selection from the web surface must still render there.
    #[test]
    fn a_selected_bank_renders_on_an_older_layout_and_configured_slots_stay_otherwise() {
        let mut configuration = MediaConfiguration::default();
        configuration.outputs[0].personality_layout = PersonalityLayout::Legacy;
        let id = configuration.outputs[0].id;
        let mut output = OutputState::new(id, LayerPersonality::TwoLayers);
        output.layers[1].effects[2] = media_domain::EffectSlot::blur();

        let untouched = resolve_output(&output, &configuration);
        assert_eq!(untouched.layers[1].effects, output.layers[1].effects);

        output.layers[0].effect_banks[0] = EffectBankState {
            select: 10,
            strength: 1.0,
            ..Default::default()
        };
        let resolved = resolve_output(&output, &configuration);
        assert_eq!(
            resolved.layers[0].effects[0].effect_type.as_deref(),
            Some("rasterize")
        );
        assert_eq!(resolved.layers[1].effects, output.layers[1].effects);
    }

    #[test]
    fn parameter_bytes_override_the_preset_and_zero_keeps_it() {
        let configuration = MediaConfiguration::default();
        let id = configuration.outputs[0].id;
        let mut output = OutputState::new(id, LayerPersonality::TwoLayers);
        // Preset 9 is the Kaleidoscope: repetitions, then angle.
        output.layers[0].effect_banks[0] = EffectBankState {
            select: 9,
            strength: 1.0,
            parameters: [255, 0, 0, 0],
        };
        let preset = configuration.effects.resolve(9).unwrap().effect.clone();

        let resolved = resolve_output(&output, &configuration);
        let effect = &resolved.layers[0].effects[0];
        assert_eq!(effect.parameters[0], 12.0, "255 is the most repetitions");
        assert_eq!(
            effect.parameters[1], preset.parameters[1],
            "zero keeps the angle"
        );

        output.layers[0].effect_banks[0].parameters = [0; 4];
        assert_eq!(
            resolve_output(&output, &configuration).layers[0].effects[0].parameters,
            preset.parameters
        );
    }

    #[test]
    fn cmyk_rasterize_keeps_its_mode_until_the_desk_asks_for_another() {
        let configuration = MediaConfiguration::default();
        let id = configuration.outputs[0].id;
        let mut output = OutputState::new(id, LayerPersonality::TwoLayers);
        output.layers[0].effect_banks[0] = EffectBankState {
            select: 11,
            strength: 1.0,
            parameters: [0, 255, 0, 0],
        };
        let resolved = resolve_output(&output, &configuration);
        let parameters = &resolved.layers[0].effects[0].parameters;
        assert_eq!(
            parameters[0],
            media_domain::RasterizeMode::Cmyk.parameter(),
            "an untouched mode byte keeps the CMYK preset CMYK"
        );
        assert_eq!(parameters[1], 32.0, "255 is the largest dot");
    }
}
