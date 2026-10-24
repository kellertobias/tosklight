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
                let result = FrozenStill::new(
                    &state.scene,
                    &state.values,
                    &state.view,
                    time,
                    state.renderer.frozen_media(),
                    StillOrigin {
                        pane_id: pane_id.clone(),
                        follow_preload: state.following_preload,
                    },
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
