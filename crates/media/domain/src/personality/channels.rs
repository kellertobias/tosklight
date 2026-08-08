//! The canonical v2 channel table.
//!
//! One layer occupies 34 consecutive slots; the master occupies 7. Fine channels are big-endian
//! coarse/fine pairs. This table is the single source the receivers, the API, UI metadata, the
//! tests, and the GDTF export all read — nothing restates it.

/// A slot's meaning, for GDTF channel functions, UI metadata, and the DMX map view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChannelSpec {
    /// Zero-based offset within the layer or master block.
    pub offset: u16,
    pub name: &'static str,
    pub resolution: Resolution,
}

/// How much of a value a slot carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    /// The whole value lives in this one slot.
    Byte,
    /// The high byte of a 16-bit pair; the fine byte follows immediately.
    Coarse,
    /// The low byte of a 16-bit pair.
    Fine,
}

macro_rules! layer_channels {
    ($($offset:literal $name:literal $resolution:ident)*) => {
        pub const LAYER_CHANNELS: &[ChannelSpec] = &[
            $(ChannelSpec { offset: $offset, name: $name, resolution: Resolution::$resolution },)*
        ];
    };
}

layer_channels! {
     0 "Folder"                Byte
     1 "File"                  Byte
     2 "Play mode"             Byte
     3 "Scale X"               Coarse
     4 "Scale X fine"          Fine
     5 "Scale Y"               Coarse
     6 "Scale Y fine"          Fine
     7 "Scaling mode"          Byte
     8 "Position X"            Coarse
     9 "Position X fine"       Fine
    10 "Position Y"            Coarse
    11 "Position Y fine"       Fine
    12 "Rotation"              Coarse
    13 "Rotation fine"         Fine
    14 "Dimmer"                Byte
    15 "Volume"                Byte
    16 "Cyan"                  Byte
    17 "Magenta"               Byte
    18 "Yellow"                Byte
    19 "Grayscale"             Byte
    20 "Mask folder"           Byte
    21 "Mask file"             Byte
    22 "Mask scale X"          Coarse
    23 "Mask scale X fine"     Fine
    24 "Mask scale Y"          Coarse
    25 "Mask scale Y fine"     Fine
    26 "Mask invert"           Byte
    27 "Mask opacity"          Byte
    28 "Effect 1"              Byte
    29 "Effect 2"              Byte
    30 "Effect 3"              Byte
    31 "Effect 4"              Byte
    32 "Speed multiplier"      Byte
    33 "Playback BPM"          Byte
}

/// The master section, beginning immediately after the controlled layers.
pub const MASTER_CHANNELS: &[ChannelSpec] = &[
    ChannelSpec {
        offset: 0,
        name: "Master dimmer",
        resolution: Resolution::Byte,
    },
    ChannelSpec {
        offset: 1,
        name: "Master volume",
        resolution: Resolution::Byte,
    },
    ChannelSpec {
        offset: 2,
        name: "Master cyan",
        resolution: Resolution::Byte,
    },
    ChannelSpec {
        offset: 3,
        name: "Master magenta",
        resolution: Resolution::Byte,
    },
    ChannelSpec {
        offset: 4,
        name: "Master yellow",
        resolution: Resolution::Byte,
    },
    ChannelSpec {
        offset: 5,
        name: "Flip/mirror",
        resolution: Resolution::Byte,
    },
    ChannelSpec {
        offset: 6,
        name: "Master mask",
        resolution: Resolution::Byte,
    },
];

/// Zero-based layer offsets, named so the decoder reads as the published table does.
pub mod layer {
    pub const FOLDER: usize = 0;
    pub const FILE: usize = 1;
    pub const PLAY_MODE: usize = 2;
    pub const SCALE_X: usize = 3;
    pub const SCALE_Y: usize = 5;
    pub const SCALING_MODE: usize = 7;
    pub const POSITION_X: usize = 8;
    pub const POSITION_Y: usize = 10;
    pub const ROTATION: usize = 12;
    pub const DIMMER: usize = 14;
    pub const VOLUME: usize = 15;
    pub const CYAN: usize = 16;
    pub const MAGENTA: usize = 17;
    pub const YELLOW: usize = 18;
    pub const GRAYSCALE: usize = 19;
    pub const MASK_FOLDER: usize = 20;
    pub const MASK_FILE: usize = 21;
    pub const MASK_SCALE_X: usize = 22;
    pub const MASK_SCALE_Y: usize = 24;
    pub const MASK_INVERT: usize = 26;
    pub const MASK_OPACITY: usize = 27;
    pub const EFFECT_1: usize = 28;
    pub const SPEED_MULTIPLIER: usize = 32;
    pub const PLAYBACK_BPM: usize = 33;
}

