//! Media Server controls derived from semantic Color intent (TL-593).
//!
//! The Media Server interprets the shared Color intent differently from a lamp: its **White
//! Blend is source desaturation** and its RGB tint stays active at 100%. This module is the one
//! desk-side translation of a composed [`ColorIntent`] into the Media personality's existing
//! controls, and the one reader of what those controls mean to the Media decoder:
//!
//! ```text
//! tint        = linear sRGB/Rec.709 of base XYZ × relativeOutput      (never lamp-whitened XYZ)
//! White Blend = the intent's White Blend on a layer; absent on the master
//! wire        = cyan/magenta/yellow = 1 − tint, grayscale = White Blend (Media personality)
//! ```
//!
//! # Transfer boundary
//!
//! The base XYZ is linear light; the tint stays linear from here to the Media shader, which
//! multiplies linear source pixels by it (`media_domain::MediaColor::apply_linear`). The only
//! transfer conversions in the whole path are the authoring engine's single sRGB decode of the
//! recipe into XYZ and the renderer's decode/encode of each pixel. Nothing here applies an sRGB
//! transfer function.
//!
//! # Ownership
//!
//! The derived controls are never stored and never written back into the intent. White
//! temperature and Duv are retained by the intent but inapplicable to Media; they never alter the
//! tint. UV has no Media equivalent and is reported unsupported, never rendered as violet.
//! Layer dimmer, alpha and the master dimmer are separate Intensity controls and are not touched.
use crate::{ChannelBehavior, ChannelFunctionBehavior, ChannelResolution, FixtureMode};
use light_core::programming::{ColorIntent, IntentError};
use light_core::xyz_to_linear_srgb;
use uuid::Uuid;

/// Tolerance of the XYZ → linear sRGB round trip of an in-gamut recipe; far below one 8-bit step.
const GAMUT_EPSILON: f64 = 1e-4;

/// Which Media color stage a head drives.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MediaColorSurface {
    /// A layer: tint and White Blend (the personality's Grayscale control).
    Layer,
    /// The master: tint only. It has no White Blend control, so White Blend is absent there —
    /// unsupported and passed through, which is not the same as an authored 0%.
    Master,
}

/// Passive capability notes of one translation. Never a notification or a rejection.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MediaColorLimitations {
    /// The base color lies outside linear sRGB (or above full scale after relativeOutput); it
    /// was mapped into the tint range by clamping negative primaries and normalizing the peak.
    pub tint_gamut_mapped: bool,
    /// A nonzero White Blend was requested on a surface without a White Blend control.
    pub white_blend_unsupported: bool,
    /// UV was requested; Media has no UV output.
    pub uv_unsupported: bool,
}

/// The Media color controls in domain units: exactly what `media_domain::MediaColor` holds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MediaColorControls {
    /// Linear RGB multipliers in `0..=1`; white is the neutral tint.
    pub tint: [f32; 3],
    /// Source desaturation in `0..=1`, or `None` where the surface has no White Blend control.
    pub white_blend: Option<f32>,
}

/// Wire values of [`MediaColorControls`] for controls of `raw_max` full scale.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MediaColorRaw {
    /// Cyan, magenta, yellow: subtractive, so zero is a full tint component.
    pub subtractive: [u32; 3],
    /// Grayscale; `None` on a surface without it.
    pub white_blend: Option<u32>,
}

impl MediaColorControls {
    /// Translate one composed semantic Color intent for a Media surface.
    ///
    /// The intent must be valid and fully resolved for one head (no pending spreads). The result
    /// is derived output, never stored and never fed back into the intent.
    pub fn from_intent(
        intent: &ColorIntent,
        surface: MediaColorSurface,
    ) -> Result<(Self, MediaColorLimitations), IntentError> {
        intent.validate()?;
        if !intent.spreads.is_empty() {
            return Err(IntentError(
                "Media Color requires a Color intent resolved for one head".into(),
            ));
        }
        let (tint, tint_gamut_mapped) = linear_tint(intent);
        let white_blend = match surface {
            MediaColorSurface::Layer => Some(intent.white_blend),
            MediaColorSurface::Master => None,
        };
        let limitations = MediaColorLimitations {
            tint_gamut_mapped,
            white_blend_unsupported: white_blend.is_none() && intent.white_blend > 0.0,
            uv_unsupported: intent.uv.amount > 0.0,
        };
        Ok((Self { tint, white_blend }, limitations))
    }

