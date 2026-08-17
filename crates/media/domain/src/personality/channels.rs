//! The canonical v2 channel table.
//!
//! One layer occupies 39 consecutive slots; the complete master occupies 40. Fine channels are big-endian
//! coarse/fine pairs. This table is the single source the receivers, the API, UI metadata, the
//! tests, and the GDTF export all read — nothing restates it.

use crate::layer::ScalingMode;
use crate::playback::PlayMode;
use crate::speed::SpeedMultiplier;

/// A slot's meaning, for GDTF channel functions, UI metadata, and the DMX map view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChannelSpec {
    /// Zero-based offset within the layer or master block.
    pub offset: u16,
    pub name: &'static str,
    pub resolution: Resolution,
    /// The complete raw home value. A coarse slot therefore carries the 16-bit value rather than
    /// only its high byte; a following fine slot has zero because it is not a control of its own.
    pub default_value: u16,
    /// How raw values should be explained to an operator and emitted as GDTF channel sets.
    pub values: ValueKind,
    /// Whether the runtime actually provides the declared control yet.
    pub implementation: ChannelImplementation,
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

/// The decoder that gives a channel's raw values their operator-facing meaning.
///
/// This deliberately names domain decoders rather than carrying copied range tables. Calling
/// [`ValueKind::sets`] projects the ranges from the same implementations that consume DMX.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueKind {
    /// A continuous value whose useful endpoints belong in help text, not an invented set list.
    Continuous,
    PlayMode,
    ScalingMode,
    /// A byte that changes from `off` to `on` at 128.
    Binary {
        off: &'static str,
        on: &'static str,
    },
    FlipMirror,
    SpeedMultiplier,
    PlaybackBpm,
    /// A declared channel with no effect implementation to select or describe yet.
    Unimplemented,
}

/// Whether receiving this slot currently changes an implemented operator feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelImplementation {
    Implemented,
    Unimplemented { reason: &'static str },
}

impl ChannelImplementation {
    pub const fn is_implemented(self) -> bool {
        matches!(self, Self::Implemented)
    }

    pub const fn reason(self) -> Option<&'static str> {
        match self {
            Self::Implemented => None,
            Self::Unimplemented { reason } => Some(reason),
        }
    }
}

/// One raw value set, inclusive at both ends.
///
/// Most sets use `step = 1`. Flip/mirror uses `step = 4` because the decoder intentionally
/// normalizes every byte modulo four; expressing that stride keeps values 4..=255 truthful.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelValueSet {
    pub name: String,
    pub from: u16,
    pub to: u16,
    pub step: u16,
    pub implemented: bool,
}

impl ChannelValueSet {
    fn range(name: impl Into<String>, from: u8, to: u8) -> Self {
        Self {
            name: name.into(),
            from: u16::from(from),
            to: u16::from(to),
            step: 1,
            implemented: true,
        }
    }

    fn strided(name: impl Into<String>, from: u8, to: u8, step: u16) -> Self {
        Self {
            name: name.into(),
            from: u16::from(from),
            to: u16::from(to),
            step,
            implemented: true,
        }
    }
}

impl ValueKind {
    /// Projects the value sets from the canonical decoder behavior.
    pub fn sets(self) -> Vec<ChannelValueSet> {
        match self {
            Self::Continuous => Vec::new(),
            Self::PlayMode => PlayMode::ALL
                .into_iter()
                .map(|mode| {
                    let (from, to) = mode.dmx_range();
                    ChannelValueSet::range(mode.label(), from, to)
                })
                .collect(),
            Self::ScalingMode => ScalingMode::ALL
                .into_iter()
                .map(|mode| {
                    let (from, to) = mode.dmx_range();
                    ChannelValueSet::range(
                        match mode {
                            ScalingMode::Fit => "Fit",
                            ScalingMode::Fill => "Fill",
                            ScalingMode::Original => "Original",
                            ScalingMode::Stretch => "Stretch",
                        },
                        from,
                        to,
                    )
                })
                .collect(),
            Self::Binary { off, on } => vec![
                ChannelValueSet::range(off, 0, 127),
                ChannelValueSet::range(on, 128, 255),
            ],
            Self::FlipMirror => vec![
                ChannelValueSet::strided("None", 0, 252, 4),
                ChannelValueSet::strided("Horizontal", 1, 253, 4),
                ChannelValueSet::strided("Vertical", 2, 254, 4),
                ChannelValueSet::strided("Both", 3, 255, 4),
            ],
            Self::SpeedMultiplier => {
                contiguous_byte_sets(|value| SpeedMultiplier::from_dmx(value).label())
            }
            Self::PlaybackBpm => vec![
                ChannelValueSet::range("Off", 0, 0),
                ChannelValueSet::range("1–255 BPM", 1, 255),
            ],
            Self::Unimplemented => vec![ChannelValueSet {
                name: "Declared — effect engine not implemented".to_owned(),
                from: 0,
                to: 255,
                step: 1,
                implemented: false,
            }],
        }
    }
}

