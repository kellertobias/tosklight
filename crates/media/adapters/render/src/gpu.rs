//! Device ownership and capability validation.
//!
//! Startup validation reports a missing capability clearly instead of silently producing an empty
//! source. A machine that genuinely lacks a GPU is allowed to say so; a platform adapter that was
//! never written is not.

use std::sync::Arc;

use wgpu::{
    Adapter, Backends, Device, Instance, InstanceDescriptor, Queue, RequestAdapterOptions, Surface,
};

/// Anything one output can present into.
///
/// The renderer takes a surface adapter rather than owning a window, so an output can be a
/// monitor, an off-screen target, or a test harness without the renderer knowing which.
pub trait PresentationSurface {
    fn create_surface(&self, instance: &Instance) -> Result<Surface<'static>, String>;
    /// Physical pixel size of the surface.
    fn size(&self) -> (u32, u32);
}

/// Why an output cannot render.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GpuError {
    #[error("no GPU or software adapter is available: {detail}")]
    NoAdapter { detail: String },
    #[error("the graphics device could not be opened: {detail}")]
    NoDevice { detail: String },
    #[error("this surface is not supported by the selected adapter")]
    UnsupportedSurface,
    #[error("the surface could not be created: {detail}")]
    SurfaceCreation { detail: String },
}

/// The device and queue one output renders with, and what the adapter can do.
#[derive(Clone)]
pub struct Gpu {
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
    pub adapter: Arc<Adapter>,
    pub capabilities: Capabilities,
}

/// What the selected adapter reports about itself.
///
/// Recorded rather than assumed, so diagnostics can say why an output behaves the way it does on
/// one machine and differently on another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capabilities {
    pub adapter_name: String,
    pub backend: String,
    /// Whether this adapter is a software rasterizer. A build machine is exactly where that
    /// happens, and a reference render is still valid on one.
    pub is_software: bool,
    pub max_texture_dimension: u32,
}

impl Gpu {
    /// Opens a device for a real presentation surface.
    pub fn for_surface(
        target: &dyn PresentationSurface,
    ) -> Result<(Self, Surface<'static>), GpuError> {
        let instance = new_instance();
        let surface = target
            .create_surface(&instance)
            .map_err(|detail| GpuError::SurfaceCreation { detail })?;
        let adapter = request_adapter(&instance, Some(&surface))?;
        let gpu = Self::open(adapter)?;
        Ok((gpu, surface))
    }

    /// Opens a device with no window at all.
    ///
    /// Off-screen outputs, CITP preview-only operation, and the deterministic reference renders
    /// all take this path. A software adapter is accepted when no hardware one answers, because
    /// a build machine is exactly where that happens — but it fails loudly rather than quietly
    /// producing nothing, since a capture that silently did not render is indistinguishable from
    /// one that rendered black.
    pub fn off_screen() -> Result<Self, GpuError> {
        let instance = new_instance();
        Self::open(request_adapter(&instance, None)?)
    }

    fn open(adapter: Adapter) -> Result<Self, GpuError> {
        let info = adapter.get_info();
        let limits = adapter.limits();
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("media-output"),
            // BC sampling is requested when the adapter has it, so HAP frames upload compressed
            // rather than being expanded. An adapter without it still works; the frames are
            // expanded on the way in.
            required_features: adapter.features() & wgpu::Features::TEXTURE_COMPRESSION_BC,
            required_limits: limits.clone(),
            ..Default::default()
        }))
        .map_err(|error| GpuError::NoDevice {
            detail: error.to_string(),
        })?;

        Ok(Self {
            device: Arc::new(device),
            queue: Arc::new(queue),
            capabilities: Capabilities {
                adapter_name: info.name,
                backend: format!("{:?}", info.backend),
                is_software: matches!(info.device_type, wgpu::DeviceType::Cpu),
                max_texture_dimension: limits.max_texture_dimension_2d,
            },
            adapter: Arc::new(adapter),
        })
    }

    /// Whether this adapter samples BC (DXT/S3TC) textures, which is what a HAP frame stores.
    ///
    /// Desktop GPUs all do; some ARM parts expose only ETC2 and ASTC. Where this is false the
    /// blocks are expanded to RGBA on the way in rather than the format being unsupported.
    pub fn samples_block_compression(&self) -> bool {
        self.adapter
            .features()
            .contains(wgpu::Features::TEXTURE_COMPRESSION_BC)
    }

    /// Whether this adapter can render an output of the requested size.
    pub fn supports_resolution(&self, width: u32, height: u32) -> bool {
        let limit = self.capabilities.max_texture_dimension;
        width > 0 && height > 0 && width <= limit && height <= limit
    }

    /// The presentation mode to configure a surface with, chosen from what the surface actually
    /// supports rather than from a hard-coded rate.
    pub fn choose_present_mode(
        &self,
        surface: &Surface<'static>,
        requested: media_domain::PresentationMode,
    ) -> wgpu::PresentMode {
        let supported = surface.get_capabilities(&self.adapter).present_modes;
        let preferred: &[wgpu::PresentMode] = match requested {
            // Follow the display. FIFO is the vsynchronized mode every backend must support.
            media_domain::PresentationMode::DisplaySynchronized => &[wgpu::PresentMode::Fifo],
            // A fixed-rate output schedules against its own monotonic deadlines, so it wants the
            // surface to hand a frame back as soon as it can rather than blocking on the display.
            media_domain::PresentationMode::FixedFps { .. }
            | media_domain::PresentationMode::Unlocked => &[
                wgpu::PresentMode::Mailbox,
                wgpu::PresentMode::Immediate,
                wgpu::PresentMode::Fifo,
            ],
        };
        preferred
            .iter()
            .copied()
            .find(|candidate| supported.contains(candidate))
            // FIFO is guaranteed by the specification, so this is a real fallback rather than a
            // guess.
            .unwrap_or(wgpu::PresentMode::Fifo)
    }
}

fn new_instance() -> Instance {
    let mut descriptor = InstanceDescriptor::new_without_display_handle();
    descriptor.backends = Backends::from_env().unwrap_or(Backends::PRIMARY);
    Instance::new(descriptor)
}

fn request_adapter(
    instance: &Instance,
    surface: Option<&Surface<'static>>,
) -> Result<Adapter, GpuError> {
    pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: surface,
        force_fallback_adapter: false,
        ..Default::default()
    }))
    .or_else(|_| {
        pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: surface,
            force_fallback_adapter: true,
            ..Default::default()
        }))
    })
    .map_err(|error| GpuError::NoAdapter {
        detail: error.to_string(),
    })
}
