//! Source textures.
//!
//! Slice 2 covers still images. A video frame arrives through the same type once playback exists:
//! the compositor only ever sees a texture and its dimensions.

use media_domain::geometry::Size;
use wgpu::util::DeviceExt as _;

use crate::gpu::Gpu;

/// One layer's visual input, already on the GPU.
pub struct SourceTexture {
    pub(crate) view: wgpu::TextureView,
    size: Size,
}

impl SourceTexture {
    /// Uploads straight 8-bit RGBA.
    ///
    /// The format is linear rather than sRGB so a reference render is byte-identical wherever it
    /// runs, instead of depending on which transfer function a swapchain preferred on that
    /// machine.
    pub fn from_rgba8(gpu: &Gpu, size: Size, pixels: &[u8]) -> Result<Self, TextureError> {
        let expected = size.width as usize * size.height as usize * 4;
        if size.is_empty() {
            return Err(TextureError::Empty);
        }
        if pixels.len() != expected {
            return Err(TextureError::WrongLength {
                expected,
                found: pixels.len(),
            });
        }
        if !gpu.supports_resolution(size.width, size.height) {
            return Err(TextureError::TooLarge {
                width: size.width,
                height: size.height,
                limit: gpu.capabilities.max_texture_dimension,
            });
        }

        let texture = gpu.device.create_texture_with_data(
            &gpu.queue,
            &wgpu::TextureDescriptor {
                label: Some("media-source"),
                size: wgpu::Extent3d {
                    width: size.width,
                    height: size.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            pixels,
        );

        Ok(Self {
            view: texture.create_view(&wgpu::TextureViewDescriptor::default()),
            size,
        })
    }

    /// A single-colour texture, for tests and for the solid backgrounds a generated source needs
    /// before its own slice arrives.
    pub fn solid(gpu: &Gpu, size: Size, colour: [u8; 4]) -> Result<Self, TextureError> {
        let pixels = colour.repeat(size.width as usize * size.height as usize);
        Self::from_rgba8(gpu, size, &pixels)
    }

    pub const fn size(&self) -> Size {
        self.size
    }
}

/// Why a source could not be uploaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TextureError {
    #[error("a source texture cannot have a zero width or height")]
    Empty,
    #[error("the pixel buffer holds {found} bytes but the size needs {expected}")]
    WrongLength { expected: usize, found: usize },
    #[error("a {width}x{height} source exceeds this adapter's {limit}-pixel texture limit")]
    TooLarge { width: u32, height: u32, limit: u32 },
}
