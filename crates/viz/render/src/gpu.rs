//! Device, queue, and swapchain ownership.
//!
//! The render core takes a presentation-surface adapter rather than owning a window, so the
//! standalone application can supply a native window while a future in-process host can supply
//! an embedded viewport.

use std::sync::Arc;
use wgpu::{
    Adapter, Backends, Device, DeviceDescriptor, Instance, InstanceDescriptor, Queue,
    RequestAdapterOptions, Surface, SurfaceConfiguration, TextureFormat,
};

/// Anything the render core can present into.
pub trait PresentationSurface {
    /// Create the surface for `instance`.
    fn create_surface(&self, instance: &Instance) -> Result<Surface<'static>, String>;
    /// Physical pixel size of the surface.
    fn size(&self) -> (u32, u32);
}

pub struct Gpu {
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
    /// The swapchain this renders into, or `None` when there is no window at all.
    ///
    /// A headless renderer still draws every pass exactly as an interactive one does — the frame
    /// path already renders into a supplied texture view whenever a capture is requested, and
    /// takes the swapchain only when one is not. So "headless" is the absence of a surface and
    /// nothing else: same scene projection, same materials, same lighting, same quality tiers.
    pub surface: Option<Surface<'static>>,
    pub config: SurfaceConfiguration,
    pub format: TextureFormat,
    pub adapter_name: String,
    pub backend: String,
    pub timestamps: bool,
    /// Samples per pixel the shaded passes are drawn with.
    pub samples: u32,
}

impl Gpu {
    pub fn new(target: &dyn PresentationSurface) -> Result<Self, String> {
        let mut descriptor = InstanceDescriptor::new_without_display_handle();
        descriptor.backends = Backends::from_env().unwrap_or(Backends::PRIMARY);
        let instance = Instance::new(descriptor);
        let surface = target.create_surface(&instance)?;
        let adapter = pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
            ..Default::default()
        }))
        .map_err(|error| format!("no compatible GPU adapter: {error}"))?;
        let info = adapter.get_info();
        let timestamps = adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY);
        let samples = preferred_sample_count(&adapter);
        let (device, queue) = Self::open_device(&adapter, timestamps)?;
        let (width, height) = target.size();
        let capabilities = surface.get_capabilities(&adapter);
        let format = preferred_format(&capabilities.formats);
        let present_mode = preferred_present_mode(&capabilities.present_modes);
        let mut config = surface
            .get_default_config(&adapter, width.max(1), height.max(1))
            .ok_or_else(|| "the surface is not supported by this adapter".to_owned())?;
        config.format = format;
        config.present_mode = present_mode;
        // One frame of latency keeps the presented image as close to the newest packet as the
        // platform allows.
        config.desired_maximum_frame_latency = 1;
        surface.configure(&device, &config);
        Ok(Self {
            device: Arc::new(device),
            queue: Arc::new(queue),
            surface: Some(surface),
            config,
            format,
            adapter_name: info.name,
            backend: format!("{:?}", info.backend),
            timestamps,
            samples,
        })
    }

    /// A renderer with no window, for deterministic capture on a machine that has no display.
    ///
    /// The adapter is chosen without a compatible surface, and a software adapter is accepted when
    /// no hardware one answers, because a build machine is exactly where that happens. It fails
    /// loudly rather than quietly producing nothing: a capture that silently did not render is
    /// indistinguishable from one that rendered a black stage.
    pub fn headless(width: u32, height: u32) -> Result<Self, String> {
        let mut descriptor = InstanceDescriptor::new_without_display_handle();
        descriptor.backends = Backends::from_env().unwrap_or(Backends::PRIMARY);
        let instance = Instance::new(descriptor);
        let adapter = pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
            ..Default::default()
        }))
        .or_else(|_| {
            pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: true,
                ..Default::default()
            }))
        })
        .map_err(|error| {
            format!("no GPU or software adapter is available for headless rendering: {error}")
        })?;
        let info = adapter.get_info();
        let timestamps = adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY);
        let samples = preferred_sample_count(&adapter);
        let (device, queue) = Self::open_device(&adapter, timestamps)?;
        // Straight RGBA, so a capture is byte-identical wherever it runs rather than depending on
        // whatever channel order a swapchain would have preferred on this machine.
        let format = TextureFormat::Rgba8UnormSrgb;
        let config = SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: width.max(1),
            height: height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 1,
            alpha_mode: wgpu::CompositeAlphaMode::Opaque,
            view_formats: Vec::new(),
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        Ok(Self {
            device: Arc::new(device),
            queue: Arc::new(queue),
            surface: None,
            config,
            format,
            adapter_name: info.name,
            backend: format!("{:?}", info.backend),
            timestamps,
            samples,
        })
    }

    fn open_device(adapter: &Adapter, timestamps: bool) -> Result<(Device, Queue), String> {
        let mut features = wgpu::Features::empty();
        if timestamps {
            features |= wgpu::Features::TIMESTAMP_QUERY;
        }
        let mut limits = adapter.limits();
        limits.max_storage_buffers_per_shader_stage = limits
            .max_storage_buffers_per_shader_stage
            .max(wgpu::Limits::default().max_storage_buffers_per_shader_stage);
        pollster::block_on(adapter.request_device(&DeviceDescriptor {
            label: Some("viz-render device"),
            required_features: features,
            required_limits: limits,
            ..Default::default()
        }))
        .map_err(|error| format!("GPU device request failed: {error}"))
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        if self.config.width == width && self.config.height == height {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        if let Some(surface) = &self.surface {
            surface.configure(&self.device, &self.config);
        }
    }

    /// Reconfigure after a recoverable surface loss. A headless renderer has nothing to lose.
    pub fn reconfigure(&self) {
        if let Some(surface) = &self.surface {
            surface.configure(&self.device, &self.config);
        }
    }

    pub fn aspect(&self) -> f32 {
        self.config.width.max(1) as f32 / self.config.height.max(1) as f32
    }
}

