//! Immutable invocation-time inputs and a separate full-budget native 4K renderer.
//! Callers must supply the final resolved motion/physics/effect values and effective local
//! camera, before the native image worker starts. This core never advances them.
use crate::{CapturedImage, PendingCapture, RenderError, Renderer, media::FrozenMedia};
use viz_scene::{RenderQuality, Scene, SceneValues, ViewConfiguration, ViewMode};

pub const STILL_WIDTH: u32 = 3840;
pub const STILL_HEIGHT: u32 = 2160;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StillOrigin {
    pub pane_id: String,
    pub follow_preload: bool,
}

#[derive(Clone)]
pub struct FrozenStill {
    scene: Scene,
    values: SceneValues,
    view: ViewConfiguration,
    time_seconds: f32,
    crowd_amount: f32,
    media: FrozenMedia,
    origin: StillOrigin,
}
impl FrozenStill {
    pub fn new(
        scene: &Scene,
        values: &SceneValues,
        view: &ViewConfiguration,
        time_seconds: f32,
        media: FrozenMedia,
        origin: StillOrigin,
    ) -> Result<Self, String> {
        Self::new_with_crowd_amount(scene, values, view, time_seconds, media, origin, 1.0)
    }
    /// Freeze the invocation-time local crowd preference before any engine chooser.
    pub fn new_with_crowd_amount(
        scene: &Scene,
        values: &SceneValues,
        view: &ViewConfiguration,
        time_seconds: f32,
        media: FrozenMedia,
        origin: StillOrigin,
        crowd_amount: f32,
    ) -> Result<Self, String> {
        if !crowd_amount.is_finite() || !(0.0..=1.0).contains(&crowd_amount) {
            return Err("still capture requires a finite crowd amount between zero and one".into());
        }
        if !time_seconds.is_finite() || time_seconds < 0.0 {
            return Err("still capture requires a finite resolved scene time".into());
        }
        let media = media.with_scene_fallbacks(scene)?;
        Ok(Self {
            scene: scene.clone(),
            values: values.clone(),
            view: view.clone(),
            time_seconds,
            crowd_amount,
            media,
            origin,
        })
    }
    pub fn crowd_amount(&self) -> f32 {
        self.crowd_amount
    }
    pub fn scene(&self) -> &Scene {
        &self.scene
    }
    pub fn values(&self) -> &SceneValues {
        &self.values
    }
    pub fn view(&self) -> &ViewConfiguration {
        &self.view
    }
    pub fn media(&self) -> &FrozenMedia {
        &self.media
    }
    pub fn origin(&self) -> &StillOrigin {
        &self.origin
    }
    /// Unavailable here is not evidence that a media server itself is black/offline. Keep this
    /// distinction visible until the embedded source pipe supplies invocation-time frames.
    pub fn media_notice(&self) -> Option<String> {
        let ids: std::collections::BTreeSet<_> = self
            .scene
            .media_sections
            .iter()
            .filter_map(|section| section.source_id)
            .collect();
        let missing: Vec<_> = ids
            .into_iter()
            .filter(|id| {
                !self
                    .media
                    .sources()
                    .iter()
                    .any(|source| source.frame.source_id == *id && source.available)
            })
            .map(|id| {
                self.scene
                    .media_sources
                    .iter()
                    .find(|source| source.id == id)
                    .map_or_else(
                        || id.to_string(),
                        |source| source.name.chars().take(128).collect(),
                    )
            })
            .collect();
        (!missing.is_empty()).then(||format!("Media frames unavailable to this renderer at capture: {}. The image uses the frozen show fallback or black; no later server frame is substituted.",missing.join(", ")))
    }

    pub fn time_seconds(&self) -> f32 {
        self.time_seconds
    }

    /// Run on the job worker: device creation and submission must never block the UI thread.
    pub fn begin_native(&self) -> Result<NativeStill, String> {
        let mut renderer = Renderer::headless_still()?;
        renderer.set_crowd_amount(self.crowd_amount);
        renderer.restore_frozen_media(&self.media)?;
        let pending = renderer
            .begin_capture(
                &self.scene,
                &self.values,
                &self.view,
                &crate::Overlay::default(),
                self.time_seconds,
            )
            .map_err(|e| e.to_string())?;
        Ok(NativeStill {
            renderer,
            pending,
            completed: false,
        })
    }
}

