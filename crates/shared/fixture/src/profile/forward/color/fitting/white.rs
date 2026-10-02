//! Semantic Color request arithmetic: the white target and the lamp visible target.
//!
//! Colorimetric conversion only; no emitter spectra or fixture data are involved.
use light_core::Xyz;
use light_core::programming::{ColorIntent, IntentError, WhiteTarget};

/// Krystek (1985) rational approximation of the Planckian locus in CIE 1960 (u, v).
/// Documented accuracy is better than 1e-4 (u, v) within 1000–15000 K; the requested domain
/// continues smoothly to 20000 K.
fn planckian_uv(kelvin: f64) -> (f64, f64) {
    let t = kelvin;
    let u = (0.860_117_757 + 1.541_182_54e-4 * t + 1.286_412_12e-7 * t * t)
        / (1.0 + 8.424_202_35e-4 * t + 7.081_451_63e-7 * t * t);
    let v = (0.317_398_726 + 4.228_062_45e-5 * t + 4.204_816_91e-8 * t * t)
        / (1.0 - 2.897_418_16e-5 * t + 1.614_560_53e-7 * t * t);
    (u, v)
}

/// White target chromaticity at relative luminance Y = 1, the reference-white scale of the
/// semantic intent. Positive Duv lies above the locus (toward green), negative toward magenta.
pub fn white_target_xyz(target: WhiteTarget) -> Xyz {
    let kelvin = f64::from(target.kelvin).clamp(1000.0, 20000.0);
    let (u0, v0) = planckian_uv(kelvin);
    // Tangent from a symmetric reciprocal-temperature difference, then its upward normal.
    let (ua, va) = planckian_uv(kelvin * 0.999);
    let (ub, vb) = planckian_uv(kelvin * 1.001);
    let (du, dv) = (ub - ua, vb - va);
    let length = du.hypot(dv).max(f64::MIN_POSITIVE);
    let (mut nu, mut nv) = (-dv / length, du / length);
    if nv < 0.0 {
        (nu, nv) = (-nu, -nv);
    }
    let duv = f64::from(target.duv);
    let (u, v) = (u0 + duv * nu, v0 + duv * nv);
    let denominator = 2.0 * u - 8.0 * v + 4.0;
    let x = 3.0 * u / denominator;
    let y = 2.0 * v / denominator;
    Xyz {
        x: (x / y) as f32,
        y: 1.0,
        z: ((1.0 - x - y) / y) as f32,
    }
}

/// The derived request, never written back into the stored intent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct DerivedRequest {
    /// `relativeOutput × (colored × base + white × white target)`.
    pub visible: Xyz,
    pub white: Xyz,
    /// Colored and white contributions after `relativeOutput`, for recipe-preserving allocation.
    pub colored_part: [f64; 3],
    pub white_part: [f64; 3],
}

/// Lamp visible target of a validated local intent. UV and Intensity are deliberately absent.
pub fn requested_visible_xyz(intent: &ColorIntent) -> Result<Xyz, IntentError> {
    derive(intent).map(|request| request.visible)
}

pub(super) fn derive(intent: &ColorIntent) -> Result<DerivedRequest, IntentError> {
    let white = white_target_xyz(intent.white_target);
    let visible = intent.blend_visible(white)?;
    let blend = f64::from(intent.white_blend);
    let output = f64::from(intent.relative_output);
    let colored = output * (2.0 * (1.0 - blend)).min(1.0);
    let whiteness = output * (2.0 * blend).min(1.0);
    let base = xyz_array(intent.base_xyz);
    let white_array = xyz_array(white);
    Ok(DerivedRequest {
        visible,
        white,
        colored_part: base.map(|v| v * colored),
        white_part: white_array.map(|v| v * whiteness),
    })
}

pub(super) fn xyz_array(value: Xyz) -> [f64; 3] {
    [f64::from(value.x), f64::from(value.y), f64::from(value.z)]
}
