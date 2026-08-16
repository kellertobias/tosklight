use super::{EmitterBinding, MotionAxes};
use crate::fallback::{self, OpticalClass};
use glam::Vec3;
use viz_scene::{EmitterOptics, MotionAxis};

/// Spread fallback heads along the body so a bar's heads do not stack on one point.
pub(super) fn head_offset(
    head_index: usize,
    count: usize,
    face_width: f32,
    body_width: f32,
) -> Vec3 {
    if count <= 1 {
        return Vec3::ZERO;
    }
    let position = head_index as f32 / (count - 1) as f32 - 0.5;
    Vec3::new(position * (body_width - face_width).max(0.0), 0.0, 0.0)
}

/// How far along the body the fallback layout spreads a fixture's heads, in metres.
pub(super) fn head_span(class: OpticalClass) -> f32 {
    match class {
        OpticalClass::Emissive | OpticalClass::Blinder => 1.0,
        _ => 0.6,
    }
}

/// Fit one fallback head's source face to the pitch between neighbouring heads.
pub(super) fn fitted_to_head_pitch(
    optics: &EmitterOptics,
    head_count: usize,
    body_width: f32,
) -> EmitterOptics {
    let mut fitted = optics.clone();
    if head_count < 2 {
        return fitted;
    }
    let pitch = body_width / head_count as f32;
    let bound = (pitch * 0.9).max(0.01);
    fitted.source.width = fitted.source.width.min(bound);
    fitted.source.height = fitted.source.height.min(bound);
    fitted
}

pub(super) fn pan_axis(motion: &MotionAxes, binding: &EmitterBinding) -> Option<MotionAxis> {
    motion.pan.or_else(|| {
        binding.pan.as_ref().map(|_| MotionAxis {
            axis: Vec3::Y,
            min_degrees: -fallback::FALLBACK_PAN_DEGREES,
            max_degrees: fallback::FALLBACK_PAN_DEGREES,
        })
    })
}

pub(super) fn tilt_axis(motion: &MotionAxes, binding: &EmitterBinding) -> Option<MotionAxis> {
    motion.tilt.or_else(|| {
        binding.tilt.as_ref().map(|_| MotionAxis {
            axis: Vec3::X,
            min_degrees: -fallback::FALLBACK_TILT_DEGREES,
            max_degrees: fallback::FALLBACK_TILT_DEGREES,
        })
    })
}
