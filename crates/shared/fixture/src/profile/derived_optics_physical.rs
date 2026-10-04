//! Derived Zoom opening conventions and degree ranges for modes that author none.
//!
//! The Live Zoom adapter, the engine's physical projection (Stage/preview) and the encoder pages
//! read a Zoom function's physical range through [`super::CompiledPhysicalMapping`]. A degree
//! request needs explicit degree units and an opening convention (Beam/Field); a Zoom function
//! in normalized or percent travel otherwise reads as unsupported. This module derives both in
//! the transient runtime projection only ([`super::apply_runtime_profile_compatibility`]), so it
//! never changes stored profile bytes, digests or package files:
//!
//! - a known shipped profile gets its manufacturer's documented range in the direction its DMX
//!   chart gives (Robin DLS and LEDBeam 150 run from maximum to minimum beam angle), labelled
//!   `manufacturer` with the source when range, convention and direction are all documented,
//!   else `estimated` with the source and what is assumed;
//! - any other lighting fixture gets the nominal range of its fixture type (the same nominal
//!   cone the Stage already draws for it), labelled `estimated` with [`DERIVED_ZOOM_SOURCE`]:
//!   travel 0 (or 0 %) is the narrow end, so an authored descending travel stays descending;
//! - the convention is Beam unless a source says otherwise (JBLED A7 documents a field angle);
//! - raw encoding, function boundaries and defaults are unchanged;
//! - an authored degree mapping with a convention is untouched.
//!
//! A mode it cannot describe honestly keeps its function as authored and gets a named
//! [`ZoomDerivationExclusion`]: the `zoom` attribute of an effect or a laser that is not a beam
//! opening, an authored calibration in another unit, degrees without a convention, an unknown
//! unit, or an ambiguous channel.
use super::{
    ChannelFunctionBehavior, FixtureMode, FixtureProfile, OpeningConvention, PhysicalDataQuality,
    PhysicalMappingCalibration, PhysicalUnit,
};

mod ranges;
use ranges::{ZoomRange, zoom_range};

/// Provenance text of a nominal Zoom range derived from the fixture type.
pub const DERIVED_ZOOM_SOURCE: &str = "Estimated: nominal Zoom opening for the fixture type, derived at runtime because the profile declares no degrees; Beam convention, travel 0 = narrow, endpoints and curve unverified; not fixture data";

const ATTRIBUTE: &str = "zoom";

/// Why a mode with a Zoom function gets no derived degree range or convention.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ZoomDerivationExclusion {
    /// An effect or atmosphere device whose `zoom` attribute drives something else (spark
    /// lifetime, fluid colour); it has no beam opening.
    NotABeamOpening,
    /// A laser's `zoom` is its scan pattern size, not a beam opening.
    LaserPatternSize,
    /// The function declares degrees but no Beam/Field convention; which one is not invented.
    DegreesWithoutConvention,
    /// An authored calibration in another unit is never overwritten by an estimate.
    AuthoredCalibration,
    /// The unit (or its endpoints) is not normalized or percent travel.
    UnknownUnit,
    /// No continuous Zoom function to fit.
    NoContinuousFunction,
    /// One channel carries several continuous Zoom functions.
    AmbiguousFunctions,
}

impl ZoomDerivationExclusion {
    pub fn reason(self) -> &'static str {
        match self {
            Self::NotABeamOpening => "the zoom attribute of this effect is not a beam opening",
            Self::LaserPatternSize => "a laser's zoom is its scan pattern size, not a beam opening",
            Self::DegreesWithoutConvention => {
                "the Zoom declares degrees without a beam/field convention"
            }
            Self::AuthoredCalibration => "the Zoom has an authored calibration in another unit",
            Self::UnknownUnit => "the Zoom unit is not normalized or percent travel",
            Self::NoContinuousFunction => "the Zoom has no continuous function",
            Self::AmbiguousFunctions => "one Zoom channel has several continuous functions",
        }
    }
}

/// The derived opening of a mode: degrees at the function's `physical_min` and `physical_max`.
#[derive(Clone, Debug, PartialEq)]
pub struct DerivedZoom {
    pub convention: OpeningConvention,
    pub degrees: (f32, f32),
    pub quality: PhysicalDataQuality,
    pub source: &'static str,
}

/// What the runtime projection does with one mode's Zoom.
#[derive(Clone, Debug, PartialEq)]
pub enum ZoomDerivation {
    /// No Zoom function at all.
    NotApplicable,
    /// The profile authors degrees with a convention; it is left untouched.
    Authored,
    /// Every continuous Zoom function of the mode gets this range (per its own direction).
    Derived(Vec<DerivedZoom>),
    Excluded(ZoomDerivationExclusion),
}