fn contiguous_byte_sets(mut label: impl FnMut(u8) -> String) -> Vec<ChannelValueSet> {
    let mut sets = Vec::new();
    let mut from = 0u8;
    let mut current = label(0);
    for value in 1..=255u8 {
        let next = label(value);
        if next == current {
            continue;
        }
        sets.push(ChannelValueSet::range(current, from, value - 1));
        from = value;
        current = next;
    }
    sets.push(ChannelValueSet::range(current, from, 255));
    sets
}

macro_rules! layer_channels {
    ($($offset:literal $name:literal $resolution:ident $default:literal, $values:expr, $implementation:expr;)*) => {
        pub const LAYER_CHANNELS: &[ChannelSpec] = &[
            $(ChannelSpec {
                offset: $offset,
                name: $name,
                resolution: Resolution::$resolution,
                default_value: $default,
                values: $values,
                implementation: $implementation,
            },)*
        ];
    };
}

const IMPLEMENTED: ChannelImplementation = ChannelImplementation::Implemented;

layer_channels! {
     0 "Folder"                Byte       0, ValueKind::Continuous, IMPLEMENTED;
     1 "File"                  Byte       0, ValueKind::Continuous, IMPLEMENTED;
     2 "Play mode"             Byte       0, ValueKind::PlayMode, IMPLEMENTED;
     3 "Scale X"               Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
     4 "Scale X fine"          Fine       0, ValueKind::Continuous, IMPLEMENTED;
     5 "Scale Y"               Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
     6 "Scale Y fine"          Fine       0, ValueKind::Continuous, IMPLEMENTED;
     7 "Scaling mode"          Byte       0, ValueKind::ScalingMode, IMPLEMENTED;
     8 "Position X"            Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
     9 "Position X fine"       Fine       0, ValueKind::Continuous, IMPLEMENTED;
    10 "Position Y"            Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
    11 "Position Y fine"       Fine       0, ValueKind::Continuous, IMPLEMENTED;
    12 "Rotation"              Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
    13 "Rotation fine"         Fine       0, ValueKind::Continuous, IMPLEMENTED;
    14 "Dimmer"                Byte       0, ValueKind::Continuous, IMPLEMENTED;
    15 "Volume"                Byte     255, ValueKind::Continuous, IMPLEMENTED;
    16 "Cyan"                  Byte       0, ValueKind::Continuous, IMPLEMENTED;
    17 "Magenta"               Byte       0, ValueKind::Continuous, IMPLEMENTED;
    18 "Yellow"                Byte       0, ValueKind::Continuous, IMPLEMENTED;
    19 "Grayscale"             Byte       0, ValueKind::Continuous, IMPLEMENTED;
    20 "Mask folder"           Byte       0, ValueKind::Continuous, IMPLEMENTED;
    21 "Mask file"             Byte       0, ValueKind::Continuous, IMPLEMENTED;
    22 "Mask scale X"          Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
    23 "Mask scale X fine"     Fine       0, ValueKind::Continuous, IMPLEMENTED;
    24 "Mask scale Y"          Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
    25 "Mask scale Y fine"     Fine       0, ValueKind::Continuous, IMPLEMENTED;
    26 "Mask invert"           Byte       0, ValueKind::Binary { off: "Normal", on: "Inverted" }, IMPLEMENTED;
    27 "Mask opacity"          Byte       0, ValueKind::Continuous, IMPLEMENTED;
    28 "Effect 1"              Byte       0, ValueKind::Continuous, IMPLEMENTED;
    29 "Effect 2"              Byte       0, ValueKind::Continuous, IMPLEMENTED;
    30 "Effect 3"              Byte       0, ValueKind::Continuous, IMPLEMENTED;
    31 "Effect 4"              Byte       0, ValueKind::Continuous, IMPLEMENTED;
    32 "Speed multiplier"      Byte     127, ValueKind::SpeedMultiplier, IMPLEMENTED;
    33 "Playback BPM"          Byte       0, ValueKind::PlaybackBpm, IMPLEMENTED;
    34 "Blur"                  Byte       0, ValueKind::Continuous, IMPLEMENTED;
    // New controls append to the published block. Existing desks therefore keep sending every
    // pre-existing control, especially playback speed, at the slot they originally patched.
    35 "Mask position X"       Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
    36 "Mask position X fine"  Fine       0, ValueKind::Continuous, IMPLEMENTED;
    37 "Mask position Y"       Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
    38 "Mask position Y fine"  Fine       0, ValueKind::Continuous, IMPLEMENTED;
}