    /// Wire values for controls of `raw_max` full scale, rounded to the nearest step.
    pub fn encode(&self, raw_max: u32) -> MediaColorRaw {
        let scale = |fraction: f32| {
            (f64::from(fraction.clamp(0.0, 1.0)) * f64::from(raw_max)).round() as u32
        };
        MediaColorRaw {
            subtractive: self.tint.map(|component| scale(1.0 - component)),
            white_blend: self.white_blend.map(scale),
        }
    }

    /// What the Media decoder reads from these wire values: the achieved controls.
    /// Mirrors `media_domain::dmx::{subtractive, unit}` for 8-bit controls.
    pub fn decode(raw: MediaColorRaw, raw_max: u32) -> Self {
        let unit = |value: u32| (f64::from(value.min(raw_max)) / f64::from(raw_max)) as f32;
        Self {
            tint: raw.subtractive.map(|value| 1.0 - unit(value)),
            white_blend: raw.white_blend.map(unit),
        }
    }
}

/// Base XYZ in linear sRGB primaries, scaled by relativeOutput and mapped into `0..=1`.
/// Black (zero XYZ or zero relativeOutput) stays exactly black.
fn linear_tint(intent: &ColorIntent) -> ([f32; 3], bool) {
    let output = f64::from(intent.relative_output);
    let mut rgb = xyz_to_linear_srgb(intent.base_xyz).map(|component| component * output);
    let mut mapped = rgb.iter().any(|component| *component < -GAMUT_EPSILON);
    rgb = rgb.map(|component| component.max(0.0));
    let peak = rgb.into_iter().fold(0.0, f64::max);
    if peak > 1.0 + GAMUT_EPSILON {
        mapped = true;
        rgb = rgb.map(|component| component / peak);
    }
    (
        rgb.map(|component| component.clamp(0.0, 1.0) as f32),
        mapped,
    )
}

/// One Media color control of a profile mode, addressed like every native write.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MediaColorControl {
    /// Index into the mode's channel list.
    pub channel_index: u32,
    pub channel_id: Uuid,
    pub split: u16,
    pub raw_max: u32,
}

/// The Media color controls of one profile head, recognised by the Media personality's native
/// channel identities. A head that is not a complete Media color head is not one: lamp CMY
/// profiles never match.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MediaColorHead {
    pub head_id: Uuid,
    pub surface: MediaColorSurface,
    /// Cyan, magenta, yellow.
    pub subtractive: [MediaColorControl; 3],
    /// The layer Grayscale control; `None` on the master.
    pub white_blend: Option<MediaColorControl>,
}

const LAYER_ATTRIBUTES: [&str; 4] = [
    "media.layer.cyan",
    "media.layer.magenta",
    "media.layer.yellow",
    "media.layer.grayscale",
];
const MASTER_ATTRIBUTES: [&str; 3] = [
    "media.master.master.cyan",
    "media.master.master.magenta",
    "media.master.master.yellow",
];

/// A native Media color identity reserves this head for Media semantics even when its wire
/// contract is unsupported. Such a head must remain passive rather than fall back to lamp fitting.
pub fn has_media_color_identity(mode: &FixtureMode, head_id: Uuid) -> bool {
    mode.channels.iter().any(|channel| {
        channel.head_id == head_id
            && LAYER_ATTRIBUTES
                .iter()
                .chain(&MASTER_ATTRIBUTES)
                .any(|attribute| &*channel.fixture_attribute.0 == *attribute)
    })
}