/// Give every mode without an authored Zoom convention the derived degree range and convention,
/// when its Zoom functions can be described. Authored mappings are never touched.
pub fn apply_derived_zoom_physical(profile: &mut FixtureProfile) {
    let Some(range) = profile_range(profile) else {
        return;
    };
    for mode in &mut profile.modes {
        if !matches!(classify_mode(mode, &range), ZoomDerivation::Derived(_)) {
            continue;
        }
        for function in mode
            .channels
            .iter_mut()
            .flat_map(|c| c.functions.iter_mut())
            .filter(|f| f.attribute.0.as_ref() == ATTRIBUTE)
        {
            let ChannelFunctionBehavior::Continuous {
                physical_min,
                physical_max,
                unit,
            } = &mut function.behavior
            else {
                continue;
            };
            let Some(domain) = travel_domain(unit.as_deref()) else {
                continue;
            };
            let (min, max) = range.degrees((*physical_min, *physical_max), domain);
            *physical_min = min;
            *physical_max = max;
            *unit = Some("degrees".into());
            function.physical_mapping = Some(PhysicalMappingCalibration {
                quality: range.quality,
                source: Some(range.source.into()),
                revision: 0,
                samples: Vec::new(),
                opening_convention: Some(range.convention),
            });
        }
    }
}

/// Classify one mode exactly as [`apply_derived_zoom_physical`] would treat it.
pub fn zoom_derivation(profile: &FixtureProfile, mode: &FixtureMode) -> ZoomDerivation {
    if !has_zoom(mode) {
        return ZoomDerivation::NotApplicable;
    }
    match profile_range(profile) {
        Some(range) => classify_mode(mode, &range),
        None => {
            let laser = words(&profile.fixture_type).iter().any(|w| w == "laser");
            ZoomDerivation::Excluded(if laser {
                ZoomDerivationExclusion::LaserPatternSize
            } else {
                ZoomDerivationExclusion::NotABeamOpening
            })
        }
    }
}

fn has_zoom(mode: &FixtureMode) -> bool {
    mode.channels
        .iter()
        .flat_map(|c| &c.functions)
        .any(|f| f.attribute.0.as_ref() == ATTRIBUTE)
}

fn words(value: &str) -> Vec<String> {
    value
        .to_ascii_lowercase()
        .replace(['_', '-'], " ")
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

/// The profile's Zoom range, or `None` when its fixture type has no beam opening.
fn profile_range(profile: &FixtureProfile) -> Option<ZoomRange> {
    zoom_range(profile.id.0, &words(&profile.fixture_type))
}

/// The normalized travel domain of a non-degree unit: normalized/unknown 0..1, percent 0..100.
fn travel_domain(unit: Option<&str>) -> Option<f32> {
    match PhysicalUnit::parse(unit) {
        PhysicalUnit::Normalized | PhysicalUnit::Unknown => Some(1.0),
        PhysicalUnit::Percent => Some(100.0),
        _ => None,
    }
}

fn classify_mode(mode: &FixtureMode, range: &ZoomRange) -> ZoomDerivation {
    use ZoomDerivationExclusion as X;
    let mut derived = Vec::new();
    for channel in &mode.channels {
        let zoom: Vec<_> = channel
            .functions
            .iter()
            .filter(|f| f.attribute.0.as_ref() == ATTRIBUTE)
            .collect();
        if zoom.is_empty() {
            continue;
        }
        let continuous: Vec<_> = zoom
            .iter()
            .filter_map(|f| match &f.behavior {
                ChannelFunctionBehavior::Continuous {
                    physical_min,
                    physical_max,
                    unit,
                } => Some((*f, *physical_min, *physical_max, unit.as_deref())),
                _ => None,
            })
            .collect();
        let [(function, min, max, unit)] = continuous.as_slice() else {
            return ZoomDerivation::Excluded(if continuous.is_empty() {
                X::NoContinuousFunction
            } else {
                X::AmbiguousFunctions
            });
        };
        let calibration = function.physical_mapping.as_ref();
        if PhysicalUnit::parse(*unit) == PhysicalUnit::Degrees {
            if calibration.is_some_and(|c| c.opening_convention.is_some()) {
                return ZoomDerivation::Authored;
            }
            return ZoomDerivation::Excluded(X::DegreesWithoutConvention);
        }
        if calibration.is_some() {
            return ZoomDerivation::Excluded(X::AuthoredCalibration);
        }
        let Some(domain) = travel_domain(*unit) else {
            return ZoomDerivation::Excluded(X::UnknownUnit);
        };
        let inside = |v: f32| v.is_finite() && (0.0..=domain).contains(&v);
        if !inside(*min) || !inside(*max) || min == max {
            return ZoomDerivation::Excluded(X::UnknownUnit);
        }
        derived.push(DerivedZoom {
            convention: range.convention,
            degrees: range.degrees((*min, *max), domain),
            quality: range.quality,
            source: range.source,
        });
    }
    if derived.is_empty() {
        ZoomDerivation::Excluded(X::NoContinuousFunction)
    } else {
        ZoomDerivation::Derived(derived)
    }
}
