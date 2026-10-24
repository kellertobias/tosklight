//! One device and presentation owner, with independently initialized render backends.
use super::draft::DraftRenderer;
use super::lines::LinesRenderer;
use super::{
    BASE_EXPOSURE, CapturedImage, FrameContext, FrameStats, OverlayRenderer, RenderError,
    ShadedRenderer,
};
use crate::gpu::{Gpu, PresentationSurface};
use crate::timing::{GpuTimer, TimingContext};
use viz_scene::{RenderQuality, Scene, SceneValues, ViewConfiguration, ViewMode};

pub struct Renderer {
    pub(super) gpu: Gpu,
    shaded: Option<Box<ShadedRenderer>>,
    draft: Option<Box<DraftRenderer>>,
    lines: Option<LinesRenderer>,
    overlay: OverlayRenderer,
    media_atlas: Option<crate::media::MediaAtlas>,
    media_content_enabled: bool,
    crowd_amount: f32,
    stats: FrameStats,
    pub(super) capture_request: Option<wgpu::TextureView>,
    timer: Option<GpuTimer>,
    timing_context: Option<TimingContext>,
    last_timing_sample: u64,
    active_samples: u32,
    active_mode: ViewMode,
    active_draft: bool,
    capture_quality: bool,
    pending_frame_interval: Option<u64>,
}

impl Renderer {
    pub fn new(target: &dyn PresentationSurface) -> Result<Self, String> {
        Self::with_icon(target, None)
    }

    pub fn headless(width: u32, height: u32) -> Result<Self, String> {
        Ok(Self::build(Gpu::headless(width, height)?, None))
    }

    /// A separate full-budget still renderer; it never adopts live adaptive reductions.
    pub fn headless_still() -> Result<Self, String> {
        let mut renderer = Self::headless(3840, 2160)?;
        renderer.capture_quality = true;
        Ok(renderer)
    }

    pub fn frozen_media(&self) -> crate::media::FrozenMedia {
        self.media_atlas
            .as_ref()
            .map_or_else(Default::default, |atlas| atlas.snapshot())
    }

    /// Restore invocation-time pixels into this fresh still renderer only.
    pub fn restore_frozen_media(
        &mut self,
        media: &crate::media::FrozenMedia,
    ) -> Result<(), String> {
        if !self.capture_quality || self.media_atlas.is_some() {
            return Err("frozen media requires a fresh still renderer".into());
        }
        if !media.sources().is_empty() {
            let mut atlas = crate::media::MediaAtlas::new(&self.gpu.device, &self.gpu.queue);
            media.restore_into(&mut atlas, &self.gpu.queue)?;
            self.media_atlas = Some(atlas);
        }
        Ok(())
    }

    pub fn with_icon(
        target: &dyn PresentationSurface,
        icon: Option<&[u8]>,
    ) -> Result<Self, String> {
        Ok(Self::build(Gpu::new(target)?, icon))
    }

    fn build(gpu: Gpu, icon: Option<&[u8]>) -> Self {
        let timer = GpuTimer::new(&gpu.device, &gpu.queue, gpu.timestamps);
        let overlay = OverlayRenderer::new(&gpu.device, &gpu.queue, gpu.format, icon);
        Self {
            active_samples: 1,
            gpu,
            shaded: None,
            draft: None,
            lines: None,
            overlay,
            media_atlas: None,
            media_content_enabled: true,
            crowd_amount: 1.0,
            stats: FrameStats::default(),
            capture_request: None,
            timer,
            timing_context: None,
            last_timing_sample: 0,
            active_mode: ViewMode::Full3d,
            active_draft: false,
            capture_quality: false,
            pending_frame_interval: None,
        }
    }

