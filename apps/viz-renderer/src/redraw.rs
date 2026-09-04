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
    pub media_revision: u64,
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
        media_revision: u64,
        values: &SceneValues,
        view: &ViewConfiguration,
        size: (u32, u32),
        overlay: &[OverlayQuad],
    ) -> Self {
        Self {
            scene_revision,
            media_revision,
            values_frame: values.frame,
            view: *view,
            size,
            atmosphere: values.atmosphere,
            selection: values.selected_fixtures.clone(),
            overlay: overlay.to_vec(),
        }
    }
}

/// Everything the fixture labels are built from.
///
/// Building them raycasts the whole scene once per fixture, so both the window and the embedded
/// pane keep the quads from the last build and reuse them while none of this has changed and the
/// picture is not moving on its own.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LabelInputs {
    pub scene_revision: u64,
    pub values_frame: u64,
    pub view: ViewConfiguration,
    pub size: (u32, u32),
    pub selection: HashSet<Uuid>,
}

impl LabelInputs {
    pub fn new(
        scene_revision: u64,
        values: &SceneValues,
        view: &ViewConfiguration,
        size: (u32, u32),
    ) -> Self {
        Self {
            scene_revision,
            values_frame: values.frame,
            view: *view,
            size,
            selection: values.selected_fixtures.clone(),
        }
    }
}

/// The fixture labels as last built, and what they were built from.
#[derive(Debug, Default)]
pub(crate) struct LabelCache {
    inputs: Option<LabelInputs>,
    quads: Vec<OverlayQuad>,
}

impl LabelCache {
    /// Append the labels for `inputs` to `overlay`, building them with `build` only when
    /// something they read changed or the picture is time-driven.
    pub fn append(
        &mut self,
        overlay: &mut viz_render::Overlay,
        inputs: LabelInputs,
        time_driven: bool,
        build: impl FnOnce(&mut viz_render::Overlay),
    ) {
        if !time_driven && self.inputs.as_ref() == Some(&inputs) {
            overlay.quads.extend_from_slice(&self.quads);
            return;
        }
        let start = overlay.quads.len();
        build(overlay);
        self.quads.clear();
        self.quads.extend_from_slice(&overlay.quads[start..]);
        self.inputs = Some(inputs);
    }
}

#[derive(Debug, Default)]
pub(crate) struct RedrawGate {
    last: Option<RedrawState>,
    /// The fixture labels from the last frame, reused while nothing they read has changed.
    pub labels: LabelCache,
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
        let values = SceneValues {
            frame,
            ..SceneValues::default()
        };
        RedrawState::new(
            1,
            0,
            &values,
            &ViewConfiguration::default(),
            (800, 600),
            &[],
        )
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

        let mut changed = state(7);
        changed.media_revision += 1;
        assert!(gate.should_draw(changed, false));
    }
}

#[cfg(test)]
mod label_cache_tests {
    use super::*;

    fn quad(x: f32) -> OverlayQuad {
        OverlayQuad {
            rect: [x, 0.0, 1.0, 1.0],
            uv_rect: [0.0; 4],
            colour: [1.0; 4],
        }
    }

    fn inputs(frame: u64) -> LabelInputs {
        let values = SceneValues {
            frame,
            ..SceneValues::default()
        };
        LabelInputs::new(1, &values, &ViewConfiguration::default(), (800, 600))
    }

    /// The same inputs on a still picture reuse the quads without calling the builder.
    #[test]
    fn unchanged_inputs_reuse_the_last_labels() {
        let mut cache = LabelCache::default();
        let mut overlay = viz_render::Overlay::default();
        cache.append(&mut overlay, inputs(1), false, |overlay| {
            overlay.quads.push(quad(1.0))
        });
        overlay.clear();
        cache.append(&mut overlay, inputs(1), false, |_| {
            panic!("rebuilt for nothing")
        });
        assert_eq!(overlay.quads.len(), 1);
        assert_eq!(overlay.quads[0].rect[0], 1.0);
    }

    /// A new value frame, or a picture moving on its own, builds again.
    #[test]
    fn changed_inputs_or_motion_rebuild() {
        let mut cache = LabelCache::default();
        let mut overlay = viz_render::Overlay::default();
        cache.append(&mut overlay, inputs(1), false, |overlay| {
            overlay.quads.push(quad(1.0))
        });
        overlay.clear();
        cache.append(&mut overlay, inputs(2), false, |overlay| {
            overlay.quads.push(quad(2.0))
        });
        assert_eq!(overlay.quads[0].rect[0], 2.0);
        overlay.clear();
        cache.append(&mut overlay, inputs(2), true, |overlay| {
            overlay.quads.push(quad(3.0))
        });
        assert_eq!(overlay.quads[0].rect[0], 3.0);
    }
}
