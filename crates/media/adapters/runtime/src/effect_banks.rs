use media_application::MediaConfiguration;
use media_domain::{EffectLibrary, LayerState, OutputState, PersonalityLayout};

/// Resolves the two lightweight DMX selectors into the existing typed renderer effect chain.
/// Legacy layouts retain their four directly configured slots unchanged.
pub(crate) fn resolve_output(
    output: &OutputState,
    configuration: &MediaConfiguration,
) -> OutputState {
    let Some(configured) = configuration.output(output.id) else {
        return output.clone();
    };
    if configured.personality_layout != PersonalityLayout::EffectBanks {
        return output.clone();
    }
    let mut resolved = output.clone();
    for (layer_index, layer) in resolved.layers.iter_mut().enumerate() {
        resolve_layer(layer, &configuration.effects, layer_index);
    }
    resolved
}

fn resolve_layer(layer: &mut LayerState, library: &EffectLibrary, layer_index: usize) {
    layer.effects = Default::default();
    for (bank_index, bank) in layer.effect_banks.iter().copied().enumerate() {
        let Some(preset) = library.resolve(bank.select) else {
            continue;
        };
        let mut effect = preset.effect.clone();
        effect.enabled = true;
        effect.mix = bank.strength.clamp(0.0, 1.0);
        effect.seed = ((layer_index as u32) << 8) | bank_index as u32;
        effect.normalize();
        layer.effects[bank_index] = effect;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use media_domain::{EffectBankState, LayerPersonality};

    #[test]
    fn two_banks_resolve_in_order_and_zero_or_missing_is_off() {
        let mut configuration = MediaConfiguration::default();
        let id = configuration.outputs[0].id;
        let mut output = OutputState::new(id, LayerPersonality::TwoLayers);
        output.layers[0].effect_banks = [
            EffectBankState {
                select: 2,
                strength: 0.4,
            },
            EffectBankState {
                select: 255,
                strength: 1.0,
            },
        ];
        let resolved = resolve_output(&output, &configuration);
        assert_eq!(
            resolved.layers[0].effects[0].effect_type.as_deref(),
            Some("digital-tv")
        );
        assert_eq!(resolved.layers[0].effects[0].mix, 0.4);
        assert!(resolved.layers[0].effects[1].effect_type.is_none());

        configuration.outputs[0].personality_layout = PersonalityLayout::Extended;
        assert_eq!(
            resolve_output(&output, &configuration).layers[0].effects,
            output.layers[0].effects
        );
    }
}