/// Owns the offscreen device until readback completes. Poll on a worker, not the live renderer.
pub struct NativeStill {
    renderer: Renderer,
    pending: PendingCapture,
    completed: bool,
}
impl NativeStill {
    pub fn poll(&mut self) -> Result<Option<CapturedImage>, RenderError> {
        if self.completed {
            return Err(RenderError::Surface(
                "still readback already completed".into(),
            ));
        }
        self.renderer
            .device()
            .poll(wgpu::PollType::Poll)
            .map_err(|error| RenderError::Surface(format!("still polling: {error}")))?;
        let image = self.pending.try_complete()?;
        self.completed = image.is_some();
        Ok(image)
    }
    pub fn stats(&self) -> crate::FrameStats {
        self.renderer.stats()
    }
}

pub(crate) fn capture_view(view: &ViewConfiguration) -> ViewConfiguration {
    let mut capture = view.clone();
    capture.quality = RenderQuality::Ultra;
    if matches!(capture.mode, ViewMode::Lines3d | ViewMode::Simple3d) {
        capture.mode = ViewMode::Full3d;
    }
    capture
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frozen_inputs_and_hq_policy_leave_live_view_and_lane_untouched() {
        let mut scene = Scene::default();
        let mut values = SceneValues::default();
        let mut view = ViewConfiguration::default();
        view.mode = ViewMode::Lines3d;
        view.quality = RenderQuality::Draft;
        view.detailed_people = true;
        let origin = StillOrigin {
            pane_id: "preload-pane".into(),
            follow_preload: true,
        };
        let frozen = FrozenStill::new(
            &scene,
            &values,
            &view,
            12.5,
            FrozenMedia::default(),
            origin.clone(),
        )
        .unwrap();
        scene.revision += 1;
        values.frame += 1;
        view.camera.position.x += 10.0;
        view.detailed_people = false;
        let hq = capture_view(frozen.view());
        assert_eq!(hq.mode, ViewMode::Full3d);
        assert_eq!(hq.quality, RenderQuality::Ultra);
        assert!(
            frozen.view().detailed_people && hq.detailed_people,
            "raw Draft detail freezes before chooser and survives HQ upgrade"
        );
        assert!(
            !view.detailed_people,
            "later local preference changes cannot mutate the frozen job"
        );
        assert_eq!(view.mode, ViewMode::Lines3d);
        assert_eq!(view.quality, RenderQuality::Draft);
        assert_ne!(frozen.view().camera.position, view.camera.position);
        assert_ne!(frozen.scene().revision, scene.revision);
        assert_ne!(frozen.values().frame, values.frame);
        assert_eq!(frozen.origin(), &origin);
        assert_eq!(frozen.time_seconds(), 12.5);
    }
    #[test]
    fn crowd_preference_is_frozen_and_invalid_amounts_are_rejected() {
        let scene = Scene::default();
        let values = SceneValues::default();
        let view = ViewConfiguration::default();
        let origin = StillOrigin {
            pane_id: "live".into(),
            follow_preload: false,
        };
        let mut live_amount = 0.25;
        let frozen = FrozenStill::new_with_crowd_amount(
            &scene,
            &values,
            &view,
            1.0,
            FrozenMedia::default(),
            origin.clone(),
            live_amount,
        )
        .unwrap();
        live_amount = 1.0;
        assert_eq!(frozen.crowd_amount(), 0.25);
        assert_ne!(frozen.crowd_amount(), live_amount);
        for amount in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
            assert!(
                FrozenStill::new_with_crowd_amount(
                    &scene,
                    &values,
                    &view,
                    1.0,
                    FrozenMedia::default(),
                    origin.clone(),
                    amount,
                )
                .is_err()
            );
        }
        for amount in [0.0, 0.5, 1.0] {
            assert_eq!(
                FrozenStill::new_with_crowd_amount(
                    &scene,
                    &values,
                    &view,
                    1.0,
                    FrozenMedia::default(),
                    origin.clone(),
                    amount,
                )
                .unwrap()
                .crowd_amount(),
                amount
            );
        }
        assert_eq!(
            FrozenStill::new(&scene, &values, &view, 1.0, FrozenMedia::default(), origin,)
                .unwrap()
                .crowd_amount(),
            1.0
        );
    }
    #[test]
    fn still_preserves_orthographic_mode_and_rejects_invalid_time() {
        let mut view = ViewConfiguration::default();
        view.mode = ViewMode::TopDown;
        assert_eq!(capture_view(&view).mode, ViewMode::TopDown);
        assert!(
            FrozenStill::new(
                &Scene::default(),
                &SceneValues::default(),
                &view,
                f32::NAN,
                FrozenMedia::default(),
                StillOrigin {
                    pane_id: "live".into(),
                    follow_preload: false
                }
            )
            .is_err()
        );
    }
}

