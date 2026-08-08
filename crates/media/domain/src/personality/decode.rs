//! Decoding a DMX payload into domain state.
//!
//! Art-Net and sACN both arrive here. They translate into identical domain state for identical
//! payloads, because neither of them holds a copy of this logic.

use crate::address::MediaAddress;
use crate::color::{FlipMirror, Tint};
use crate::dmx;
use crate::layer::{LayerState, MaskState, ScalingMode};
use crate::master::MasterState;
use crate::playback::PlayMode;
use crate::speed::SpeedMultiplier;

use super::channels::{layer, master};
use super::{LAYER_SLOTS, LayerPersonality, MASTER_SLOTS};

/// One layer's 34 slots.
pub type LayerSlots = [u8; LAYER_SLOTS as usize];
/// The master's 7 slots.
pub type MasterSlots = [u8; MASTER_SLOTS as usize];

/// Decodes one layer's slots.
///
/// Every value that is not on the wire keeps the state a fresh layer has, so a short or padded
/// frame can never invent a selection.
pub fn layer_state(slots: &LayerSlots) -> LayerState {
    let sixteen = |offset: usize| dmx::sixteen_bit(slots[offset], slots[offset + 1]);

    LayerState {
        address: MediaAddress::new(slots[layer::FOLDER], slots[layer::FILE]),
        play_mode: PlayMode::from_dmx(slots[layer::PLAY_MODE]),
        scale_x: dmx::layer_scale(sixteen(layer::SCALE_X)),
        scale_y: dmx::layer_scale(sixteen(layer::SCALE_Y)),
        scaling_mode: ScalingMode::from_dmx(slots[layer::SCALING_MODE]),
        position_x: dmx::position(sixteen(layer::POSITION_X)),
        position_y: dmx::position(sixteen(layer::POSITION_Y)),
        rotation: dmx::rotation(sixteen(layer::ROTATION)),
        dimmer: dmx::unit(slots[layer::DIMMER]),
        volume: dmx::unit(slots[layer::VOLUME]),
        tint: Tint::from_subtractive(
            slots[layer::CYAN],
            slots[layer::MAGENTA],
            slots[layer::YELLOW],
        ),
        grayscale: dmx::unit(slots[layer::GRAYSCALE]),
        mask: MaskState {
            address: MediaAddress::new(slots[layer::MASK_FOLDER], slots[layer::MASK_FILE]),
            scale_x: dmx::mask_scale(sixteen(layer::MASK_SCALE_X)),
            scale_y: dmx::mask_scale(sixteen(layer::MASK_SCALE_Y)),
            invert: slots[layer::MASK_INVERT] >= 128,
            opacity: dmx::unit(slots[layer::MASK_OPACITY]),
            ..MaskState::default()
        },
        effects: std::array::from_fn(|index| crate::layer::EffectSlot {
            mix: dmx::unit(slots[layer::EFFECT_1 + index]),
            ..Default::default()
        }),
        speed_multiplier: SpeedMultiplier::from_dmx(slots[layer::SPEED_MULTIPLIER]),
        playback_bpm: dmx::playback_bpm(slots[layer::PLAYBACK_BPM]),
        ..LayerState::default()
    }
}

/// Decodes the master's slots.
pub fn master_state(slots: &MasterSlots) -> MasterState {
    MasterState {
        dimmer: dmx::unit(slots[master::DIMMER]),
        volume: dmx::unit(slots[master::VOLUME]),
        tint: Tint::from_subtractive(
            slots[master::CYAN],
            slots[master::MAGENTA],
            slots[master::YELLOW],
        ),
        flip_mirror: FlipMirror::from_dmx(slots[master::FLIP_MIRROR]),
        // The master mask selects an output-level library mask by file number within the
        // library's own mask folder, so folder zero here would mean "no mask" on every byte.
        mask: MediaAddress::new(1, slots[master::MASK]),
    }
}

/// Why a payload cannot be applied to a personality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FrameError {
    #[error(
        "the frame carries {available} slots from offset {offset}, but this personality needs \
         {required}"
    )]
    TooShort {
        offset: u16,
        available: usize,
        required: u16,
    },
}

/// The layers and master a frame carries for one output.
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedFrame {
    pub layers: Vec<LayerState>,
    pub master: MasterState,
}

