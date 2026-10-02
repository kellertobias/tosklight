//! Atomic Direct Color capture and verified native replay eligibility.
//!
//! Capture turns one coherent premaster observation of a complete native Color path into the
//! tagged Direct value in a single step: the exact full-width values, the pinned ORIGINAL
//! identity and the portable appearance/UV knowledge predicted by that same original model.
//! Replay planning grants exact native replay only from verified identity and layout evidence.
//! It never compares fixture names, canonical aliases, channel counts or DMX slot numbers, and
//! it never recomputes the saved portable estimate from a destination or newer calibration.
use super::*;
use crate::{AttributeValue, NativeColorIdentity, NativeColorValue, PhysicalDataQuality};
use std::sync::Arc;

/// Diagnostic comparison of the exact source drive with the source model's declared maximum.
/// The captured recipe is never edited to honour the maximum.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeDriveLimit {
    Within,
    AboveModelMaximum,
    /// The model has no drive evidence (for example, optical prediction is unavailable).
    Unknown,
}

/// One source-model evaluation of a materialized recipe.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeColorPrediction {
    pub portable: PortableColorEstimate,
    pub drive_limit: NativeDriveLimit,
}

/// Exact premaster values of every control of one pinned native Color path, all sampled from
/// one captured frame. The caller supplies the source identity of the head that produced them.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeColorObservation {
    pub source: NativeColorIdentity,
    pub values: Vec<NativeColorValue>,
}

/// A validated Direct value and its capture diagnostics. Fields are private: the recipe and its
/// portable estimate can only be produced together by [`capture_direct_color`].
#[derive(Clone, Debug, PartialEq)]
pub struct DirectColorCapture {
    program: ColorProgram,
    drive_limit: NativeDriveLimit,
}

impl DirectColorCapture {
    pub fn program(&self) -> &ColorProgram {
        &self.program
    }
    pub fn recipe(&self) -> &NativeColorRecipe {
        match &self.program {
            ColorProgram::Direct { recipe, .. } => recipe,
            ColorProgram::Semantic { .. } => unreachable!("capture always produces Direct"),
        }
    }
    pub fn portable(&self) -> &PortableColorEstimate {
        match &self.program {
            ColorProgram::Direct { portable, .. } => portable,
            ColorProgram::Semantic { .. } => unreachable!("capture always produces Direct"),
        }
    }
    pub fn drive_limit(&self) -> NativeDriveLimit {
        self.drive_limit
    }
    /// Independent visible/UV knowledge an incompatible destination may resolve.
    pub fn fallback(&self) -> DirectFallback {
        DirectFallback::from_portable(self.portable())
    }
    pub fn into_value(self) -> AttributeValue {
        AttributeValue::ColorProgram(Arc::new(self.program))
    }
}

/// Capture exact native values and derive portable appearance, relative output and UV in one
/// transaction. The model must be the verified ORIGINAL of `observation.source`; it validates
/// complete ownership, duplicates, foreign/service functions and raw domains. Unknown optical
/// appearance is a valid native-only limitation; malformed controls are errors. Nothing is
/// returned unless the complete tagged value validates.
pub fn capture_direct_color(
    model: &dyn NativeColorEditModel,
    observation: NativeColorObservation,
) -> Result<DirectColorCapture, IntentError> {
    let NativeColorObservation { source, mut values } = observation;
    source.validate()?;
    require(
        model.source() == &source,
        "Direct capture requires its exact pinned original source model",
    )?;
    // Canonical order makes equal observations produce equal records, independent of the
    // adapter's write order. Values themselves are never rescaled or requantized.
    values.sort_by_key(|value| value.channel_id);
    let recipe = NativeColorRecipe {
        source,
        channels: values,
        spreads: vec![],
    };
    recipe.validate()?;
    // XYZ already contains drive response and known UV leakage with relative_output 1. It is
    // stored as predicted: no normalization, no second brightness factor, no added leakage.
    let NativeColorPrediction {
        portable,
        drive_limit,
    } = model.predict_with_status(&recipe)?;
    let program = ColorProgram::Direct { recipe, portable };
    program.validate()?;
    Ok(DirectColorCapture {
        program,
        drive_limit,
    })
}