#[cfg(test)]
mod gpu_tests {
    use super::*;
    use glam::Vec3;
    use std::time::{Duration, Instant};
    #[test]
    #[ignore = "requires a real GPU; run explicitly with --ignored --test-threads=1"]
    fn real_model_still_is_4k_full_budget_and_does_not_change_live_lines() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../assets/models/lamps/par-64-long-nose-silver.glb");
        let model = viz_scene::read_glb(&std::fs::read(path).unwrap()).unwrap();
        let mut scene = Scene::default();
        scene.models.push(model);
        scene.fixtures.push(viz_scene::FixtureInstance {
            lamp_response: Default::default(),
            model: Some(0),
            body: viz_scene::FixtureBody {
                kind: viz_scene::BodyKind::Lantern,
                size: Vec3::splat(1.0),
            },
            ..Default::default()
        });
        scene.recompute_bounds();
        let values = SceneValues::default();
        let mut view = ViewConfiguration::default();
        view.mode = ViewMode::Lines3d;
        view.quality = RenderQuality::Draft;
        view.floor_grid = false;
        view.show_labels = false;
        view.ambient = 1.0;
        view.background = Some([0.0; 3]);
        view.camera.position = Vec3::new(1.5, 1.0, 2.2);
        view.camera.target = Vec3::ZERO;
        let mut live = Renderer::headless(128, 128).expect("GPU required for live renderer");
        let before = live
            .capture(&scene, &values, &view, &crate::Overlay::default(), 2.5)
            .unwrap();
        let frozen = FrozenStill::new(
            &scene,
            &values,
            &view,
            2.5,
            live.frozen_media(),
            StillOrigin {
                pane_id: "preload".into(),
                follow_preload: true,
            },
        )
        .unwrap();
        let mut job = frozen.begin_native().expect("separate native 4K renderer");
        let deadline = Instant::now() + Duration::from_secs(30);
        let image = loop {
            if let Some(image) = job.poll().unwrap() {
                break image;
            }
            assert!(Instant::now() < deadline, "native still readback timed out");
            std::thread::sleep(Duration::from_millis(1));
        };
        assert_eq!((image.width, image.height), (STILL_WIDTH, STILL_HEIGHT));
        assert_eq!(image.rgba.len(), (STILL_WIDTH * STILL_HEIGHT * 4) as usize);
        assert!(
            image
                .rgba
                .chunks_exact(4)
                .filter(|p| p[0] > 20 || p[1] > 20 || p[2] > 20)
                .count()
                > 1000,
            "real modeled housing must occupy visible shaded pixels"
        );
        let stats = job.stats();
        assert_eq!(stats.render_scale, 1.0);
        assert_eq!(stats.volumetric_steps, 64);
        assert_eq!(stats.shadow_budget, 16);
        assert!(!stats.degraded && !stats.quality_reduction.is_active());
        for _ in 0..12 {
            job.renderer
                .observe_frame_interval(RenderQuality::Ultra, 1_000_000);
        }
        let still_again = job
            .renderer
            .capture(&scene, &values, &view, &crate::Overlay::default(), 2.5)
            .unwrap();
        assert_eq!(
            image.rgba, still_again.rgba,
            "adaptive observations cannot degrade a still renderer"
        );
        let after = live
            .capture(&scene, &values, &view, &crate::Overlay::default(), 2.5)
            .unwrap();
        assert_eq!(
            before.rgba, after.rgba,
            "capture leaves live Lines pixels and quality untouched"
        );
        assert_eq!(live.samples(), 1);
        assert_eq!(view.quality, RenderQuality::Draft);
        assert_eq!(view.mode, ViewMode::Lines3d);
    }
}