/// Zero-based master offsets.
pub mod master {
    pub const DIMMER: usize = 0;
    pub const VOLUME: usize = 1;
    pub const CYAN: usize = 2;
    pub const MAGENTA: usize = 3;
    pub const YELLOW: usize = 4;
    pub const FLIP_MIRROR: usize = 5;
    pub const MASK: usize = 6;
}

#[cfg(test)]
mod tests {
    use super::super::{LAYER_SLOTS, MASTER_SLOTS};
    use super::*;

    #[test]
    fn the_table_covers_every_slot_exactly_once_in_order() {
        assert_eq!(LAYER_CHANNELS.len(), usize::from(LAYER_SLOTS));
        for (index, channel) in LAYER_CHANNELS.iter().enumerate() {
            assert_eq!(usize::from(channel.offset), index, "{}", channel.name);
        }

        assert_eq!(MASTER_CHANNELS.len(), usize::from(MASTER_SLOTS));
        for (index, channel) in MASTER_CHANNELS.iter().enumerate() {
            assert_eq!(usize::from(channel.offset), index, "{}", channel.name);
        }
    }

    #[test]
    fn every_coarse_channel_is_followed_by_its_fine_byte() {
        for (index, channel) in LAYER_CHANNELS.iter().enumerate() {
            if channel.resolution != Resolution::Coarse {
                continue;
            }
            let fine = LAYER_CHANNELS.get(index + 1).unwrap_or_else(|| {
                panic!("{} is the last slot and has no fine byte", channel.name)
            });
            assert_eq!(
                fine.resolution,
                Resolution::Fine,
                "{} is not followed by a fine byte",
                channel.name
            );
        }
    }

    #[test]
    fn no_fine_byte_stands_alone() {
        for (index, channel) in LAYER_CHANNELS.iter().enumerate() {
            if channel.resolution != Resolution::Fine {
                continue;
            }
            let coarse = LAYER_CHANNELS[index - 1];
            assert_eq!(
                coarse.resolution,
                Resolution::Coarse,
                "{} has no coarse byte",
                channel.name
            );
        }
    }

    #[test]
    fn there_are_seven_sixteen_bit_pairs_per_layer() {
        let pairs = LAYER_CHANNELS
            .iter()
            .filter(|channel| channel.resolution == Resolution::Coarse)
            .count();
        assert_eq!(
            pairs, 7,
            "scale X/Y, position X/Y, rotation, and both mask axes"
        );
    }

    #[test]
    fn the_named_offsets_match_the_published_table() {
        assert_eq!(LAYER_CHANNELS[layer::FOLDER].name, "Folder");
        assert_eq!(LAYER_CHANNELS[layer::PLAY_MODE].name, "Play mode");
        assert_eq!(LAYER_CHANNELS[layer::MASK_SCALE_Y].name, "Mask scale Y");
        assert_eq!(
            LAYER_CHANNELS[layer::SPEED_MULTIPLIER].name,
            "Speed multiplier"
        );
        assert_eq!(LAYER_CHANNELS[layer::PLAYBACK_BPM].name, "Playback BPM");
        assert_eq!(MASTER_CHANNELS[master::FLIP_MIRROR].name, "Flip/mirror");
        assert_eq!(MASTER_CHANNELS[master::MASK].name, "Master mask");
    }

    #[test]
    fn every_channel_name_is_distinct() {
        let mut names: Vec<&str> = LAYER_CHANNELS
            .iter()
            .chain(MASTER_CHANNELS)
            .map(|channel| channel.name)
            .collect();
        let total = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), total);
    }
}
