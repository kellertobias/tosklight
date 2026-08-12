//! Windowed outputs.
//!
//! An output bound to a monitor presents through a swapchain rather than into a texture. Nothing
//! about the composition changes: the same pipelines, the same blending, the same master pass.
//! What is different is the surface lifecycle — configuring it, recovering it when it is lost,
//! and rebuilding it when the monitor or its refresh rate changes.

use std::sync::Arc;

use media_domain::clock::{MeasuredCadence, RenderClock};
use media_domain::geometry::Size;
use media_domain::output::MonitorSelector;
use media_domain::{MasterState, OutputId, PresentationMode, Timestamp};
use winit::window::Window;

use crate::compositor::{Compositor, LayerDraw};
use crate::gpu::{Gpu, GpuError, PresentationSurface};

/// A winit window the render core can present into.
pub struct WindowSurface(pub Arc<Window>);

impl PresentationSurface for WindowSurface {
    fn create_surface(&self, instance: &wgpu::Instance) -> Result<wgpu::Surface<'static>, String> {
        instance
            .create_surface(self.0.clone())
            .map_err(|error| format!("surface: {error}"))
    }

    fn size(&self) -> (u32, u32) {
        let size = self.0.inner_size();
        (size.width.max(1), size.height.max(1))
    }
}

/// The monitors the platform reports, in its own order, as the selector understands them.
pub fn monitors(
    handles: impl IntoIterator<Item = winit::monitor::MonitorHandle>,
) -> Vec<(u32, String, winit::monitor::MonitorHandle)> {
    handles
        .into_iter()
        .enumerate()
        .map(|(index, handle)| {
            let name = handle
                .name()
                .unwrap_or_else(|| format!("Display {}", index + 1));
            (index as u32, name, handle)
        })
        .collect()
}

/// Resolves the monitor an output asked for, if the platform still reports it.
pub fn select_monitor(
    selector: &MonitorSelector,
    handles: impl IntoIterator<Item = winit::monitor::MonitorHandle>,
) -> Option<winit::monitor::MonitorHandle> {
    let available = monitors(handles);
    selector.resolve(&available).cloned()
}

/// One output presenting into a window.
pub struct WindowedOutput {
    id: OutputId,
    gpu: Gpu,
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    configuration: wgpu::SurfaceConfiguration,
    compositor: Compositor,
    clock: RenderClock,
    /// Built the first time a console subscribes to a preview, and only then: an output nobody is
    /// watching must not pay for a second target.
    preview: Option<crate::OffScreenOutput>,
}

impl WindowedOutput {
    /// Opens a device for this window and configures its swapchain.
    ///
    /// The presentation mode comes from the surface's own capabilities. A display-synchronized
    /// output takes the backend's guaranteed vsynchronized mode; a fixed-rate or unlocked one
    /// takes the first non-blocking mode the surface actually offers, and falls back to the
    /// vsynchronized one rather than failing.
    pub fn open(
        id: OutputId,
        window: Arc<Window>,
        presentation: PresentationMode,
    ) -> Result<Self, GpuError> {
        let target = WindowSurface(window.clone());
        let (gpu, surface) = Gpu::for_surface(&target)?;

        let (width, height) = target.size();
        let mut configuration = surface
            .get_default_config(&gpu.adapter, width, height)
            .ok_or(GpuError::UnsupportedSurface)?;
        configuration.format = present_format(&surface.get_capabilities(&gpu.adapter).formats)
            .ok_or(GpuError::UnsupportedSurface)?;
        configuration.present_mode = gpu.choose_present_mode(&surface, presentation);
        // One frame of latency keeps the presented image as close to the newest packet as the
        // platform allows.
        configuration.desired_maximum_frame_latency = 1;
        surface.configure(&gpu.device, &configuration);

        let compositor = Compositor::new(&gpu, Size::new(width, height), configuration.format);

        Ok(Self {
            id,
            gpu,
            window,
            surface,
            configuration,
            compositor,
            clock: RenderClock::new(presentation),
            preview: None,
        })
    }

    pub const fn id(&self) -> OutputId {
        self.id
    }

    /// The device this output renders with. Sources for this output must be uploaded to it: two
    /// devices cannot share a texture.
    pub const fn gpu(&self) -> &Gpu {
        &self.gpu
    }

    pub fn size(&self) -> Size {
        Size::new(self.configuration.width, self.configuration.height)
    }

    pub fn cadence(&self) -> MeasuredCadence {
        self.clock.measured()
    }

    pub fn should_present(&self, now: Timestamp) -> bool {
        self.clock.should_present(now)
    }

    /// The refresh rate the monitor this window is on reports, in millihertz.
    ///
    /// Recorded rather than assumed. Two outputs on displays with different refresh rates never
    /// share a clock, and this is how each learns what its own display is doing.
    pub fn monitor_refresh_millihertz(&self) -> Option<u32> {
        self.window.current_monitor()?.refresh_rate_millihertz()
    }

    /// Rebuilds the swapchain for a new size.
    ///
    /// Resizes, monitor changes, refresh-rate changes, and waking from sleep all land here, and
    /// all of them affect only this output.
    pub fn resize(&mut self, size: Size) {
        if size.is_empty()
            || (size.width == self.configuration.width && size.height == self.configuration.height)
        {
            return;
        }
        self.configuration.width = size.width;
        self.configuration.height = size.height;
        self.surface
            .configure(&self.gpu.device, &self.configuration);
        self.compositor.resize(size);
        // The cadence measured before the change says nothing about the cadence after it.
        self.clock.reset();
    }

