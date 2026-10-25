//! Drawing the renderer's pane into the desk's own window.
//!
//! The window is a native window with the interface added on top as a transparent child webview,
//! so there is a surface underneath everything the desk draws. This is what draws on it: one
//! picture, in one rectangle, with the whole rest of the surface left transparent so the interface
//! above is untouched.
//!
//! The scissor is the contract rather than a convention. The renderer is given a rectangle and this
//! draws inside it and nowhere else, so a menu opening across the pane, a dialog over it, or the
//! sheet beside it are drawn by the webview above and physically cannot be painted over.
//!
//! Where the picture comes from depends on what the two processes agreed. A shared surface is
//! sampled where it lies. Copied frames are uploaded into a texture of the desk's own. Either way
//! what reaches the draw below is a texture, and the draw does not care which it was.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Instant;
use viz_helper::pane::PaneRect;
use viz_helper::protocol::{FrameId, SHARED_FRAME_SLOTS};
mod measurement;
mod ownership;
mod render;
mod retirement;
mod schedule;
mod sequence;
mod sources;
mod suspension;
pub(crate) use measurement::CompositorMeasurements;
use measurement::{CompositorScope, Measurement};
use ownership::Exchange;
use retirement::Retirement;
pub(crate) use schedule::DrawOutcome;
use schedule::Schedule;
use sources::{Payload, Slot};
use suspension::Suspension;

/// The blit.
///
/// A fullscreen triangle rather than a quad: one primitive and no seam down the diagonal. The
/// viewport is what puts it in the pane, so the vertex positions never mention the rectangle —
/// but `@builtin(position)` is in the surface's coordinates rather than the viewport's, so the
/// fragment subtracts the pane's origin to find where in the picture it is.
const SHADER: &str = r"
struct Pane {
    origin: vec2<f32>,
    size: vec2<f32>,
}

@group(0) @binding(0) var picture: texture_2d<f32>;
@group(0) @binding(1) var picture_sampler: sampler;
@group(0) @binding(2) var<uniform> pane: Pane;

@vertex
fn vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let x = f32(i32(index) / 2) * 4.0 - 1.0;
    let y = f32(i32(index) & 1) * 4.0 - 1.0;
    return vec4<f32>(x, y, 0.0, 1.0);
}

@fragment
fn fragment(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = (position.xy - pane.origin) / pane.size;
    return textureSample(picture, picture_sampler, uv);
}
";

/// What the desk draws, and where.
pub struct StageCompositor {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    format: wgpu::TextureFormat,
    alpha_mode: wgpu::CompositeAlphaMode,
    /// Physical pixel size of the whole window surface.
    surface_size: (u32, u32),
    needs_configure: bool,
    detached: bool,
    retirement: Retirement,
    terminal_queue_done: Option<Arc<AtomicBool>>,
    suspension: Suspension,
    window_visible: bool,
    flight_is_hidden_clear: bool,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// Where the pane is, in the surface's pixels, as the shader reads it.
    uniform: wgpu::Buffer,
    pictures: Exchange<Payload>,
    slots: Vec<Slot>,
    waiting_copies: Vec<ownership::Picture<Payload>>,
    source_generation: u64,
    ready_sequence: sequence::ReadySequence,
    schedule: Schedule,
    clock: Instant,
    flight_done: Option<Arc<AtomicBool>>,
    source_error: Option<String>,
    pane: PaneRect,
    scale: f32,
    generation: u64,
    measurement: Option<Measurement>,
}

/// The picture to draw, however it arrived.
struct Source {
    /// Held for a shared surface so its memory outlives the texture over it.
    shared: Option<viz_surface::SharedSurface>,
    _texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    size: (u32, u32),
}

