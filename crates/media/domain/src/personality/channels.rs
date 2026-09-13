//! The canonical channel tables.
//!
//! The current mapping layer occupies 59 consecutive slots and its master 40. Fine channels are
//! big-endian coarse/fine pairs. These tables are the single source the receivers, the API, UI
//! metadata, the tests, and the GDTF export all read — nothing restates them.
//!
//! The effect-bank tables are the frozen published layout every older channel layout is a prefix
//! of, so a desk patched against one of those keeps the meaning it was programmed with.

use crate::blend::{BlendMode, STROBE_DMX};
use crate::layer::ScalingMode;
use crate::master::BeatRatio;
use crate::playback::PlayMode;
use crate::speed::SpeedMultiplier;

use super::PersonalityLayout;

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
    BeatRatio,
    /// One effect-bank parameter byte: zero keeps the selected preset's stored value, and
    /// 1–255 spans whatever range that preset's effect gives the parameter.
    EffectParameter,
    /// One visualizer parameter byte: zero keeps the configured value, and 1–255 spans the
    /// selected visualizer kind's parameter.
    VisualizerParameter,
    /// Blend modes in bands of sixteen, then strobe, then Normal without strobe.
    BlendMode,
    /// Zero draws the layer flat; 1–255 selects a numbered 3D model.
    ModelSelect,
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
            Self::BeatRatio => [
                BeatRatio::Disabled,
                BeatRatio::Divide(16),
                BeatRatio::Divide(8),
                BeatRatio::Divide(4),
                BeatRatio::Divide(2),
                BeatRatio::Unity,
                BeatRatio::Multiply(2),
                BeatRatio::Multiply(4),
                BeatRatio::Multiply(8),
                BeatRatio::Multiply(16),
            ]
            .into_iter()
            .map(|ratio| {
                let (from, to) = ratio.dmx_range();
                ChannelValueSet::range(ratio.label(), from, to)
            })
            .collect(),
            Self::EffectParameter => vec![
                ChannelValueSet::range("Preset value", 0, 0),
                ChannelValueSet::range("Parameter", 1, 255),
            ],
            Self::VisualizerParameter => vec![
                ChannelValueSet::range("Configured value", 0, 0),
                ChannelValueSet::range("Parameter", 1, 255),
            ],
            Self::BlendMode => {
                let mut sets: Vec<ChannelValueSet> = BlendMode::ALL
                    .into_iter()
                    .map(|mode| {
                        let (from, to) = mode.dmx_range();
                        ChannelValueSet::range(mode.label(), from, to)
                    })
                    .collect();
                sets.push(ChannelValueSet::range(
                    "Strobe slow–fast",
                    STROBE_DMX.0,
                    STROBE_DMX.1,
                ));
                sets.push(ChannelValueSet::range(
                    "Normal, no strobe",
                    STROBE_DMX.1 + 1,
                    255,
                ));
                sets
            }
            Self::ModelSelect => vec![
                ChannelValueSet::range("Flat", 0, 0),
                ChannelValueSet::range("Model", 1, 255),
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

macro_rules! channel_table {
    ($table:ident; $($offset:literal $name:literal $resolution:ident $default:literal, $values:expr, $implementation:expr;)*) => {
        pub const $table: &[ChannelSpec] = &[
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
const LEGACY_ONLY: ChannelImplementation = ChannelImplementation::Unimplemented {
    reason: "reserved for legacy Blur; current effect-bank personalities select Blur from the Effects library",
};

channel_table! { LAYER_CHANNELS;
     0 "Folder"                  Byte       0, ValueKind::Continuous, IMPLEMENTED;
     1 "File"                    Byte       0, ValueKind::Continuous, IMPLEMENTED;
     2 "Play mode"               Byte       0, ValueKind::PlayMode, IMPLEMENTED;
     3 "Scale X"                 Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
     4 "Scale X fine"            Fine       0, ValueKind::Continuous, IMPLEMENTED;
     5 "Scale Y"                 Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
     6 "Scale Y fine"            Fine       0, ValueKind::Continuous, IMPLEMENTED;
     7 "Scaling mode"            Byte       0, ValueKind::ScalingMode, IMPLEMENTED;
     8 "Position X"              Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
     9 "Position X fine"         Fine       0, ValueKind::Continuous, IMPLEMENTED;
    10 "Position Y"              Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
    11 "Position Y fine"         Fine       0, ValueKind::Continuous, IMPLEMENTED;
    // With a 3D model selected this is the model's roll, applied after pan and tilt.
    12 "Rotation"                Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
    13 "Rotation fine"           Fine       0, ValueKind::Continuous, IMPLEMENTED;
    14 "Dimmer"                  Byte       0, ValueKind::Continuous, IMPLEMENTED;
    15 "Volume"                  Byte     255, ValueKind::Continuous, IMPLEMENTED;
    16 "Cyan"                    Byte       0, ValueKind::Continuous, IMPLEMENTED;
    17 "Magenta"                 Byte       0, ValueKind::Continuous, IMPLEMENTED;
    18 "Yellow"                  Byte       0, ValueKind::Continuous, IMPLEMENTED;
    19 "Grayscale"               Byte       0, ValueKind::Continuous, IMPLEMENTED;
    20 "Mask folder"             Byte       0, ValueKind::Continuous, IMPLEMENTED;
    21 "Mask file"               Byte       0, ValueKind::Continuous, IMPLEMENTED;
    22 "Mask scale X"            Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
    23 "Mask scale X fine"       Fine       0, ValueKind::Continuous, IMPLEMENTED;
    24 "Mask scale Y"            Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
    25 "Mask scale Y fine"       Fine       0, ValueKind::Continuous, IMPLEMENTED;
    26 "Mask invert"             Byte       0, ValueKind::Binary { off: "Normal", on: "Inverted" }, IMPLEMENTED;
    27 "Mask opacity"            Byte       0, ValueKind::Continuous, IMPLEMENTED;
    28 "Effect 1 Select"         Byte       0, ValueKind::Continuous, IMPLEMENTED;
    29 "Effect 1 Strength"       Byte       0, ValueKind::Continuous, IMPLEMENTED;
    30 "Effect 2 Select"         Byte       0, ValueKind::Continuous, IMPLEMENTED;
    31 "Effect 2 Strength"       Byte       0, ValueKind::Continuous, IMPLEMENTED;
    32 "Speed multiplier"        Byte     127, ValueKind::SpeedMultiplier, IMPLEMENTED;
    // The effect-bank layout's Playback BPM and ignored Blur bytes carry these two instead.
    33 "3D model"                Byte       0, ValueKind::ModelSelect, IMPLEMENTED;
    34 "Blend mode"              Byte       0, ValueKind::BlendMode, IMPLEMENTED;
    35 "Mask position X"         Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
    36 "Mask position X fine"    Fine       0, ValueKind::Continuous, IMPLEMENTED;
    37 "Mask position Y"         Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
    38 "Mask position Y fine"    Fine       0, ValueKind::Continuous, IMPLEMENTED;
    39 "Effect 1 Parameter 1"    Byte       0, ValueKind::EffectParameter, IMPLEMENTED;
    40 "Effect 1 Parameter 2"    Byte       0, ValueKind::EffectParameter, IMPLEMENTED;
    41 "Effect 1 Parameter 3"    Byte       0, ValueKind::EffectParameter, IMPLEMENTED;
    42 "Effect 1 Parameter 4"    Byte       0, ValueKind::EffectParameter, IMPLEMENTED;
    43 "Effect 2 Parameter 1"    Byte       0, ValueKind::EffectParameter, IMPLEMENTED;
    44 "Effect 2 Parameter 2"    Byte       0, ValueKind::EffectParameter, IMPLEMENTED;
    45 "Effect 2 Parameter 3"    Byte       0, ValueKind::EffectParameter, IMPLEMENTED;
    46 "Effect 2 Parameter 4"    Byte       0, ValueKind::EffectParameter, IMPLEMENTED;
    // Frames. The In point counts from the clip's start, the Out point back from its end, so zero
    // on both plays the whole clip.
    47 "In point"                Coarse     0, ValueKind::Continuous, IMPLEMENTED;
    48 "In point fine"           Fine       0, ValueKind::Continuous, IMPLEMENTED;
    49 "Out point"               Coarse     0, ValueKind::Continuous, IMPLEMENTED;
    50 "Out point fine"          Fine       0, ValueKind::Continuous, IMPLEMENTED;
    51 "Visualizer Parameter 1"  Byte       0, ValueKind::VisualizerParameter, IMPLEMENTED;
    52 "Visualizer Parameter 2"  Byte       0, ValueKind::VisualizerParameter, IMPLEMENTED;
    53 "Visualizer Parameter 3"  Byte       0, ValueKind::VisualizerParameter, IMPLEMENTED;
    54 "Visualizer Parameter 4"  Byte       0, ValueKind::VisualizerParameter, IMPLEMENTED;
    55 "Model pan"               Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
    56 "Model pan fine"          Fine       0, ValueKind::Continuous, IMPLEMENTED;
    57 "Model tilt"              Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
    58 "Model tilt fine"         Fine       0, ValueKind::Continuous, IMPLEMENTED;
}

channel_table! { EFFECT_BANK_LAYER_CHANNELS;
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
    28 "Effect 1 Select"       Byte       0, ValueKind::Continuous, IMPLEMENTED;
    29 "Effect 1 Strength"     Byte       0, ValueKind::Continuous, IMPLEMENTED;
    30 "Effect 2 Select"       Byte       0, ValueKind::Continuous, IMPLEMENTED;
    31 "Effect 2 Strength"     Byte       0, ValueKind::Continuous, IMPLEMENTED;
    32 "Speed multiplier"      Byte     127, ValueKind::SpeedMultiplier, IMPLEMENTED;
    33 "Playback BPM"          Byte       0, ValueKind::PlaybackBpm, IMPLEMENTED;
    34 "Legacy Blur (ignored)" Byte       0, ValueKind::Unimplemented, LEGACY_ONLY;
    // New controls append to the published block. Existing desks therefore keep sending every
    // pre-existing control, especially playback speed, at the slot they originally patched.
    35 "Mask position X"       Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
    36 "Mask position X fine"  Fine       0, ValueKind::Continuous, IMPLEMENTED;
    37 "Mask position Y"       Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
    38 "Mask position Y fine"  Fine       0, ValueKind::Continuous, IMPLEMENTED;
}

channel_table! { MASTER_CHANNELS;
     0 "Master dimmer"               Byte     255, ValueKind::Continuous, IMPLEMENTED;
     1 "Master volume"               Byte     255, ValueKind::Continuous, IMPLEMENTED;
     2 "Master cyan"                 Byte       0, ValueKind::Continuous, IMPLEMENTED;
     3 "Master magenta"              Byte       0, ValueKind::Continuous, IMPLEMENTED;
     4 "Master yellow"               Byte       0, ValueKind::Continuous, IMPLEMENTED;
     5 "Master mask"                 Byte       0, ValueKind::Continuous, IMPLEMENTED;
     6 "Master mask position X"      Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
     7 "Master mask position X fine" Fine       0, ValueKind::Continuous, IMPLEMENTED;
     8 "Master mask position Y"      Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
     9 "Master mask position Y fine" Fine       0, ValueKind::Continuous, IMPLEMENTED;
    // -4× to +4×; a negative axis mirrors the composite. 40960 is exactly 1×.
    10 "Master scale X"              Coarse 40960, ValueKind::Continuous, IMPLEMENTED;
    11 "Master scale X fine"         Fine       0, ValueKind::Continuous, IMPLEMENTED;
    12 "Master scale Y"              Coarse 40960, ValueKind::Continuous, IMPLEMENTED;
    13 "Master scale Y fine"         Fine       0, ValueKind::Continuous, IMPLEMENTED;
    14 "Master scaling mode"         Byte       0, ValueKind::ScalingMode, IMPLEMENTED;
    15 "Master position X"           Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
    16 "Master position X fine"      Fine       0, ValueKind::Continuous, IMPLEMENTED;
    17 "Master position Y"           Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
    18 "Master position Y fine"      Fine       0, ValueKind::Continuous, IMPLEMENTED;
    19 "Master rotation"             Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
    20 "Master rotation fine"        Fine       0, ValueKind::Continuous, IMPLEMENTED;
    21 "Shaper left"                 Coarse     0, ValueKind::Continuous, IMPLEMENTED;
    22 "Shaper left fine"            Fine       0, ValueKind::Continuous, IMPLEMENTED;
    23 "Shaper right"                Coarse     0, ValueKind::Continuous, IMPLEMENTED;
    24 "Shaper right fine"           Fine       0, ValueKind::Continuous, IMPLEMENTED;
    25 "Shaper top"                  Coarse     0, ValueKind::Continuous, IMPLEMENTED;
    26 "Shaper top fine"             Fine       0, ValueKind::Continuous, IMPLEMENTED;
    27 "Shaper bottom"               Coarse     0, ValueKind::Continuous, IMPLEMENTED;
    28 "Shaper bottom fine"          Fine       0, ValueKind::Continuous, IMPLEMENTED;
    29 "Shaper left rotation"        Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
    30 "Shaper left rotation fine"   Fine       0, ValueKind::Continuous, IMPLEMENTED;
    31 "Shaper right rotation"       Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
    32 "Shaper right rotation fine"  Fine       0, ValueKind::Continuous, IMPLEMENTED;
    33 "Shaper top rotation"         Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
    34 "Shaper top rotation fine"    Fine       0, ValueKind::Continuous, IMPLEMENTED;
    35 "Shaper bottom rotation"      Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
    36 "Shaper bottom rotation fine" Fine       0, ValueKind::Continuous, IMPLEMENTED;
    37 "Shaper rotation"             Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
    38 "Shaper rotation fine"        Fine       0, ValueKind::Continuous, IMPLEMENTED;
    39 "Layer Opacity Cycle"         Byte       0, ValueKind::BeatRatio, IMPLEMENTED;
}

channel_table! { EFFECT_BANK_MASTER_CHANNELS;
     0 "Master dimmer"               Byte     255, ValueKind::Continuous, IMPLEMENTED;
     1 "Master volume"               Byte     255, ValueKind::Continuous, IMPLEMENTED;
     2 "Master cyan"                 Byte       0, ValueKind::Continuous, IMPLEMENTED;
     3 "Master magenta"              Byte       0, ValueKind::Continuous, IMPLEMENTED;
     4 "Master yellow"               Byte       0, ValueKind::Continuous, IMPLEMENTED;
     5 "Flip/mirror"                 Byte       0, ValueKind::FlipMirror, IMPLEMENTED;
     6 "Master mask"                 Byte       0, ValueKind::Continuous, IMPLEMENTED;
     7 "Master mask position X"      Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
     8 "Master mask position X fine" Fine       0, ValueKind::Continuous, IMPLEMENTED;
     9 "Master mask position Y"      Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
    10 "Master mask position Y fine" Fine       0, ValueKind::Continuous, IMPLEMENTED;
    11 "Master scale X"              Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
    12 "Master scale X fine"         Fine       0, ValueKind::Continuous, IMPLEMENTED;
    13 "Master scale Y"              Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
    14 "Master scale Y fine"         Fine       0, ValueKind::Continuous, IMPLEMENTED;
    15 "Master scaling mode"         Byte       0, ValueKind::ScalingMode, IMPLEMENTED;
    16 "Master position X"           Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
    17 "Master position X fine"      Fine       0, ValueKind::Continuous, IMPLEMENTED;
    18 "Master position Y"           Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
    19 "Master position Y fine"      Fine       0, ValueKind::Continuous, IMPLEMENTED;
    20 "Master rotation"             Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
    21 "Master rotation fine"        Fine       0, ValueKind::Continuous, IMPLEMENTED;
    22 "Shaper left"                 Coarse     0, ValueKind::Continuous, IMPLEMENTED;
    23 "Shaper left fine"            Fine       0, ValueKind::Continuous, IMPLEMENTED;
    24 "Shaper right"                Coarse     0, ValueKind::Continuous, IMPLEMENTED;
    25 "Shaper right fine"           Fine       0, ValueKind::Continuous, IMPLEMENTED;
    26 "Shaper top"                  Coarse     0, ValueKind::Continuous, IMPLEMENTED;
    27 "Shaper top fine"             Fine       0, ValueKind::Continuous, IMPLEMENTED;
    28 "Shaper bottom"               Coarse     0, ValueKind::Continuous, IMPLEMENTED;
    29 "Shaper bottom fine"          Fine       0, ValueKind::Continuous, IMPLEMENTED;
    30 "Shaper left rotation"        Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
    31 "Shaper left rotation fine"   Fine       0, ValueKind::Continuous, IMPLEMENTED;
    32 "Shaper right rotation"       Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
    33 "Shaper right rotation fine"  Fine       0, ValueKind::Continuous, IMPLEMENTED;
    34 "Shaper top rotation"         Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
    35 "Shaper top rotation fine"    Fine       0, ValueKind::Continuous, IMPLEMENTED;
    36 "Shaper bottom rotation"      Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
    37 "Shaper bottom rotation fine" Fine       0, ValueKind::Continuous, IMPLEMENTED;
    38 "Shaper rotation"             Coarse 32768, ValueKind::Continuous, IMPLEMENTED;
    39 "Shaper rotation fine"        Fine       0, ValueKind::Continuous, IMPLEMENTED;
    40 "Layer Opacity Cycle"         Byte       0, ValueKind::BeatRatio, IMPLEMENTED;
}

/// The layer table a layout decodes. Every older layout is a prefix of the effect-bank table.
pub fn layer_channels(layout: PersonalityLayout) -> &'static [ChannelSpec] {
    let table = match layout {
        PersonalityLayout::Mapping => LAYER_CHANNELS,
        _ => EFFECT_BANK_LAYER_CHANNELS,
    };
    &table[..usize::from(layout.layer_slots())]
}

/// The master table a layout decodes. Every older layout is a prefix of the effect-bank table.
pub fn master_channels(layout: PersonalityLayout) -> &'static [ChannelSpec] {
    let table = match layout {
        PersonalityLayout::Mapping => MASTER_CHANNELS,
        _ => EFFECT_BANK_MASTER_CHANNELS,
    };
    &table[..usize::from(layout.master_slots())]
}

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
    pub const EFFECT_1_SELECT: usize = 28;
    pub const EFFECT_1_STRENGTH: usize = 29;
    pub const EFFECT_2_SELECT: usize = 30;
    pub const EFFECT_2_STRENGTH: usize = 31;
    pub const SPEED_MULTIPLIER: usize = 32;
    /// Older layouts only; the mapping layout carries the 3D model here.
    pub const PLAYBACK_BPM: usize = 33;
    /// Older layouts only; the mapping layout carries Blend mode here.
    pub const BLUR: usize = 34;
    pub const MODEL: usize = 33;
    pub const BLEND: usize = 34;
    pub const MASK_POSITION_X: usize = 35;
    pub const MASK_POSITION_Y: usize = 37;
    pub const EFFECT_1_PARAMETERS: usize = 39;
    pub const EFFECT_2_PARAMETERS: usize = 43;
    pub const IN_POINT: usize = 47;
    pub const OUT_POINT: usize = 49;
    pub const VISUALIZER_PARAMETERS: usize = 51;
    pub const MODEL_PAN: usize = 55;
    pub const MODEL_TILT: usize = 57;
}

/// Zero-based offsets of the current mapping master.
pub mod master {
    pub const DIMMER: usize = 0;
    pub const VOLUME: usize = 1;
    pub const CYAN: usize = 2;
    pub const MAGENTA: usize = 3;
    pub const YELLOW: usize = 4;
    pub const MASK: usize = 5;
    pub const MASK_POSITION_X: usize = 6;
    pub const MASK_POSITION_Y: usize = 8;
    pub const SCALE_X: usize = 10;
    pub const SCALE_Y: usize = 12;
    pub const SCALING_MODE: usize = 14;
    pub const POSITION_X: usize = 15;
    pub const POSITION_Y: usize = 17;
    pub const ROTATION: usize = 19;
    pub const SHAPER_LEFT: usize = 21;
    pub const SHAPER_RIGHT: usize = 23;
    pub const SHAPER_TOP: usize = 25;
    pub const SHAPER_BOTTOM: usize = 27;
    pub const SHAPER_LEFT_ROTATION: usize = 29;
    pub const SHAPER_RIGHT_ROTATION: usize = 31;
    pub const SHAPER_TOP_ROTATION: usize = 33;
    pub const SHAPER_BOTTOM_ROTATION: usize = 35;
    pub const SHAPER_ROTATION: usize = 37;
    pub const OPACITY_CYCLE: usize = 39;
}

/// Zero-based offsets of the effect-bank master, which every older master layout is a prefix of.
pub mod effect_bank_master {
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
    pub const OPACITY_CYCLE: usize = 40;
}

#[cfg(test)]
mod tests {
    use super::super::{
        EFFECT_BANK_LAYER_SLOTS, EFFECT_BANK_MASTER_SLOTS, EFFECT_BANK_PARAMETERS, LAYER_SLOTS,
        MASTER_SLOTS, VISUALIZER_PARAMETERS,
    };
    use super::*;

    const TABLES: [&[ChannelSpec]; 4] = [
        LAYER_CHANNELS,
        MASTER_CHANNELS,
        EFFECT_BANK_LAYER_CHANNELS,
        EFFECT_BANK_MASTER_CHANNELS,
    ];

    #[test]
    fn the_table_covers_every_slot_exactly_once_in_order() {
        for (table, slots) in [
            (LAYER_CHANNELS, LAYER_SLOTS),
            (MASTER_CHANNELS, MASTER_SLOTS),
            (EFFECT_BANK_LAYER_CHANNELS, EFFECT_BANK_LAYER_SLOTS),
            (EFFECT_BANK_MASTER_CHANNELS, EFFECT_BANK_MASTER_SLOTS),
        ] {
            assert_eq!(table.len(), usize::from(slots));
            for (index, channel) in table.iter().enumerate() {
                assert_eq!(usize::from(channel.offset), index, "{}", channel.name);
            }
        }
    }

    #[test]
    fn every_coarse_channel_is_followed_by_its_fine_byte() {
        for table in TABLES {
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
        for table in TABLES {
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
    fn the_sixteen_bit_pairs_per_layer() {
        let pairs = |table: &[ChannelSpec]| {
            table
                .iter()
                .filter(|channel| channel.resolution == Resolution::Coarse)
                .count()
        };
        assert_eq!(
            pairs(EFFECT_BANK_LAYER_CHANNELS),
            9,
            "scale X/Y, position X/Y, rotation, and mask scale plus position axes"
        );
        assert_eq!(
            pairs(LAYER_CHANNELS),
            13,
            "plus in point, out point, model pan, and model tilt"
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
        assert_eq!(LAYER_CHANNELS[layer::MODEL].name, "3D model");
        assert_eq!(LAYER_CHANNELS[layer::BLEND].name, "Blend mode");
        assert_eq!(LAYER_CHANNELS[layer::IN_POINT].name, "In point");
        assert_eq!(LAYER_CHANNELS[layer::OUT_POINT].name, "Out point");
        assert_eq!(
            LAYER_CHANNELS[layer::VISUALIZER_PARAMETERS].name,
            "Visualizer Parameter 1"
        );
        assert_eq!(LAYER_CHANNELS[layer::MODEL_PAN].name, "Model pan");
        assert_eq!(LAYER_CHANNELS[layer::MODEL_TILT].name, "Model tilt");
        assert_eq!(
            EFFECT_BANK_LAYER_CHANNELS[layer::PLAYBACK_BPM].name,
            "Playback BPM"
        );
        assert_eq!(
            EFFECT_BANK_MASTER_CHANNELS[effect_bank_master::FLIP_MIRROR].name,
            "Flip/mirror"
        );
        assert_eq!(MASTER_CHANNELS[master::MASK].name, "Master mask");
        assert_eq!(MASTER_CHANNELS[master::SCALE_X].name, "Master scale X");
        assert_eq!(
            MASTER_CHANNELS[master::OPACITY_CYCLE].name,
            "Layer Opacity Cycle"
        );
        assert_eq!(
            EFFECT_BANK_MASTER_CHANNELS[effect_bank_master::MASK_POSITION_Y].name,
            "Master mask position Y"
        );
    }

    #[test]
    fn the_mapping_layer_keeps_every_shared_control_at_its_published_slot() {
        for (current, published) in LAYER_CHANNELS.iter().zip(EFFECT_BANK_LAYER_CHANNELS) {
            if matches!(usize::from(published.offset), layer::MODEL | layer::BLEND) {
                continue;
            }
            assert_eq!(current, published);
        }
    }

    #[test]
    fn the_mapping_master_drops_flip_and_keeps_the_order() {
        let published: Vec<&str> = EFFECT_BANK_MASTER_CHANNELS
            .iter()
            .map(|channel| channel.name)
            .filter(|name| *name != "Flip/mirror")
            .collect();
        let current: Vec<&str> = MASTER_CHANNELS.iter().map(|channel| channel.name).collect();
        assert_eq!(current, published);
        assert_eq!(
            MASTER_CHANNELS[master::SCALE_X].default_value,
            crate::dmx::SIGNED_MASTER_SCALE_HOME
        );
    }

    #[test]
    fn older_layouts_read_prefixes_of_the_effect_bank_tables() {
        for layout in [
            PersonalityLayout::Legacy,
            PersonalityLayout::Current,
            PersonalityLayout::Extended,
            PersonalityLayout::EffectBanks,
        ] {
            assert_eq!(
                layer_channels(layout),
                &EFFECT_BANK_LAYER_CHANNELS[..usize::from(layout.layer_slots())]
            );
            assert_eq!(
                master_channels(layout),
                &EFFECT_BANK_MASTER_CHANNELS[..usize::from(layout.master_slots())]
            );
        }
        assert_eq!(layer_channels(PersonalityLayout::Mapping), LAYER_CHANNELS);
        assert_eq!(master_channels(PersonalityLayout::Mapping), MASTER_CHANNELS);
    }

    #[test]
    fn every_channel_name_is_distinct() {
        for (layer_table, master_table) in [
            (LAYER_CHANNELS, MASTER_CHANNELS),
            (EFFECT_BANK_LAYER_CHANNELS, EFFECT_BANK_MASTER_CHANNELS),
        ] {
            let mut names: Vec<&str> = layer_table
                .iter()
                .chain(master_table)
                .map(|channel| channel.name)
                .collect();
            let total = names.len();
            names.sort_unstable();
            names.dedup();
            assert_eq!(names.len(), total);
        }
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
        let sets = EFFECT_BANK_MASTER_CHANNELS[effect_bank_master::FLIP_MIRROR]
            .values
            .sets();
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
    fn blend_sets_are_projected_from_the_blend_decoder() {
        let sets = LAYER_CHANNELS[layer::BLEND].values.sets();
        for raw in 0..=255u8 {
            let decoded = crate::blend::LayerBlend::from_dmx(raw);
            let set = matching_set(&sets, raw);
            match decoded.strobe_hz {
                Some(_) => assert_eq!(set.name, "Strobe slow–fast"),
                None if raw >= 250 => assert_eq!(set.name, "Normal, no strobe"),
                None => assert_eq!(set.name, decoded.mode.label()),
            }
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
    fn each_bank_carries_four_byte_parameters_that_default_to_the_preset() {
        for (bank, base) in [
            (1, layer::EFFECT_1_PARAMETERS),
            (2, layer::EFFECT_2_PARAMETERS),
        ] {
            for index in 0..EFFECT_BANK_PARAMETERS {
                let channel = LAYER_CHANNELS[base + index];
                assert_eq!(
                    channel.name,
                    format!("Effect {bank} Parameter {}", index + 1)
                );
                assert_eq!(channel.resolution, Resolution::Byte);
                assert_eq!(channel.default_value, 0);
                let sets = channel.values.sets();
                assert_eq!(matching_set(&sets, 0).name, "Preset value");
                assert_eq!(matching_set(&sets, 255).name, "Parameter");
            }
        }
        for index in 0..VISUALIZER_PARAMETERS {
            let channel = LAYER_CHANNELS[layer::VISUALIZER_PARAMETERS + index];
            assert_eq!(channel.name, format!("Visualizer Parameter {}", index + 1));
            assert_eq!(channel.default_value, 0);
        }
    }

    #[test]
    fn canonical_defaults_are_neutral_and_fit_their_resolution() {
        for channel in TABLES.into_iter().flatten() {
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
        assert_eq!(LAYER_CHANNELS[layer::OUT_POINT].default_value, 0);
        assert_eq!(LAYER_CHANNELS[layer::MODEL_PAN].default_value, 32_768);
        assert_eq!(MASTER_CHANNELS[master::DIMMER].default_value, 255);
        assert_eq!(MASTER_CHANNELS[master::CYAN].default_value, 0);
    }
}