    pub fn adapter_name(&self) -> &str {
        &self.gpu.adapter_name
    }
    pub fn backend(&self) -> &str {
        &self.gpu.backend
    }
    /// Effective sample count of the last selected backend. Lines always uses one sample.
    pub fn samples(&self) -> u32 {
        self.active_samples
    }
    pub fn stats(&self) -> FrameStats {
        self.stats
    }
    pub fn device(&self) -> &wgpu::Device {
        &self.gpu.device
    }
    pub fn queue(&self) -> &wgpu::Queue {
        &self.gpu.queue
    }
    pub fn output_format(&self) -> wgpu::TextureFormat {
        self.gpu.format
    }
    pub fn exposure(&self) -> f32 {
        BASE_EXPOSURE
    }

    #[cfg(test)]
    pub(in crate::renderer) fn force_draft_legacy_composite_for_test(&mut self, legacy: bool) {
        self.draft
            .as_mut()
            .expect("render Draft first")
            .scene
            .draft
            .as_mut()
            .unwrap()
            .force_legacy_for_test(legacy);
    }
    #[cfg(test)]
    pub(in crate::renderer) fn draft_composite_state_for_test(&self) -> (bool, u32) {
        let scene = &self.draft.as_ref().expect("render Draft first").scene;
        (
            scene.draft.as_ref().unwrap().last_fused_composite,
            scene.laser_instances.length,
        )
    }
    #[cfg(test)]
    pub(in crate::renderer) fn force_draft_cone_cull_for_test(&mut self, enabled: bool) {
        let scene = &mut self.draft.as_mut().expect("render Draft first").scene;
        let bindings = scene.cull_pipeline.get_bind_group_layout(0);
        scene.cull_pipeline =
            super::pipelines::test_cone_cull_pipeline(&self.gpu.device, &bindings, enabled);
    }
    pub fn crowd_amount(&self) -> f32 {
        self.crowd_amount
    }
    pub fn set_crowd_amount(&mut self, amount: f32) {
        self.crowd_amount = amount.clamp(0.0, 1.0);
    }
    pub fn set_media_content_enabled(&mut self, enabled: bool) {
        self.media_content_enabled = enabled;
    }

    pub fn update_media_frame(&mut self, frame: &crate::media::MediaFrame) -> Result<bool, String> {
        self.media_atlas
            .get_or_insert_with(|| crate::media::MediaAtlas::new(&self.gpu.device, &self.gpu.queue))
            .update(&self.gpu.queue, frame)
    }
    pub fn media_upload_count(&self) -> u64 {
        self.media_atlas
            .as_ref()
            .map_or(0, crate::media::MediaAtlas::upload_count)
    }
    pub fn media_presentation_identity(&self) -> u64 {
        self.media_atlas
            .as_ref()
            .map_or(0, crate::media::MediaAtlas::presentation_identity)
    }

