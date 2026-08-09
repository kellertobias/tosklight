//! One redraw decision shared by the standalone window and the embedded Stage pane.
//!
//! Providers and input are still polled at their normal cadence. This gate sits immediately in
//! front of GPU submission, so an unchanged picture costs no acquire, encode, submit, or present.

use std::collections::HashSet;
use uuid::Uuid;
use viz_render::OverlayQuad;
use viz_scene::{Atmosphere, PersistencePreference, SceneValues, ViewConfiguration};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RedrawState {
    pub scene_revision: u64,
    pub values_frame: u64,
    pub view: ViewConfiguration,
    pub size: (u32, u32),
    pub atmosphere: Atmosphere,
    pub selection: HashSet<Uuid>,
    pub overlay: Vec<OverlayQuad>,
}

impl RedrawState {
    pub fn new(
        scene_revision: u64,
        values: &SceneValues,
        view: &ViewConfiguration,
        size: (u32, u32),
        overlay: &[OverlayQuad],
    ) -> Self {
        Self {
            scene_revision,
            values_frame: values.frame,
            view: view.clone(),
            size,
            atmosphere: values.atmosphere,
            selection: values.selected_fixtures.clone(),
            overlay: overlay.to_vec(),
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct RedrawGate {
    last: Option<RedrawState>,
}

impl RedrawGate {
    pub fn should_draw(&mut self, state: RedrawState, time_driven: bool) -> bool {
        if !time_driven && self.last.as_ref() == Some(&state) {
            return false;
        }
        self.last = Some(state);
        true
    }
}

/// Whether display time alone can change the next picture.
pub(crate) fn is_time_driven(
    values: &SceneValues,
    view: &ViewConfiguration,
    persistence: &PersistencePreference,
) -> bool {
    values.is_time_driven(persistence)
        || (view.mode.renders_beams()
            && view.quality == viz_scene::RenderQuality::Ultra
            && values.atmosphere.density > 0.0005
            && values
                .emitters
                .iter()
                .any(|emitter| emitter.visible_intensity() > f32::EPSILON))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(frame: u64) -> RedrawState {
        let mut values = SceneValues::default();
        values.frame = frame;
        RedrawState::new(1, &values, &ViewConfiguration::default(), (800, 600), &[])
    }

    #[test]
    fn identical_static_picture_is_suppressed() {
        let mut gate = RedrawGate::default();
        assert!(gate.should_draw(state(7), false));
        assert!(!gate.should_draw(state(7), false));
    }

    #[test]
    fn changed_picture_or_time_driven_state_draws() {
        let mut gate = RedrawGate::default();
        assert!(gate.should_draw(state(7), false));
        assert!(gate.should_draw(state(8), false));
        assert!(gate.should_draw(state(8), true));
    }

    #[test]
    fn camera_size_selection_and_overlay_are_part_of_the_identity() {
        let base = state(7);
        let mut gate = RedrawGate::default();
        assert!(gate.should_draw(base.clone(), false));
        let mut changed = base.clone();
        changed.view.camera.position.x += 1.0;
        assert!(gate.should_draw(changed, false));
        let mut changed = base.clone();
        changed.size.0 += 1;
        assert!(gate.should_draw(changed, false));
        let mut changed = base.clone();
        changed.selection.insert(Uuid::nil());
        assert!(gate.should_draw(changed, false));
        let mut changed = base;
        changed.overlay.push(OverlayQuad {
            rect: [0.0; 4],
            uv_rect: [0.0; 4],
            colour: [1.0; 4],
        });
        assert!(gate.should_draw(changed, false));
    }
}