impl StageCompositor {
    /// Attach to the desk's window.
    ///
    /// `target` is the native window the interface sits on top of. Everything drawn here is beneath
    /// that interface, which is the arrangement the embedded pane depends on entirely.
    pub fn attach<T>(target: Arc<T>, width: u32, height: u32, scale: f32) -> Result<Self, String>
    where
        T: raw_window_handle::HasWindowHandle
            + raw_window_handle::HasDisplayHandle
            + Send
            + Sync
            + 'static,
    {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance
            .create_surface(target)
            .map_err(|error| format!("the desk's window has no drawable surface: {error}"))?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
            ..Default::default()
        }))
        .map_err(|error| format!("no adapter can draw on the desk's window: {error}"))?;
        let (device, queue, benchmark, timestamps) = measurement::open_device(&adapter)?;

        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(|format| format.is_srgb())
            .or_else(|| capabilities.formats.first().copied())
            .ok_or("the desk's window offers no surface format")?;
        // Everything outside the pane is left fully transparent, and it is the window's own
        // transparency that lets the interface show through there. A platform without an alpha
        // mode still draws the pane; the rest of the window is simply opaque, which is the desk's
        // background colour rather than a hole.
        let alpha_mode = if capabilities
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::PostMultiplied)
        {
            wgpu::CompositeAlphaMode::PostMultiplied
        } else if capabilities
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::PreMultiplied)
        {
            wgpu::CompositeAlphaMode::PreMultiplied
        } else {
            capabilities.alpha_modes[0]
        };

        let (pipeline, layout, sampler, uniform) = render::create_resources(&device, format);

        let measurement = benchmark
            .then(|| Measurement::new(&device, &queue, timestamps))
            .transpose()?;
        let mut compositor = Self {
            surface,
            device,
            queue,
            format,
            alpha_mode,
            surface_size: (width.max(1), height.max(1)),
            needs_configure: false,
            detached: false,
            retirement: Retirement::default(),
            terminal_queue_done: None,
            suspension: Suspension::default(),
            window_visible: true,
            flight_is_hidden_clear: false,
            pipeline,
            layout,
            sampler,
            uniform,
            pictures: Exchange::default(),
            slots: Vec::with_capacity(SHARED_FRAME_SLOTS),
            waiting_copies: Vec::with_capacity(1),
            source_generation: 0,
            ready_sequence: sequence::ReadySequence::default(),
            schedule: Schedule::default(),
            clock: Instant::now(),
            flight_done: None,
            source_error: None,
            pane: PaneRect::default(),
            scale: if scale > 0.0 { scale } else { 1.0 },
            generation: 0,
            measurement,
        };
        compositor.configure();
        // Behind the interface, which is the whole point of drawing here rather than in a window
        // of its own: a menu, a dialog or the pane's own settings must be able to open across the
        // Stage. Done after configuring, so the layer exists to be ordered.
        #[cfg(target_os = "macos")]
        viz_surface::send_surface_layer_to_back(&compositor.surface);
        if benchmark
            && std::env::var_os("LIGHT_STAGE_PACKAGED_BENCH_REPORT")
                .is_some_and(|path| std::path::Path::new(&path).is_absolute())
            && let Some(identity) = viz_surface::surface_layer_identity(&compositor.surface)
        {
            eprintln!(
                "[stage-layer-identity] pid={} surface={}x{} scale={} {}",
                std::process::id(),
                width,
                height,
                scale,
                identity
            );
        }
        Ok(compositor)
    }

    /// Where the desk's layout says the Stage pane is, in the points the web layout works in.
    pub fn set_pane(&mut self, pane: PaneRect, scale: f32) {
        if self.pane != pane || (scale > 0.0 && self.scale != scale) {
            self.generation = self.generation.saturating_add(1);
            self.schedule.dirty();
        }
        self.pane = pane;
        if scale > 0.0 {
            self.scale = scale;
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let size = (width.max(1), height.max(1));
        if size == self.surface_size {
            return;
        }
        self.surface_size = size;
        // Reconfiguration can wait for a drawable still in use. Defer it until our single
        // composition has completed instead of serializing the window event behind the GPU.
        self.needs_configure = true;
        self.schedule.dirty();
    }

    /// Forget whatever was being drawn, leaving the whole window to the interface again.
    pub fn clear_source(&mut self) {
        self.generation = self.generation.saturating_add(1);
        if let Err(error) = self.pictures.clear() {
            self.source_error = Some(error);
        }
        self.slots.clear();
        self.waiting_copies.clear();
        self.source_generation = 0;
        self.ready_sequence = sequence::ReadySequence::default();
        self.schedule.dirty();
    }

    pub fn set_visible(&mut self, visible: bool) {
        if self.detached {
            return;
        }
        if self.schedule.visible == visible {
            return;
        }
        match viz_surface::set_owned_surface_layer_visible(&self.surface, visible) {
            Ok(handled) if !visible => self.suspension.hide(handled),
            Ok(_) => self.suspension.reveal(),
            Err(error) => {
                self.source_error = Some(error);
            }
        }
        self.schedule.set_visible(visible);
        if !visible && let Err(error) = self.pictures.discard_ready() {
            self.source_error = Some(error);
        }
    }

    pub fn set_window_visible(&mut self, visible: bool) {
        self.window_visible = visible;
    }

    /// Remove only this compositor's native layer without submitting or waiting for GPU work.
    /// In-flight callbacks keep source backing alive even if the caller drops us immediately.
    /// `false` leaves normal state intact for asynchronous blank-frame retirement on platforms
    /// without an owned detachable layer.
    pub fn detach(&mut self) -> Result<bool, String> {
        if self.detached {
            return Ok(true);
        }
        let detached = viz_surface::detach_owned_surface_layer(&self.surface)
            .map_err(|error| error.to_string())?;
        if detached {
            self.detached = true;
            self.schedule.set_visible(false);
        }
        Ok(detached)
    }

    pub fn producer_visible(&self) -> bool {
        !self.retirement.closing()
            && !self.detached
            && self.window_visible
            && self.schedule.visible
            && !self.schedule.occluded
    }

    pub fn has_front_picture(&self) -> bool {
        !self.detached && self.pictures.front.is_some()
    }

    pub fn next_poll_delay(&self) -> std::time::Duration {
        if self.retirement.awaiting_queue() {
            return std::time::Duration::from_millis(2);
        }
        let mut schedule = self.schedule.clone();
        if self.suspension.pending && self.window_visible {
            schedule.visible = true;
        }
        schedule.next_poll_delay(self.clock.elapsed(), self.pictures.busy())
    }

    /// Begin closing without waiting for the GPU. Both detached and blanked surfaces are retained
    /// until a terminal queue callback is observed; dropping the last Queue can otherwise wait.
    pub fn begin_retirement(&mut self) -> Result<bool, String> {
        if self.retirement.closing() {
            return Ok(self.retirement.complete());
        }
        let detached = self.detach()?;
        self.cancel_measurement();
        self.retirement.begin(detached);
        if !detached {
            self.suspension.reveal();
            // The old helper is no longer a credit recipient during retirement.
            self.pictures.drain_released();
            self.clear_source();
        }
        self.arm_terminal_queue();
        Ok(false)
    }

    fn cancel_measurement(&mut self) {
        if let Some(timer) = self
            .measurement
            .as_mut()
            .and_then(|value| value.timer.as_mut())
        {
            timer.cancel();
        }
    }

    fn arm_terminal_queue(&mut self) {
        if !self.retirement.awaiting_queue() || self.terminal_queue_done.is_some() {
            return;
        }
        let done = Arc::new(AtomicBool::new(false));
        let completed = done.clone();
        self.queue
            .on_submitted_work_done(move || completed.store(true, Ordering::Release));
        self.terminal_queue_done = Some(done);
    }

    /// The native window no longer exists. It needs no transparent clear, but still owns GPU
    /// resources until all previously queued work completes. Never acquire another drawable.
    pub fn begin_window_retirement(&mut self) {
        if self.terminal_queue_done.is_some() || self.retirement.complete() {
            return;
        }
        self.cancel_measurement();
        let _ = self.detach(); // Best effort: window disappearance can invalidate native lookup.
        self.schedule.set_visible(false);
        self.retirement.window_gone();
        self.arm_terminal_queue();
    }

    /// Keep driving this on the native UI pump, separately from any replacement attachment.
    /// Hidden/occluded surfaces remain pending; neither skipping nor timing out proves a clear.
    pub fn poll_retirement(&mut self, visible: bool) -> Result<bool, String> {
        if !self.retirement.closing() {
            return Err("Stage compositor retirement has not begun".to_owned());
        }
        if self.retirement.complete() {
            return Ok(true);
        }
        self.poll_completions();
        if let Some(error) = self.source_error.take() {
            return Err(error);
        }
        self.arm_terminal_queue();
        if self.retirement.awaiting_queue() || self.retirement.complete() {
            self.pictures.drain_released();
            return Ok(self.retirement.complete());
        }
        self.set_window_visible(visible);
        self.set_visible(visible);
        self.draw()?;
        self.pictures.drain_released();
        Ok(self.retirement.complete())
    }

    /// The actual native Picture instruction, after all operator/benchmark resolution.
    pub(crate) fn set_timing_view(
        &mut self,
        mode: viz_helper::protocol::StageViewMode,
        quality: viz_helper::protocol::RenderQuality,
    ) {
        if let Some(timer) = self
            .measurement
            .as_mut()
            .and_then(|value| value.timer.as_mut())
        {
            timer.set_diagnostic(
                mode == viz_helper::protocol::StageViewMode::Lines3d
                    || quality == viz_helper::protocol::RenderQuality::Draft,
            );
        }
    }

    fn poll_completions(&mut self) {
        if let Err(error) = self.device.poll(wgpu::PollType::Poll) {
            self.source_error = Some(error.to_string());
        }
        if self
            .flight_done
            .as_ref()
            .is_some_and(|done| done.load(Ordering::Acquire))
        {
            self.flight_done = None;
            let completion = if self.flight_is_hidden_clear {
                self.flight_is_hidden_clear = false;
                self.suspension.completed();
                self.pictures.complete_preserving_front()
            } else {
                self.pictures.complete()
            };
            if let Err(error) = completion {
                self.source_error = Some(error);
            } else {
                self.retirement.completed();
            }
        }
        if let Some(measurement) = self.measurement.as_mut() {
            measurement.collect(&self.device);
        }
        if self
            .terminal_queue_done
            .as_ref()
            .is_some_and(|done| done.load(Ordering::Acquire))
        {
            self.retirement.queue_completed();
        }
    }

    pub fn drain_released_frames(&mut self) -> Vec<FrameId> {
        self.poll_completions();
        self.pictures.drain_released()
    }

    /// Drain bounded benchmark observations; polling never waits for pending GPU work.
    pub(crate) fn drain_measurements(&mut self) -> Option<CompositorMeasurements> {
        self.poll_completions();
        let measurement = self.measurement.as_mut()?;
        Some(measurement.drain())
    }

    /// Draw one frame: the pane where the layout put it, and nothing anywhere else.
    pub fn draw(&mut self) -> Result<DrawOutcome, String> {
        let started = std::time::Instant::now();
        self.poll_completions();
        if let Some(measurement) = self.measurement.as_mut() {
            measurement.stats.draw_attempts += 1;
        }
        let result = if let Some(error) = self.source_error.take() {
            Err(error)
        } else {
            self.draw_inner()
        };
        if result.is_err() {
            // Preserve dirty work but do not turn an immediately due failed draw into a
            // zero-delay native UI pump loop (also applies outside benchmark mode).
            self.schedule.retry(self.clock.elapsed());
        }
        if let Some(measurement) = self.measurement.as_mut() {
            measurement.stats.cpu_wall_micros = measurement
                .stats
                .cpu_wall_micros
                .saturating_add(started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64);
            if result.is_err() {
                measurement.stats.errors += 1;
            }
            match result.as_ref().ok() {
                Some(DrawOutcome::CleanSkipped) => measurement.stats.clean_skips += 1,
                Some(DrawOutcome::HiddenSkipped) => measurement.stats.hidden_skips += 1,
                Some(DrawOutcome::BusySkipped) => measurement.stats.busy_skips += 1,
                Some(DrawOutcome::PacedSkipped) => measurement.stats.paced_skips += 1,
                Some(DrawOutcome::Retry) => measurement.stats.retries += 1,
                _ => {}
            }
        }
        result
    }

    fn draw_inner(&mut self) -> Result<DrawOutcome, String> {
        use wgpu::CurrentSurfaceTexture;
        if self.retirement.awaiting_queue() || self.retirement.complete() {
            return Ok(DrawOutcome::HiddenSkipped);
        }
        if !self.window_visible {
            return Ok(DrawOutcome::HiddenSkipped);
        }
        let now = self.clock.elapsed();
        let hidden_clear = self.suspension.pending && !self.schedule.visible;
        let mut admission = self.schedule.clone();
        if hidden_clear {
            admission.visible = true;
        }
        let probe = match admission.admission(now, self.pictures.busy()) {
            Ok(probe) => probe,
            Err(outcome) => return Ok(outcome),
        };
        if self.needs_configure {
            self.configure();
        }
        let acquired_at = std::time::Instant::now();
        let acquired = self.surface.get_current_texture();
        if let Some(measurement) = self.measurement.as_mut() {
            measurement.stats.acquire_wall_micros = measurement
                .stats
                .acquire_wall_micros
                .saturating_add(acquired_at.elapsed().as_micros().min(u128::from(u64::MAX)) as u64);
        }
        let frame = match acquired {
            CurrentSurfaceTexture::Success(frame) | CurrentSurfaceTexture::Suboptimal(frame) => {
                frame
            }
            // A surface goes out of date while a display is reconfigured or the window moves
            // between screens. Reconfiguring and skipping one frame is the whole recovery.
            CurrentSurfaceTexture::Lost | CurrentSurfaceTexture::Outdated => {
                self.configure();
                self.schedule.retry(self.clock.elapsed());
                if probe {
                    self.schedule.acquired_occluded(now);
                }
                return Ok(DrawOutcome::Retry);
            }
            // Occluded is the window being hidden and timeout is the compositor being busy.
            // Neither is a fault, and neither has a frame to draw into.
            CurrentSurfaceTexture::Occluded => {
                if let Some(measurement) = self.measurement.as_mut() {
                    measurement.stats.occluded += 1;
                }
                self.schedule.acquired_occluded(now);
                self.pictures.discard_ready()?;
                return Ok(DrawOutcome::Occluded);
            }
            CurrentSurfaceTexture::Timeout => {
                if let Some(measurement) = self.measurement.as_mut() {
                    measurement.stats.timeouts += 1;
                }
                if probe {
                    self.schedule.acquired_occluded(now);
                }
                self.schedule.retry(self.clock.elapsed());
                return Ok(DrawOutcome::Retry);
            }
            CurrentSurfaceTexture::Validation => {
                return Err("the desk's window refused a frame".to_owned());
            }
        };
        if probe {
            // Release the acquired drawable without encoding/presenting a stale picture. The
            // host observes producer_visible() and asks the helper for its fresh current state.
            self.schedule.revealed();
            drop(frame);
            return Ok(DrawOutcome::Retry);
        }
        let mut selected = if hidden_clear {
            None
        } else {
            self.pictures.candidate().cloned()
        };
        let source = selected.as_ref().map(|picture| self.materialize(picture));
        if let (Some(picture), Some(source)) = (selected.as_mut(), source.as_ref()) {
            picture.data = Payload::Texture(source.clone());
        }
        let viewport = self.viewport();
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        if let Some(timer) = self
            .measurement
            .as_mut()
            .and_then(|value| value.timer.as_mut())
        {
            timer.begin_frame();
        }
        let encoder = self.encode_frame(&view, viewport, source.as_deref());
        let scope = CompositorScope {
            generation: self.generation,
            surface_width: self.surface_size.0,
            surface_height: self.surface_size.1,
            pane: viewport.map(|pane| [pane.0, pane.1, pane.2, pane.3]),
            source_size: source.as_ref().map(|source| [source.size.0, source.size.1]),
            transport: source.as_ref().map(|source| {
                if source.shared.is_some() {
                    "shared"
                } else {
                    "copy"
                }
            }),
        };
        let mut readback = false;
        if let Some(measurement) = self.measurement.as_mut() {
            measurement.submission_id = measurement.submission_id.saturating_add(1);
            measurement.stats.submissions += 1;
            if let Some(timer) = measurement.timer.as_mut() {
                readback = timer.resolve(measurement.submission_id, scope);
            }
        }
        let blank = selected.is_none();
        self.pictures.submit(selected)?;
        self.queue.submit(Some(encoder.finish()));
        self.flight_is_hidden_clear = hidden_clear;
        if hidden_clear {
            self.suspension.submitted();
        }
        self.retirement.submitted(blank);
        if readback
            && let Some(timer) = self
                .measurement
                .as_mut()
                .and_then(|measurement| measurement.timer.as_mut())
        {
            timer.request_readback();
        }
        self.queue.present(frame);
        let done = Arc::new(AtomicBool::new(false));
        let completed = done.clone();
        // The callback owns the backing as well as wgpu's resource handles, so an attachment
        // teardown cannot free an IOSurface that this GPU submission is still reading.
        self.queue.on_submitted_work_done(move || {
            drop(source);
            completed.store(true, Ordering::Release);
        });
        self.flight_done = Some(done);
        self.schedule.submitted(self.clock.elapsed());
        Ok(DrawOutcome::Submitted)
    }

    /// The pane in the surface's own pixels, or `None` when there is nothing to draw.
    fn viewport(&self) -> Option<(u32, u32, u32, u32)> {
        let pixels = self
            .pane
            .to_pixels(self.scale, self.surface_size.0, self.surface_size.1)?;
        Some((pixels.x, pixels.y, pixels.width, pixels.height))
    }

    fn bind(&self, texture: &wgpu::Texture) -> wgpu::BindGroup {
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("stage pane"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.uniform.as_entire_binding(),
                },
            ],
        })
    }

    fn configure(&mut self) {
        self.needs_configure = false;
        self.schedule.dirty();
        self.generation = self.generation.saturating_add(1);
        if let Some(measurement) = self.measurement.as_mut() {
            measurement.stats.reconfigures += 1;
        }
        self.surface.configure(
            &self.device,
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: self.format,
                width: self.surface_size.0,
                height: self.surface_size.1,
                present_mode: wgpu::PresentMode::AutoVsync,
                // Whatever the display is already showing the interface in. The pane must match
                // the chrome around it, and the desk does not manage colour itself.
                color_space: wgpu::SurfaceColorSpace::Auto,
                alpha_mode: self.alpha_mode,
                view_formats: Vec::new(),
                desired_maximum_frame_latency: 2,
            },
        );
    }
}