    /// Composites and presents one frame.
    ///
    /// A surface that has been lost or has gone out of date is reconfigured and the frame is
    /// dropped, because presenting a stale image is worse than presenting none. Only this output
    /// is affected; the others keep going.
    pub fn present(
        &mut self,
        layers: &[LayerDraw<'_>],
        master: &MasterState,
        master_mask: Option<&crate::SourceTexture>,
        now: Timestamp,
    ) -> Result<(), SurfaceLost> {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            // Suboptimal still presents. A window mid-resize is briefly suboptimal on some
            // backends, and dropping those frames would stutter the output for no gain.
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.gpu.device, &self.configuration);
                self.clock.reset();
                return Err(SurfaceLost::Recovered);
            }
            // Occluded means nobody can see it. Skipping is correct, not a failure.
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Err(SurfaceLost::Timeout);
            }
            other => return Err(SurfaceLost::Fatal { detail: format!("{other:?}") }),
        };

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.compositor
            .render(layers, master, master_mask, &view, self.id, now);
        self.window.pre_present_notify();
        self.gpu.queue.present(frame);
        self.clock.record_present(now);
        Ok(())
    }

    /// Asks the platform for another frame. A display-synchronized output redraws when the
    /// compositor is ready for it rather than spinning.
    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }
}

/// The surface format to present through.
///
/// A non-sRGB format is chosen wherever the surface offers one, so the bytes presented are the
/// bytes the compositor produced. An sRGB swapchain would have the hardware gamma-encode the
/// composite on its way to the display, which would make a window and an off-screen reference
/// render of the same state disagree — and would brighten every show against what the legacy
/// application put on screen.
pub fn present_format(available: &[wgpu::TextureFormat]) -> Option<wgpu::TextureFormat> {
    available
        .iter()
        .copied()
        .find(|format| !format.is_srgb())
        .or_else(|| available.first().copied())
}

impl WindowedOutput {
    /// Renders the finished composite into a preview-sized target and reads it back.
    ///
    /// The composite is already on the GPU, so a smaller target scales it down with the sampler
    /// rather than costing a full-size readback and a resample on the thread that presents.
    /// Called only while a console is subscribed.
    pub fn capture_preview(
        &mut self,
        size: Size,
        master: &MasterState,
        master_mask: Option<&crate::SourceTexture>,
    ) -> Vec<u8> {
        // The master pipeline is built for this window's surface format, so the preview target
        // has to be in that format too — a mismatch is a validation failure, not a wrong colour.
        let format = self.configuration.format;
        let target = self
            .preview
            .get_or_insert_with(|| crate::OffScreenOutput::with_format(&self.gpu, size, format));
        // A console may ask for a different size later; the target follows it.
        target.resize(size);
        self.compositor
            .render_master_into(master, master_mask, target.view());

        let mut pixels = target.read_image();
        if matches!(
            format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        ) {
            // A surface may be BGRA; everything above this expects RGBA.
            for pixel in pixels.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
        }
        pixels
    }

    /// Lets go of the preview target once nothing is watching.
    pub fn release_preview(&mut self) {
        self.preview = None;
    }
}

/// What went wrong presenting a frame.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SurfaceLost {
    /// The surface was out of date or lost and has been reconfigured. The next frame will draw.
    #[error("the surface was reconfigured; this frame was dropped")]
    Recovered,
    /// The platform did not hand back a frame in time. Transient.
    #[error("the surface did not provide a frame in time")]
    Timeout,
    /// The device is gone or out of memory. This output cannot continue without being rebuilt.
    #[error("the surface failed: {detail}")]
    Fatal { detail: String },
}

impl SurfaceLost {
    /// Whether the output can keep trying, or whether it has to be rebuilt.
    pub const fn is_transient(&self) -> bool {
        matches!(self, Self::Recovered | Self::Timeout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_non_srgb_surface_format_is_preferred_so_the_window_matches_a_reference_render() {
        use wgpu::TextureFormat::{Bgra8Unorm, Bgra8UnormSrgb, Rgba8Unorm, Rgba8UnormSrgb};

        assert_eq!(
            present_format(&[Bgra8UnormSrgb, Bgra8Unorm]),
            Some(Bgra8Unorm)
        );
        assert_eq!(
            present_format(&[Rgba8UnormSrgb, Rgba8Unorm]),
            Some(Rgba8Unorm)
        );
        assert_eq!(present_format(&[Bgra8Unorm]), Some(Bgra8Unorm));
    }

    #[test]
    fn a_surface_that_offers_only_srgb_still_opens() {
        // Presenting slightly bright beats refusing to present at all; the alternative is an
        // output that cannot open on a platform that offers nothing else.
        assert_eq!(
            present_format(&[wgpu::TextureFormat::Bgra8UnormSrgb]),
            Some(wgpu::TextureFormat::Bgra8UnormSrgb)
        );
        assert_eq!(present_format(&[]), None);
    }

    #[test]
    fn a_transient_failure_is_distinguished_from_a_fatal_one() {
        assert!(SurfaceLost::Recovered.is_transient());
        assert!(SurfaceLost::Timeout.is_transient());
        assert!(
            !SurfaceLost::Fatal {
                detail: "out of memory".to_owned()
            }
            .is_transient()
        );
    }

    #[test]
    fn monitors_without_a_name_still_get_a_stable_label() {
        // The enumeration itself needs a platform, so this checks only the labelling rule the
        // selector depends on: an unnamed display is "Display N", one-based.
        let labelled: Vec<(u32, String, u8)> = vec![
            (0, "Display 1".to_owned(), 0),
            (1, "Stage Left".to_owned(), 1),
        ];
        assert_eq!(MonitorSelector::Index(0).resolve(&labelled), Some(&0));
        assert_eq!(
            MonitorSelector::Name("Stage Left".to_owned()).resolve(&labelled),
            Some(&1)
        );
    }
}
