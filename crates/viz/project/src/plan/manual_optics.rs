//! Installed manual focusing cannot compete with an authored live Zoom role.
use super::*;

pub(super) fn apply_zoom(
    scene: &mut Scene,
    bindings: &[EmitterBinding],
    start: usize,
    instance: &PhysicalInstance,
) {
    let Some(angle) = instance
        .installed_appearance
        .manual_optics
        .as_ref()
        .and_then(|optics| optics.zoom_degrees)
    else {
        return;
    };
    if !angle.is_finite() || !(1.0..=170.0).contains(&angle) {
        return;
    }
    for (emitter, binding) in scene.emitters[start..].iter_mut().zip(&bindings[start..]) {
        if binding.zoom.is_none() {
            emitter.beam_angle_degrees = angle;
            emitter.field_angle_degrees = angle;
        }
    }
}
