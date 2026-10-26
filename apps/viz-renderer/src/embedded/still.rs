//! Capture requests are consumed only after the helper's ordered barrier has been applied.
use super::{HelperSource, PaneState};
use std::{sync::Arc, time::Instant};
use viz_helper::{
    protocol::ToHelper,
    still::{Phase, Status, StillJobId},
};
use viz_render::still::{FrozenStill, STILL_HEIGHT, STILL_WIDTH, StillOrigin};

pub(super) fn service(state: &mut PaneState, source: &mut HelperSource, epoch: Instant) {
    if let Some(request) = source.take_still_request() {
        match request {
            ToHelper::CaptureStart {
                request_id,
                pane_id,
            } => {
                if request_id.is_empty()
                    || request_id.len() > 128
                    || pane_id.is_empty()
                    || pane_id.len() > 128
                {
                    source.report("Invalid capture request identity");
                    return;
                }
                let time = epoch.elapsed().as_secs_f32();
                state
                    .mechanics
                    .advance(&state.scene, &mut state.values, Instant::now(), true);
                // Light Desk owns no live media decoders. Legacy internal requests freeze only
                // scene-provided pixels; FrozenStill reports missing live media explicitly.
                let result = FrozenStill::new_with_crowd_amount(
                    &state.scene,
                    &state.values,
                    &state.view,
                    time,
                    viz_render::media::FrozenMedia::default(),
                    StillOrigin {
                        pane_id: pane_id.clone(),
                        follow_preload: state.following_preload,
                    },
                    state.renderer.crowd_amount(),
                )
                .and_then(|frozen| {
                    state.stills.start(
                        Arc::new(frozen),
                        crate::stills::StillOptions {
                            request_id: Some(request_id.clone()),
                            configured_blender: state.still_blender.clone(),
                            ..Default::default()
                        },
                    )
                });
                if let Err(detail) = result {
                    source.send_still_status(Status {
                        job_id: StillJobId(request_id.clone()),
                        request_id: Some(request_id),
                        pane_id,
                        follow_preload: state.following_preload,
                        scene_revision: state.scene.revision,
                        values_frame: state.values.frame,
                        time_seconds: time,
                        width: STILL_WIDTH,
                        height: STILL_HEIGHT,
                        framing: "16:9 camera framing".into(),
                        engine: None,
                        notice: None,
                        phase: Phase::Failed { detail },
                    });
                }
            }
            ToHelper::CaptureChoose { job_id, engine } => {
                if let Err(detail) = state.stills.choose(&job_id, engine) {
                    source.report(&detail);
                }
            }
            ToHelper::CaptureCancel { job_id } => {
                if let Err(detail) = state.stills.cancel(&job_id) {
                    source.report(&detail);
                }
            }
            _ => {}
        }
    }
    if let Some(status) = state.stills.status() {
        if state.last_still_status.as_ref() != Some(&status) {
            source.send_still_status(status.clone());
            state.last_still_status = Some(status);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use viz_scene::{MediaCrop, MediaSection, MediaSectionKind, MediaSourceBinding};

    #[test]
    fn internal_freeze_retains_scene_fallback_only_and_reports_missing_live_media() {
        let fallback = uuid::Uuid::new_v4();
        let live = uuid::Uuid::new_v4();
        let mut scene = viz_scene::Scene::default();
        for (id, name, pixels) in [
            (fallback, "Fallback", Some(vec![42; 512 * 512 * 4])),
            (live, "Live server", None),
        ] {
            scene.media_sources.push(MediaSourceBinding {
                id,
                name: name.into(),
                server_id: uuid::Uuid::nil(),
                host: "unreachable.invalid".into(),
                port: 4809,
                advertised_source_id: 257,
                aspect_ratio: None,
                fallback_rgba: pixels,
            });
        }
        scene.media_sections.push(MediaSection {
            id: uuid::Uuid::new_v4(),
            surface_id: uuid::Uuid::new_v4(),
            name: "Panel".into(),
            source_id: Some(live),
            fallback_source_id: Some(fallback),
            position: glam::Vec3::ZERO,
            rotation_degrees: glam::Vec3::ZERO,
            size: glam::Vec3::ONE,
            crop: MediaCrop {
                left: 0.0,
                top: 0.0,
                width: 1.0,
                height: 1.0,
            },
            kind: MediaSectionKind::Tv {
                bezel_metres: 0.01,
                spill: 0.0,
            },
        });
        let frozen = FrozenStill::new(
            &scene,
            &viz_scene::SceneValues::default(),
            &viz_scene::ViewConfiguration::default(),
            1.0,
            viz_render::media::FrozenMedia::default(),
            StillOrigin {
                pane_id: "internal".into(),
                follow_preload: false,
            },
        )
        .unwrap();
        let sources = frozen.media().sources();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].frame.source_id, fallback);
        assert!(sources[0].available && sources[0].frame.persistent);
        assert!(frozen.media_notice().unwrap().contains("Live server"));
        scene.media_sources[0].fallback_rgba.as_mut().unwrap()[0] = 99;
        assert_eq!(
            sources[0].frame.rgba[0], 42,
            "frozen pixels cannot change later"
        );
    }
}