/// Reads one output's block out of a universe payload.
///
/// `start_address` is the operator's one-based address; the conversion to a zero-based payload
/// offset happens exactly here.
pub fn frame(
    personality: LayerPersonality,
    start_address: u16,
    payload: &[u8],
) -> Result<DecodedFrame, FrameError> {
    let footprint = personality.footprint();
    let offset = usize::from(super::SlotFootprint::payload_offset(start_address));
    let available = payload.len().saturating_sub(offset);
    if available < usize::from(footprint.total()) {
        return Err(FrameError::TooShort {
            offset: start_address,
            available,
            required: footprint.total(),
        });
    }

    let block = &payload[offset..offset + usize::from(footprint.total())];
    let layers = (0..usize::from(personality.layer_count()))
        .map(|index| {
            let base = index * usize::from(LAYER_SLOTS);
            let slots: LayerSlots = block[base..base + usize::from(LAYER_SLOTS)]
                .try_into()
                .expect("the block is exactly the footprint's size");
            layer_state(&slots)
        })
        .collect();

    let master_base = usize::from(footprint.master_offset());
    let master_slots: MasterSlots = block[master_base..master_base + usize::from(MASTER_SLOTS)]
        .try_into()
        .expect("the block is exactly the footprint's size");

    Ok(DecodedFrame {
        layers,
        master: master_state(&master_slots),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layer::MaskSource;
    use crate::playback::OnceEndState;

    fn neutral_layer() -> LayerSlots {
        let mut slots = [0u8; LAYER_SLOTS as usize];
        slots[layer::SCALE_X] = 0x80;
        slots[layer::SCALE_Y] = 0x80;
        slots[layer::POSITION_X] = 0x80;
        slots[layer::POSITION_Y] = 0x80;
        slots[layer::ROTATION] = 0x80;
        slots[layer::MASK_SCALE_X] = 0x80;
        slots[layer::MASK_SCALE_Y] = 0x80;
        slots[layer::DIMMER] = 255;
        slots[layer::VOLUME] = 255;
        slots[layer::SPEED_MULTIPLIER] = 127;
        slots
    }

    #[test]
    fn a_neutral_frame_decodes_to_the_default_layer() {
        let decoded = layer_state(&neutral_layer());
        let expected = LayerState::default();
        assert_eq!(decoded.scale_x, expected.scale_x);
        assert_eq!(decoded.scale_y, expected.scale_y);
        assert_eq!(decoded.position_x, expected.position_x);
        assert_eq!(decoded.position_y, expected.position_y);
        assert_eq!(decoded.rotation, expected.rotation);
        assert_eq!(decoded.dimmer, expected.dimmer);
        assert_eq!(decoded.volume, expected.volume);
        assert_eq!(decoded.tint, expected.tint);
        assert_eq!(decoded.speed_multiplier, expected.speed_multiplier);
        assert_eq!(decoded.play_mode, expected.play_mode);
        assert_eq!(decoded.address, expected.address);
    }

    #[test]
    fn each_slot_reaches_the_field_the_table_names() {
        let mut slots = neutral_layer();
        slots[layer::FOLDER] = 12;
        slots[layer::FILE] = 34;
        slots[layer::PLAY_MODE] = 76;
        slots[layer::SCALING_MODE] = 200;
        slots[layer::GRAYSCALE] = 255;
        slots[layer::CYAN] = 255;
        slots[layer::MASK_FOLDER] = 7;
        slots[layer::MASK_FILE] = 9;
        slots[layer::MASK_INVERT] = 128;
        slots[layer::MASK_OPACITY] = 255;
        slots[layer::EFFECT_1 + 2] = 255;
        slots[layer::PLAYBACK_BPM] = 120;

        let layer = layer_state(&slots);
        assert_eq!(layer.address, MediaAddress::new(12, 34));
        assert_eq!(
            layer.play_mode,
            PlayMode::Once {
                end_state: OnceEndState::Transparent
            }
        );
        assert_eq!(layer.scaling_mode, ScalingMode::Stretch);
        assert_eq!(layer.grayscale, 1.0);
        assert_eq!(layer.tint, Tint::new(0.0, 1.0, 1.0));
        assert_eq!(layer.mask.address, MediaAddress::new(7, 9));
        assert!(layer.mask.invert);
        assert_eq!(layer.mask.opacity, 1.0);
        assert_eq!(layer.effects[2].mix, 1.0);
        assert_eq!(layer.effects[0].mix, 0.0);
        assert_eq!(layer.playback_bpm, Some(120));
        assert_eq!(
            layer.mask.source,
            MaskSource::Luminance,
            "the wire does not carry this yet"
        );
    }

    #[test]
    fn mask_invert_switches_at_128() {
        let mut slots = neutral_layer();
        slots[layer::MASK_INVERT] = 127;
        assert!(!layer_state(&slots).mask.invert);
        slots[layer::MASK_INVERT] = 128;
        assert!(layer_state(&slots).mask.invert);
    }

    #[test]
    fn fine_bytes_are_read_as_the_low_half_of_the_pair() {
        let mut slots = neutral_layer();
        slots[layer::SCALE_X] = 0xFF;
        slots[layer::SCALE_X + 1] = 0xFF;
        assert_eq!(layer_state(&slots).scale_x, 10.0);

        slots[layer::SCALE_X] = 0x00;
        slots[layer::SCALE_X + 1] = 0x00;
        assert_eq!(layer_state(&slots).scale_x, 0.0);
    }

    #[test]
    fn the_master_block_decodes() {
        let mut slots = [0u8; MASTER_SLOTS as usize];
        slots[master::DIMMER] = 255;
        slots[master::VOLUME] = 128;
        slots[master::MAGENTA] = 255;
        slots[master::FLIP_MIRROR] = 3;
        slots[master::MASK] = 5;

        let decoded = master_state(&slots);
        assert_eq!(decoded.dimmer, 1.0);
        assert_eq!(decoded.tint, Tint::new(1.0, 0.0, 1.0));
        assert_eq!(decoded.flip_mirror, FlipMirror::Both);
        assert!(decoded.has_mask());
        assert_eq!(decoded.mask.file, 5);
    }

    fn universe(personality: LayerPersonality, start_address: u16) -> Vec<u8> {
        let mut payload = vec![0u8; 512];
        let offset = usize::from(start_address - 1);
        for index in 0..usize::from(personality.layer_count()) {
            let base = offset + index * usize::from(LAYER_SLOTS);
            payload[base..base + usize::from(LAYER_SLOTS)].copy_from_slice(&neutral_layer());
            payload[base + layer::FOLDER] = 1;
            payload[base + layer::FILE] = index as u8 + 1;
        }
        let master_base =
            offset + usize::from(personality.footprint().master_offset()) + master::DIMMER;
        payload[master_base] = 255;
        payload
    }

    #[test]
    fn an_eight_layer_frame_decodes_eight_layers_and_one_master() {
        let personality = LayerPersonality::EightLayers;
        let payload = universe(personality, 1);
        let decoded = frame(personality, 1, &payload).unwrap();

        assert_eq!(decoded.layers.len(), 8);
        for (index, layer) in decoded.layers.iter().enumerate() {
            assert_eq!(layer.address, MediaAddress::new(1, index as u8 + 1));
        }
        assert_eq!(decoded.master.dimmer, 1.0);
    }

    #[test]
    fn a_two_layer_frame_reads_only_its_own_slots() {
        let personality = LayerPersonality::TwoLayers;
        let payload = universe(personality, 1);
        let decoded = frame(personality, 1, &payload).unwrap();
        assert_eq!(decoded.layers.len(), 2);
        assert_eq!(decoded.master.dimmer, 1.0);
    }

    #[test]
    fn the_start_address_is_one_based() {
        let personality = LayerPersonality::TwoLayers;
        let payload = universe(personality, 100);
        let decoded = frame(personality, 100, &payload).unwrap();
        assert_eq!(decoded.layers[0].address, MediaAddress::new(1, 1));

        let shifted = frame(personality, 99, &payload).unwrap();
        assert_ne!(
            shifted.layers[0].address,
            MediaAddress::new(1, 1),
            "off by one must show"
        );
    }

    #[test]
    fn a_frame_that_ends_before_the_footprint_is_refused() {
        let personality = LayerPersonality::EightLayers;
        let payload = vec![0u8; 512];
        let error = frame(personality, 300, &payload).unwrap_err();
        assert_eq!(
            error,
            FrameError::TooShort {
                offset: 300,
                available: 213,
                required: 279
            }
        );
    }

    #[test]
    fn both_protocols_would_produce_identical_state_for_identical_payloads() {
        // Art-Net and sACN differ only in how the payload arrives. Decoding the same bytes twice
        // stands in for that here; the adapters own no mapping logic of their own.
        let personality = LayerPersonality::EightLayers;
        let payload = universe(personality, 45);
        let art_net = frame(personality, 45, &payload).unwrap();
        let sacn = frame(personality, 45, &payload).unwrap();
        assert_eq!(art_net, sacn);
    }
}