/// What is known about a replay destination's native Color path.
pub enum DirectDestination<'a> {
    /// The destination's current verified native model (its identity is `model.source()`).
    Verified(&'a dyn NativeColorEditModel),
    /// The destination has no native Color path (for example, a dimmer-only fixture).
    NoNativeColor,
    /// The destination model could not be verified; compatibility is unknown.
    Unverified(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DirectIncompatibility {
    NoNativeColor,
    /// A different profile, mode, head or optical path, however similar its names or slots.
    DifferentSource,
    /// The same path whose native control/function layout changed.
    ChangedLayout,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DirectCompatibility {
    Compatible,
    Incompatible(DirectIncompatibility),
    Unknown(String),
}

/// Identity-only comparison. Compatible requires the same profile, mode, head and optical path
/// plus the same native layout signature (exact control/function identity, raw domains, widths
/// and behavior, excluding DMX slot placement and appearance calibration). Revision, digest and
/// model revision may differ: calibration-only changes keep native replay eligibility.
pub fn direct_color_compatibility(
    source: &NativeColorIdentity,
    destination: &DirectDestination<'_>,
) -> DirectCompatibility {
    let model = match destination {
        DirectDestination::NoNativeColor => {
            return DirectCompatibility::Incompatible(DirectIncompatibility::NoNativeColor);
        }
        DirectDestination::Unverified(reason) => {
            return DirectCompatibility::Unknown(reason.clone());
        }
        DirectDestination::Verified(model) => model,
    };
    let destination = model.source();
    if (
        source.profile_id,
        source.mode_id,
        source.head_id,
        source.path_id,
    ) != (
        destination.profile_id,
        destination.mode_id,
        destination.head_id,
        destination.path_id,
    ) {
        DirectCompatibility::Incompatible(DirectIncompatibility::DifferentSource)
    } else if source.native_layout_signature != destination.native_layout_signature {
        DirectCompatibility::Incompatible(DirectIncompatibility::ChangedLayout)
    } else {
        DirectCompatibility::Compatible
    }
}

/// Visible part of an incompatible replay.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VisibleFallback {
    /// Fit this TOTAL visible XYZ at this relative output. It already includes brightness and
    /// known UV leakage; the destination must not scale it by Y or add leakage again. Zero XYZ
    /// is known black, never a chromaticity to normalize.
    Fit(PortableVisibleColor),
    /// Unknown appearance: hold the last valid visible solution, or declared safe defaults
    /// before a first valid solution. Never invent white.
    Hold,
}

/// UV part of an incompatible replay, independent of visible knowledge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UvFallback {
    /// Apply this normalized drive, including a known zero.
    Apply(PortableUv),
    /// Unknown portable UV (for example, unequal independent banks): park UV off explicitly.
    ParkOff,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DirectFallback {
    pub visible: VisibleFallback,
    pub uv: UvFallback,
    pub quality: PhysicalDataQuality,
    pub limitations: Vec<String>,
}

impl DirectFallback {
    /// Derive only from the SAVED estimate; never from a destination or newer source model.
    pub fn from_portable(portable: &PortableColorEstimate) -> Self {
        let mut limitations = portable.limitations.clone();
        let visible = match portable.visible {
            Some(visible) => VisibleFallback::Fit(visible),
            None => {
                limitations.push(
                    "Direct appearance is unknown; the visible solution is held, not matched."
                        .into(),
                );
                VisibleFallback::Hold
            }
        };
        let uv = match portable.uv {
            Some(uv) => UvFallback::Apply(uv),
            None => {
                limitations.push("Direct UV amount is unknown; UV is parked off.".into());
                UvFallback::ParkOff
            }
        };
        Self {
            visible,
            uv,
            quality: portable.quality,
            limitations,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum DirectReplay {
    /// Replay the recorded recipe unchanged, spreads included, on the verified destination.
    Exact { recipe: NativeColorRecipe },
    /// Fit the saved independent knowledge through the normal destination resolver.
    Fallback {
        compatibility: DirectCompatibility,
        fallback: DirectFallback,
    },
}

/// Decide exact replay or explicit visible/UV fallback for one recorded Direct value. A
/// destination whose identity is compatible but which rejects the recipe proves the record
/// invalid; that is an error, not a silent fallback.
pub fn plan_direct_replay(
    program: &ColorProgram,
    destination: &DirectDestination<'_>,
) -> Result<DirectReplay, IntentError> {
    let ColorProgram::Direct { recipe, portable } = program else {
        return Err(IntentError(
            "Direct replay planning requires a Direct Color value".into(),
        ));
    };
    program.validate()?;
    let compatibility = direct_color_compatibility(&recipe.source, destination);
    match (&compatibility, destination) {
        (DirectCompatibility::Compatible, DirectDestination::Verified(model)) => {
            verify_on_destination(*model, recipe)?;
            Ok(DirectReplay::Exact {
                recipe: recipe.clone(),
            })
        }
        _ => Ok(DirectReplay::Fallback {
            compatibility,
            fallback: DirectFallback::from_portable(portable),
        }),
    }
}

/// Complete-recipe verification against the destination's own descriptors. The destination's
/// prediction is only a completeness proof and is discarded: calibration-only changes must
/// not silently recompute the saved portable estimate.
fn verify_on_destination(
    model: &dyn NativeColorEditModel,
    recipe: &NativeColorRecipe,
) -> Result<(), IntentError> {
    let reference = NativeColorRecipe {
        source: model.source().clone(),
        channels: recipe.channels.clone(),
        spreads: vec![],
    };
    model.predict(&reference)?;
    for spread in &recipe.spreads {
        let descriptor = model.descriptor(spread.binding).ok_or_else(|| {
            IntentError("Direct spread function is not owned by the destination".into())
        })?;
        spread.validate(descriptor)?;
    }
    Ok(())
}