/// The master section, beginning immediately after the controlled layers.
pub const MASTER_CHANNELS: &[ChannelSpec] = &[
    ChannelSpec {
        offset: 0,
        name: "Master dimmer",
        resolution: Resolution::Byte,
        default_value: 255,
        values: ValueKind::Continuous,
        implementation: IMPLEMENTED,
    },
    ChannelSpec {
        offset: 1,
        name: "Master volume",
        resolution: Resolution::Byte,
        default_value: 255,
        values: ValueKind::Continuous,
        implementation: IMPLEMENTED,
    },
    ChannelSpec {
        offset: 2,
        name: "Master cyan",
        resolution: Resolution::Byte,
        default_value: 0,
        values: ValueKind::Continuous,
        implementation: IMPLEMENTED,
    },
    ChannelSpec {
        offset: 3,
        name: "Master magenta",
        resolution: Resolution::Byte,
        default_value: 0,
        values: ValueKind::Continuous,
        implementation: IMPLEMENTED,
    },
    ChannelSpec {
        offset: 4,
        name: "Master yellow",
        resolution: Resolution::Byte,
        default_value: 0,
        values: ValueKind::Continuous,
        implementation: IMPLEMENTED,
    },
    ChannelSpec {
        offset: 5,
        name: "Flip/mirror",
        resolution: Resolution::Byte,
        default_value: 0,
        values: ValueKind::FlipMirror,
        implementation: IMPLEMENTED,
    },
    ChannelSpec {
        offset: 6,
        name: "Master mask",
        resolution: Resolution::Byte,
        default_value: 0,
        values: ValueKind::Continuous,
        implementation: IMPLEMENTED,
    },
    ChannelSpec {
        offset: 7,
        name: "Master mask position X",
        resolution: Resolution::Coarse,
        default_value: 32_768,
        values: ValueKind::Continuous,
        implementation: IMPLEMENTED,
    },
    ChannelSpec {
        offset: 8,
        name: "Master mask position X fine",
        resolution: Resolution::Fine,
        default_value: 0,
        values: ValueKind::Continuous,
        implementation: IMPLEMENTED,
    },
    ChannelSpec {
        offset: 9,
        name: "Master mask position Y",
        resolution: Resolution::Coarse,
        default_value: 32_768,
        values: ValueKind::Continuous,
        implementation: IMPLEMENTED,
    },
    ChannelSpec {
        offset: 10,
        name: "Master mask position Y fine",
        resolution: Resolution::Fine,
        default_value: 0,
        values: ValueKind::Continuous,
        implementation: IMPLEMENTED,
    },
    ChannelSpec {
        offset: 11,
        name: "Master scale X",
        resolution: Resolution::Coarse,
        default_value: 32_768,
        values: ValueKind::Continuous,
        implementation: IMPLEMENTED,
    },
    ChannelSpec {
        offset: 12,
        name: "Master scale X fine",
        resolution: Resolution::Fine,
        default_value: 0,
        values: ValueKind::Continuous,
        implementation: IMPLEMENTED,
    },
    ChannelSpec {
        offset: 13,
        name: "Master scale Y",
        resolution: Resolution::Coarse,
        default_value: 32_768,
        values: ValueKind::Continuous,
        implementation: IMPLEMENTED,
    },
    ChannelSpec {
        offset: 14,
        name: "Master scale Y fine",
        resolution: Resolution::Fine,
        default_value: 0,
        values: ValueKind::Continuous,
        implementation: IMPLEMENTED,
    },
    ChannelSpec {
        offset: 15,
        name: "Master scaling mode",
        resolution: Resolution::Byte,
        default_value: 0,
        values: ValueKind::ScalingMode,
        implementation: IMPLEMENTED,
    },
    ChannelSpec {
        offset: 16,
        name: "Master position X",
        resolution: Resolution::Coarse,
        default_value: 32_768,
        values: ValueKind::Continuous,
        implementation: IMPLEMENTED,
    },
    ChannelSpec {
        offset: 17,
        name: "Master position X fine",
        resolution: Resolution::Fine,
        default_value: 0,
        values: ValueKind::Continuous,
        implementation: IMPLEMENTED,
    },
    ChannelSpec {
        offset: 18,
        name: "Master position Y",
        resolution: Resolution::Coarse,
        default_value: 32_768,
        values: ValueKind::Continuous,
        implementation: IMPLEMENTED,
    },
    ChannelSpec {
        offset: 19,
        name: "Master position Y fine",
        resolution: Resolution::Fine,
        default_value: 0,
        values: ValueKind::Continuous,
        implementation: IMPLEMENTED,
    },
    ChannelSpec {
        offset: 20,
        name: "Master rotation",
        resolution: Resolution::Coarse,
        default_value: 32_768,
        values: ValueKind::Continuous,
        implementation: IMPLEMENTED,
    },
    ChannelSpec {
        offset: 21,
        name: "Master rotation fine",
        resolution: Resolution::Fine,
        default_value: 0,
        values: ValueKind::Continuous,
        implementation: IMPLEMENTED,
    },
    ChannelSpec {
        offset: 22,
        name: "Shaper left",
        resolution: Resolution::Coarse,
        default_value: 0,
        values: ValueKind::Continuous,
        implementation: IMPLEMENTED,
    },
    ChannelSpec {
        offset: 23,
        name: "Shaper left fine",
        resolution: Resolution::Fine,
        default_value: 0,
        values: ValueKind::Continuous,
        implementation: IMPLEMENTED,
    },
    ChannelSpec {
        offset: 24,
        name: "Shaper right",
        resolution: Resolution::Coarse,
        default_value: 0,
        values: ValueKind::Continuous,
        implementation: IMPLEMENTED,
    },
    ChannelSpec {
        offset: 25,
        name: "Shaper right fine",
        resolution: Resolution::Fine,
        default_value: 0,
        values: ValueKind::Continuous,
        implementation: IMPLEMENTED,
    },
    ChannelSpec {
        offset: 26,
        name: "Shaper top",
        resolution: Resolution::Coarse,
        default_value: 0,
        values: ValueKind::Continuous,
        implementation: IMPLEMENTED,
    },
    ChannelSpec {
        offset: 27,
        name: "Shaper top fine",
        resolution: Resolution::Fine,
        default_value: 0,
        values: ValueKind::Continuous,
        implementation: IMPLEMENTED,
    },
    ChannelSpec {
        offset: 28,
        name: "Shaper bottom",
        resolution: Resolution::Coarse,
        default_value: 0,
        values: ValueKind::Continuous,
        implementation: IMPLEMENTED,
    },
    ChannelSpec {
        offset: 29,
        name: "Shaper bottom fine",
        resolution: Resolution::Fine,
        default_value: 0,
        values: ValueKind::Continuous,
        implementation: IMPLEMENTED,
    },
    ChannelSpec {
        offset: 30,
        name: "Shaper left rotation",
        resolution: Resolution::Coarse,
        default_value: 32_768,
        values: ValueKind::Continuous,
        implementation: IMPLEMENTED,
    },
    ChannelSpec {
        offset: 31,
        name: "Shaper left rotation fine",
        resolution: Resolution::Fine,
        default_value: 0,
        values: ValueKind::Continuous,
        implementation: IMPLEMENTED,
    },
    ChannelSpec {
        offset: 32,
        name: "Shaper right rotation",
        resolution: Resolution::Coarse,
        default_value: 32_768,
        values: ValueKind::Continuous,
        implementation: IMPLEMENTED,
    },
    ChannelSpec {
        offset: 33,
        name: "Shaper right rotation fine",
        resolution: Resolution::Fine,
        default_value: 0,
        values: ValueKind::Continuous,
        implementation: IMPLEMENTED,
    },
    ChannelSpec {
        offset: 34,
        name: "Shaper top rotation",
        resolution: Resolution::Coarse,
        default_value: 32_768,
        values: ValueKind::Continuous,
        implementation: IMPLEMENTED,
    },
    ChannelSpec {
        offset: 35,
        name: "Shaper top rotation fine",
        resolution: Resolution::Fine,
        default_value: 0,
        values: ValueKind::Continuous,
        implementation: IMPLEMENTED,
    },
    ChannelSpec {
        offset: 36,
        name: "Shaper bottom rotation",
        resolution: Resolution::Coarse,
        default_value: 32_768,
        values: ValueKind::Continuous,
        implementation: IMPLEMENTED,
    },
    ChannelSpec {
        offset: 37,
        name: "Shaper bottom rotation fine",
        resolution: Resolution::Fine,
        default_value: 0,
        values: ValueKind::Continuous,
        implementation: IMPLEMENTED,
    },
    ChannelSpec {
        offset: 38,
        name: "Shaper rotation",
        resolution: Resolution::Coarse,
        default_value: 32_768,
        values: ValueKind::Continuous,
        implementation: IMPLEMENTED,
    },
    ChannelSpec {
        offset: 39,
        name: "Shaper rotation fine",
        resolution: Resolution::Fine,
        default_value: 0,
        values: ValueKind::Continuous,
        implementation: IMPLEMENTED,
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
    pub const BLUR: usize = 34;
    pub const MASK_POSITION_X: usize = 35;
    pub const MASK_POSITION_Y: usize = 37;
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
    pub const MASK_POSITION_X: usize = 7;
    pub const MASK_POSITION_Y: usize = 9;
    pub const SCALE_X: usize = 11;
    pub const SCALE_Y: usize = 13;
    pub const SCALING_MODE: usize = 15;
    pub const POSITION_X: usize = 16;
    pub const POSITION_Y: usize = 18;
    pub const ROTATION: usize = 20;
    pub const SHAPER_LEFT: usize = 22;
    pub const SHAPER_RIGHT: usize = 24;
    pub const SHAPER_TOP: usize = 26;
    pub const SHAPER_BOTTOM: usize = 28;
    pub const SHAPER_LEFT_ROTATION: usize = 30;
    pub const SHAPER_RIGHT_ROTATION: usize = 32;
    pub const SHAPER_TOP_ROTATION: usize = 34;
    pub const SHAPER_BOTTOM_ROTATION: usize = 36;
    pub const SHAPER_ROTATION: usize = 38;
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
        for table in [LAYER_CHANNELS, MASTER_CHANNELS] {
            for (index, channel) in table.iter().enumerate() {
                if channel.resolution != Resolution::Coarse {
                    continue;
                }
                let fine = table.get(index + 1).unwrap_or_else(|| {
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
    }

    #[test]
    fn no_fine_byte_stands_alone() {
        for table in [LAYER_CHANNELS, MASTER_CHANNELS] {
            for (index, channel) in table.iter().enumerate() {
                if channel.resolution != Resolution::Fine {
                    continue;
                }
                let coarse = table[index - 1];
                assert_eq!(
                    coarse.resolution,
                    Resolution::Coarse,
                    "{} has no coarse byte",
                    channel.name
                );
            }
        }
    }

    #[test]
    fn there_are_nine_sixteen_bit_pairs_per_layer() {
        let pairs = LAYER_CHANNELS
            .iter()
            .filter(|channel| channel.resolution == Resolution::Coarse)
            .count();
        assert_eq!(
            pairs, 9,
            "scale X/Y, position X/Y, rotation, and mask scale plus position axes"
        );
    }

    #[test]
    fn the_named_offsets_match_the_published_table() {
        assert_eq!(LAYER_CHANNELS[layer::FOLDER].name, "Folder");
        assert_eq!(LAYER_CHANNELS[layer::PLAY_MODE].name, "Play mode");
        assert_eq!(LAYER_CHANNELS[layer::MASK_SCALE_Y].name, "Mask scale Y");
        assert_eq!(
            LAYER_CHANNELS[layer::MASK_POSITION_X].name,
            "Mask position X"
        );
        assert_eq!(
            LAYER_CHANNELS[layer::SPEED_MULTIPLIER].name,
            "Speed multiplier"
        );
        assert_eq!(LAYER_CHANNELS[layer::PLAYBACK_BPM].name, "Playback BPM");
        assert_eq!(MASTER_CHANNELS[master::FLIP_MIRROR].name, "Flip/mirror");
        assert_eq!(MASTER_CHANNELS[master::MASK].name, "Master mask");
        assert_eq!(
            MASTER_CHANNELS[master::MASK_POSITION_Y].name,
            "Master mask position Y"
        );
    }

    #[test]
    fn adding_mask_position_does_not_move_existing_playback_controls() {
        assert_eq!(layer::MASK_INVERT, 26);
        assert_eq!(layer::EFFECT_1, 28);
        assert_eq!(layer::SPEED_MULTIPLIER, 32);
        assert_eq!(layer::PLAYBACK_BPM, 33);
        assert_eq!(layer::BLUR, 34);
        assert_eq!(layer::MASK_POSITION_X, 35);
        assert_eq!(layer::MASK_POSITION_Y, 37);
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

    fn matching_set(sets: &[ChannelValueSet], value: u8) -> &ChannelValueSet {
        let value = u16::from(value);
        let matching: Vec<&ChannelValueSet> = sets
            .iter()
            .filter(|set| {
                (set.from..=set.to).contains(&value) && (value - set.from) % set.step == 0
            })
            .collect();
        assert_eq!(matching.len(), 1, "raw value {value} must have one set");
        matching[0]
    }

    #[test]
    fn play_and_scaling_sets_are_projected_from_the_decoders() {
        let play_sets = LAYER_CHANNELS[layer::PLAY_MODE].values.sets();
        let scaling_sets = LAYER_CHANNELS[layer::SCALING_MODE].values.sets();

        for raw in 0..=255u8 {
            assert_eq!(
                matching_set(&play_sets, raw).name,
                PlayMode::from_dmx(raw).label()
            );
            assert_eq!(
                matching_set(&scaling_sets, raw).name,
                match ScalingMode::from_dmx(raw) {
                    ScalingMode::Fit => "Fit",
                    ScalingMode::Fill => "Fill",
                    ScalingMode::Original => "Original",
                    ScalingMode::Stretch => "Stretch",
                }
            );
        }
    }

    #[test]
    fn flip_mirror_sets_describe_all_modulo_four_values() {
        let sets = MASTER_CHANNELS[master::FLIP_MIRROR].values.sets();
        assert_eq!(sets.len(), 4);
        assert!(sets.iter().all(|set| set.step == 4));

        for raw in 0..=255u8 {
            assert_eq!(
                matching_set(&sets, raw).name,
                match crate::color::FlipMirror::from_dmx(raw) {
                    crate::color::FlipMirror::None => "None",
                    crate::color::FlipMirror::Horizontal => "Horizontal",
                    crate::color::FlipMirror::Vertical => "Vertical",
                    crate::color::FlipMirror::Both => "Both",
                }
            );
        }
    }

    #[test]
    fn speed_multiplier_sets_are_the_actual_quantized_bands() {
        let sets = LAYER_CHANNELS[layer::SPEED_MULTIPLIER].values.sets();
        assert_eq!(sets.len(), 31);
        for raw in 0..=255u8 {
            assert_eq!(
                matching_set(&sets, raw).name,
                SpeedMultiplier::from_dmx(raw).label()
            );
        }
    }

    #[test]
    fn the_effect_slots_are_normalized_mix_controls() {
        for effect in &LAYER_CHANNELS[layer::EFFECT_1..layer::EFFECT_1 + 4] {
            assert!(effect.implementation.is_implemented(), "{}", effect.name);
            assert!(effect.implementation.reason().is_none(), "{}", effect.name);
            assert_eq!(effect.values, ValueKind::Continuous, "{}", effect.name);
        }
    }

    #[test]
    fn canonical_defaults_are_neutral_and_fit_their_resolution() {
        for channel in LAYER_CHANNELS.iter().chain(MASTER_CHANNELS) {
            let maximum = match channel.resolution {
                Resolution::Coarse => u16::MAX,
                Resolution::Byte | Resolution::Fine => u16::from(u8::MAX),
            };
            assert!(channel.default_value <= maximum, "{}", channel.name);
        }

        assert_eq!(LAYER_CHANNELS[layer::SCALE_X].default_value, 32_768);
        assert_eq!(LAYER_CHANNELS[layer::DIMMER].default_value, 0);
        assert_eq!(LAYER_CHANNELS[layer::CYAN].default_value, 0);
        assert_eq!(LAYER_CHANNELS[layer::SPEED_MULTIPLIER].default_value, 127);
        assert_eq!(MASTER_CHANNELS[master::DIMMER].default_value, 255);
        assert_eq!(MASTER_CHANNELS[master::CYAN].default_value, 0);
    }
}