impl MediaColorHead {
    /// The Media color head `head_id` of `mode`, if it is one. Every control must exist exactly
    /// once on the head and match the supported controlled, non-inverted, full-range 8-bit
    /// continuous Media wire contract with no Intensity/master scaling. Other personalities
    /// are declined, never guessed.
    pub fn from_mode(mode: &FixtureMode, head_id: Uuid) -> Option<Self> {
        let find = |attribute: &str| -> Option<Option<MediaColorControl>> {
            let mut matches = mode.channels.iter().enumerate().filter(|(_, channel)| {
                channel.head_id == head_id && &*channel.fixture_attribute.0 == attribute
            });
            let Some((index, channel)) = matches.next() else {
                return Some(None);
            };
            // This adapter writes the existing Media personality's plain 8-bit wire controls.
            // A retained native name does not prove an arbitrary profile has that wire contract.
            if matches.next().is_some()
                || channel.invert
                || channel.resolution != ChannelResolution::U8
                || channel.behavior != ChannelBehavior::Controlled
                || channel.reacts_to_virtual_intensity
                || channel.reacts_to_sequence_master
                || channel.reacts_to_group_master
                || channel.reacts_to_grand_master
            {
                return None;
            }
            let [function] = channel.functions.as_slice() else {
                return None;
            };
            if function.dmx_from != 0
                || function.dmx_to != 255
                || function.attribute != channel.attribute
                || !matches!(
                    &function.behavior,
                    ChannelFunctionBehavior::Continuous {
                        physical_min: 0.0,
                        physical_max: 255.0,
                        unit: None,
                    }
                )
            {
                return None;
            }
            Some(Some(MediaColorControl {
                channel_index: u32::try_from(index).ok()?,
                channel_id: channel.id,
                split: channel.split,
                raw_max: channel.resolution.max_raw(),
            }))
        };
        let layer = LAYER_ATTRIBUTES.map(find);
        let master = MASTER_ATTRIBUTES.map(find);
        // A duplicated or inverted Media control anywhere on the head disqualifies it.
        if layer.iter().chain(&master).any(Option::is_none) {
            return None;
        }
        let [c, m, y, g] = layer.map(Option::flatten);
        let [master_c, master_m, master_y] = master.map(Option::flatten);
        let head = match ((c, m, y, g), (master_c, master_m, master_y)) {
            ((Some(c), Some(m), Some(y), Some(g)), (None, None, None)) => Self {
                head_id,
                surface: MediaColorSurface::Layer,
                subtractive: [c, m, y],
                white_blend: Some(g),
            },
            ((None, None, None, None), (Some(c), Some(m), Some(y))) => Self {
                head_id,
                surface: MediaColorSurface::Master,
                subtractive: [c, m, y],
                white_blend: None,
            },
            _ => return None,
        };
        let raw_max = head.raw_max();
        head.controls()
            .iter()
            .all(|control| control.raw_max == raw_max)
            .then_some(head)
    }

    /// Full scale shared by every control of the head.
    pub fn raw_max(&self) -> u32 {
        self.subtractive[0].raw_max
    }

    /// The complete footprint: cyan, magenta, yellow, then Grayscale on a layer.
    pub fn controls(&self) -> Vec<MediaColorControl> {
        self.subtractive
            .iter()
            .chain(self.white_blend.iter())
            .copied()
            .collect()
    }

    /// Translate one composed intent for this head: the derived controls, one raw per footprint
    /// control and the controls the Media decoder reads back from exactly those raws.
    pub fn resolve(&self, intent: &ColorIntent) -> Result<MediaColorResolution, IntentError> {
        let (requested, limitations) = MediaColorControls::from_intent(intent, self.surface)?;
        let wire = requested.encode(self.raw_max());
        let mut raws = wire.subtractive.to_vec();
        raws.extend(wire.white_blend);
        debug_assert_eq!(raws.len(), self.controls().len());
        Ok(MediaColorResolution {
            requested,
            raws,
            achieved: MediaColorControls::decode(wire, self.raw_max()),
            limitations,
        })
    }
}

/// One head's translation. `raws` follow [`MediaColorHead::controls`] order.
#[derive(Clone, Debug, PartialEq)]
pub struct MediaColorResolution {
    /// Derived controls before wire quantization.
    pub requested: MediaColorControls,
    pub raws: Vec<u32>,
    /// What the Media decoder reads from `raws`.
    pub achieved: MediaColorControls,
    pub limitations: MediaColorLimitations,
}

#[cfg(test)]
#[path = "media_color_tests.rs"]
mod tests;
