use super::{Embedding, HelperSource, PaneState};

pub(super) fn prepare(
    state: &mut PaneState,
    source: &mut HelperSource,
    embedding: &Embedding,
) -> Result<(), String> {
    // Publish the framed camera after picture settings resolve, including before any gesture.
    if let Some(camera) = state.take_camera_update() {
        source.send_camera(&camera);
    }
    state.resize_for(embedding)
}

/// Adopt complete picture settings without changing the source connection or CPU clock.
pub(super) fn apply(
    state: &mut PaneState,
    source: &HelperSource,
    rig: &mut Option<Box<viz_desk::DeskProvider>>,
) {
    // The picture settings are the renderer's own, and the desk sends them rather than
    // applying them: it is not the one drawing this.
    if let Some(viz_helper::protocol::ToHelper::Picture {
        atmosphere,
        ambient,
        quality,
        exposure,
        laser_brightness,
        lamp_fog_cloudiness,
        lamp_fog_turbulence,
        laser_fog_cloudiness,
        laser_fog_turbulence,
        show_labels,
        detailed_people,
        floor_grid,
        show_beam_guides,
        background,
        mode,
        follow_preload,
    }) = source.picture()
    {
        state.values.atmosphere = viz_scene::AtmospherePreference {
            amount: atmosphere.clamp(0.0, 1.0),
        }
        .resolve();
        state.view.ambient = ambient.clamp(0.0, 2.0);
        state.view.exposure = exposure.clamp(0.05, 4.0);
        state.view.laser_brightness = laser_brightness.clamp(0.0, 4.0);
        state.view.fog_variation = viz_scene::FogVariation {
            lamp_cloudiness: lamp_fog_cloudiness.clamp(0.0, 1.0),
            lamp_turbulence: lamp_fog_turbulence.clamp(0.0, 1.0),
            laser_cloudiness: laser_fog_cloudiness.clamp(0.0, 1.0),
            laser_turbulence: laser_fog_turbulence.clamp(0.0, 1.0),
        };
        state.view.show_labels = *show_labels;
        state.view.detailed_people = *detailed_people;
        state.view.floor_grid = *floor_grid;
        state.view.show_aim_guides = *show_beam_guides;
        state.view.background = Some(background.map(|channel| channel.clamp(0.0, 1.0)));
        // The provider is what lays the preload over the live picture, so the flag goes to it
        // rather than being kept here: the values are decoded there, and a second opinion about
        // whether to overlay would eventually disagree with the first.
        if state.following_preload != *follow_preload {
            state.following_preload = *follow_preload;
            if let Some(rig) = rig.as_mut() {
                rig.follow_preload(*follow_preload);
            }
        }
        let was = state.view.mode;
        state.view.mode = match mode {
            viz_helper::protocol::StageViewMode::TopDown => viz_scene::ViewMode::TopDown,
            viz_helper::protocol::StageViewMode::LeftToRight => viz_scene::ViewMode::LeftToRight,
            viz_helper::protocol::StageViewMode::RightToLeft => viz_scene::ViewMode::RightToLeft,
            viz_helper::protocol::StageViewMode::FrontToBack => viz_scene::ViewMode::FrontToBack,
            viz_helper::protocol::StageViewMode::BackToFront => viz_scene::ViewMode::BackToFront,
            viz_helper::protocol::StageViewMode::Lines3d => viz_scene::ViewMode::Lines3d,
            viz_helper::protocol::StageViewMode::Simple3d => viz_scene::ViewMode::Simple3d,
            viz_helper::protocol::StageViewMode::Full3d => viz_scene::ViewMode::Full3d,
        };
        /*
         * Choosing a view frames the rig in it. This is the whole of what "viewed from" means:
         * an operator asking for the plan from stage left is asking to stand at stage left, not
         * to have the same picture relabelled — and the camera they had was aimed for the view
         * they were looking at, which this no longer is.
         *
         * It overrides a camera the operator moved by hand, deliberately. That camera belonged
         * to the view they have just left. Within a view their own aim is theirs and nothing
         * takes it back.
         */
        if state.view.mode != was {
            state.view.camera = viz_scene::Camera::framed(state.view.mode, state.frame_bounds());
            state.camera_is_local = false;
        }
        state.view.quality = match quality {
            viz_helper::protocol::RenderQuality::Draft => viz_scene::RenderQuality::Draft,
            viz_helper::protocol::RenderQuality::Standard => viz_scene::RenderQuality::Standard,
            viz_helper::protocol::RenderQuality::High => viz_scene::RenderQuality::High,
            viz_helper::protocol::RenderQuality::Ultra => viz_scene::RenderQuality::Ultra,
        };
    }
}