fn preferred_format(formats: &[TextureFormat]) -> TextureFormat {
    for candidate in [TextureFormat::Bgra8UnormSrgb, TextureFormat::Rgba8UnormSrgb] {
        if formats.contains(&candidate) {
            return candidate;
        }
    }
    formats
        .first()
        .copied()
        .unwrap_or(TextureFormat::Bgra8UnormSrgb)
}

/// How many samples per pixel the shaded passes get.
///
/// Four where the adapter offers it, because that is where the edge of a truss against a beam
/// stops crawling; two as the fallback; one where multisampling is unavailable rather than
/// refusing to draw. The count is decided once, for the life of the process: it is baked into
/// every pipeline and into the depth binding the beam pass reads, so it cannot follow the
/// quality tier without rebuilding both. `TOSKLIGHT_VIZ_SAMPLES` pins it for a benchmark that
/// wants the two halves compared.
fn preferred_sample_count(adapter: &Adapter) -> u32 {
    let supported = |count: u32| {
        adapter
            .get_texture_format_features(crate::targets::HDR_FORMAT)
            .flags
            .sample_count_supported(count)
            && adapter
                .get_texture_format_features(crate::targets::DEPTH_FORMAT)
                .flags
                .sample_count_supported(count)
    };
    if let Ok(requested) = std::env::var("TOSKLIGHT_VIZ_SAMPLES")
        && let Ok(count) = requested.trim().parse::<u32>()
    {
        return if count <= 1 || supported(count) {
            count.max(1)
        } else {
            1
        };
    }
    for candidate in [4, 2] {
        if supported(candidate) {
            return candidate;
        }
    }
    1
}

fn preferred_present_mode(modes: &[wgpu::PresentMode]) -> wgpu::PresentMode {
    // Mailbox keeps the presented frame newest, which directly reduces packet-to-visible
    // latency. Fifo is the guaranteed fallback.
    for candidate in [wgpu::PresentMode::Mailbox, wgpu::PresentMode::Fifo] {
        if modes.contains(&candidate) {
            return candidate;
        }
    }
    wgpu::PresentMode::Fifo
}