    /// Only this owner configures the swapchain. Backend targets follow lazily on their next use.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.gpu.resize(width, height);
    }
    pub fn reconfigure(&mut self) {
        self.gpu.reconfigure();
    }

    pub fn observe_frame_interval(&mut self, quality: RenderQuality, interval_micros: u64) {
        if quality == RenderQuality::Ultra
            && self.last_timing_sample == 0
            && self.active_mode != ViewMode::Lines3d
        {
            if let Some(shaded) = self.shaded.as_mut() {
                shaded.observe_gpu_micros(interval_micros);
            } else {
                self.pending_frame_interval = Some(interval_micros);
            }
        }
    }

    pub fn render_into(
        &mut self,
        target: &wgpu::TextureView,
        scene: &Scene,
        values: &SceneValues,
        view: &ViewConfiguration,
        overlay: &crate::overlay::Overlay,
        time_seconds: f32,
    ) -> Result<FrameStats, RenderError> {
        self.capture_request = Some(target.clone());
        self.render(scene, values, view, overlay, time_seconds)
    }

    pub fn render(
        &mut self,
        scene: &Scene,
        values: &SceneValues,
        view: &ViewConfiguration,
        overlay: &crate::overlay::Overlay,
        time_seconds: f32,
    ) -> Result<FrameStats, RenderError> {
        let capture_view = self
            .capture_quality
            .then(|| crate::still::capture_view(view));
        let view = capture_view.as_ref().unwrap_or(view);
        self.active_draft = view.mode.simulates_light() && view.quality == RenderQuality::Draft;
        let started = std::time::Instant::now();
        let _ = self.gpu.device.poll(wgpu::PollType::Poll);
        if let Some(timer) = self.timer.as_mut() {
            timer.collect();
        }
        let (frame, output, acquire_micros) = self.acquire_output()?;
        let context = FrameContext {
            device: &self.gpu.device,
            queue: &self.gpu.queue,
            size: (self.gpu.config.width, self.gpu.config.height),
            format: self.gpu.format,
            output: &output,
            timer: self.timer.as_ref(),
        };
        let mut encoder = context
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("viz frame"),
            });
        let (mut stats, render_size, assets_changed) = if view.mode == ViewMode::Lines3d {
            let lines = self
                .lines
                .get_or_insert_with(|| LinesRenderer::new(&context));
            let stats = lines.encode(
                &context,
                &mut encoder,
                scene,
                values,
                view,
                time_seconds,
                self.crowd_amount,
            )?;
            self.active_samples = 1;
            (stats, context.size, lines.assets_changed())
        } else {
            let media = self.media_atlas.get_or_insert_with(|| {
                crate::media::MediaAtlas::new(context.device, context.queue)
            });
            let is_draft = self.active_draft;
            let shaded = if is_draft {
                &mut self
                    .draft
                    .get_or_insert_with(|| Box::new(DraftRenderer::new(&context, media)))
                    .scene
            } else {
                self.shaded
                    .get_or_insert_with(|| {
                        Box::new(ShadedRenderer::new(&context, self.gpu.samples, media))
                    })
                    .as_mut()
            };
            if let Some(interval) = self.pending_frame_interval.take() {
                shaded.observe_gpu_micros(interval);
            }
            shaded.capture_quality = self.capture_quality;
            shaded.set_preferences(self.crowd_amount, self.media_content_enabled);
            let stats = shaded.encode(
                &context,
                &mut encoder,
                scene,
                values,
                view,
                media,
                time_seconds,
            );
            self.active_samples = if is_draft { 1 } else { self.gpu.samples };
            (stats, shaded.render_size(), shaded.assets_changed())
        };
        if !overlay.is_empty() {
            self.overlay.encode(
                &mut encoder,
                context.device,
                context.queue,
                context.size,
                overlay,
                context.output,
                context.timer,
            );
        }
        self.active_mode = view.mode;
        let timing_context = TimingContext {
            scene_revision: scene.revision,
            show_id: scene.show_id.map(|id| id.as_u128()),
            mode: view.mode,
            quality: view.quality,
            size: context.size,
            render_size,
            generation: 0,
        }
        .following(invalidate_asset_scope(self.timing_context, assets_changed));
        self.timing_context = Some(timing_context);
        let gpu_passes = self
            .timer
            .as_ref()
            .map(GpuTimer::timings)
            .unwrap_or_default();
        stats.gpu_micros = gpu_passes.total_micros();
        stats.gpu_passes = gpu_passes;
        stats.gpu_sample_id = self
            .timer
            .as_ref()
            .filter(|timer| timer.timings().total_micros().is_some())
            .map(GpuTimer::sample_id);
        stats.gpu_sample_matches_frame = self
            .timer
            .as_ref()
            .is_some_and(|timer| timer.sample_matches(timing_context));
        if let Some(id) = stats.gpu_sample_id
            && id != self.last_timing_sample
        {
            self.last_timing_sample = id;
            if view.mode.simulates_light()
                && view.quality == RenderQuality::Ultra
                && stats.gpu_sample_matches_frame
                && let Some(micros) = stats.gpu_micros
                && let Some(shaded) = self.shaded.as_mut()
            {
                shaded.observe_gpu_micros(micros);
            }
        }
        if let Some(timer) = self.timer.as_mut() {
            timer.resolve(&mut encoder, timing_context);
        }
        self.gpu.queue.submit(Some(encoder.finish()));
        if let Some(timer) = self.timer.as_mut() {
            timer.request_readback();
        }
        if let Some(frame) = frame {
            self.gpu.queue.present(frame);
        }
        stats.cpu_micros = started.elapsed().as_micros() as u64;
        stats.acquire_micros = acquire_micros;
        self.stats = stats;
        Ok(stats)
    }

    pub fn capture(
        &mut self,
        scene: &Scene,
        values: &SceneValues,
        view: &ViewConfiguration,
        overlay: &crate::overlay::Overlay,
        time_seconds: f32,
    ) -> Result<CapturedImage, RenderError> {
        let mut pending = self.begin_capture(scene, values, view, overlay, time_seconds)?;
        self.gpu
            .device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .map_err(|error| RenderError::Surface(format!("capture polling: {error}")))?;
        pending.try_complete()?.ok_or_else(|| {
            RenderError::Surface("capture mapping did not complete after GPU wait".to_owned())
        })
    }

    fn acquire_output(
        &mut self,
    ) -> Result<(Option<wgpu::SurfaceTexture>, wgpu::TextureView, u64), RenderError> {
        let before_acquire = std::time::Instant::now();
        let capture = self.capture_request.take();
        let result = if let Some(output) = capture {
            (None, output, 0)
        } else {
            let frame = self.acquire()?;
            let acquire_micros = before_acquire.elapsed().as_micros() as u64;
            let output = frame
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            (Some(frame), output, acquire_micros)
        };
        Ok(result)
    }

    fn acquire(&mut self) -> Result<wgpu::SurfaceTexture, RenderError> {
        let Some(surface) = self.gpu.surface.as_ref() else {
            return Err(RenderError::Surface(
                "this renderer is headless and has no window to present to; capture instead"
                    .to_owned(),
            ));
        };
        match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture)
            | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => Ok(texture),
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                Err(RenderError::SkipFrame)
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.gpu.reconfigure();
                match surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(texture)
                    | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => Ok(texture),
                    other => Err(RenderError::Surface(format!("{other:?}"))),
                }
            }
            other => Err(RenderError::Surface(format!("{other:?}"))),
        }
    }

    #[cfg(test)]
    pub(super) fn initialized_resources(&self) -> (bool, bool, bool) {
        (
            self.shaded.is_some() || self.draft.is_some(),
            self.lines.is_some(),
            self.media_atlas.is_some(),
        )
    }

    #[cfg(test)]
    pub(super) fn shaded_frame(&self) -> &crate::instances::FrameInstances {
        if self.active_draft {
            &self
                .draft
                .as_ref()
                .expect("test rendered Draft")
                .scene
                .frame
        } else {
            &self
                .shaded
                .as_ref()
                .expect("test rendered a shaded frame")
                .frame
        }
    }
}

/// Geometry may be replaced without a scene revision changing. Never let that reuse an old
/// timestamp generation, including when a backend is revisited after another mode was drawn.
fn invalidate_asset_scope(previous: Option<TimingContext>, changed: bool) -> Option<TimingContext> {
    previous.map(|mut scope| {
        if changed {
            scope.generation = scope.generation.saturating_add(1);
        }
        scope
    })
}

#[cfg(test)]
mod scope_tests {
    use super::*;

    #[test]
    fn geometry_replacement_at_same_revision_invalidates_old_timing_without_reusing_generation() {
        let scope = TimingContext {
            scene_revision: 1,
            show_id: Some(1),
            mode: ViewMode::Full3d,
            quality: RenderQuality::High,
            size: (1920, 1080),
            render_size: (1920, 1080),
            generation: 0,
        };
        let first = scope.following(invalidate_asset_scope(None, true));
        assert_eq!(first.generation, 1);
        assert_eq!(
            scope.following(invalidate_asset_scope(Some(first), false)),
            first
        );
        let replaced = scope.following(invalidate_asset_scope(Some(first), true));
        assert_eq!(replaced.generation, first.generation + 1);
        assert_ne!(replaced, first);
        assert_eq!(
            scope.following(invalidate_asset_scope(Some(replaced), false)),
            replaced
        );
        let replaced_again = scope.following(invalidate_asset_scope(Some(replaced), true));
        assert_eq!(replaced_again.generation, replaced.generation + 1);
    }
}
