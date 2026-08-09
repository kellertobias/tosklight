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
    /// Kept so a generated source can be read back — for a reference render, and later for the
    /// CITP preview, which asks for pixels only while a desk is subscribed.
    texture: wgpu::Texture,
    /// Whether this texture may be copied out of. Uploaded sources are write-only by design.
    readable: bool,
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
            texture,
            readable: false,
            size,
        })
    }

    /// Uploads BC3 blocks straight to the GPU, the way a HAP frame is stored.
    ///
    /// This is the cheap path and the reason the format was chosen: nothing expands the frame, the
    /// blocks go to the GPU as they came off disk. Requires an adapter that samples BC — check
    /// [`Gpu::samples_block_compression`] first and expand the blocks yourself where it does not.
    pub fn from_bc3_blocks(gpu: &Gpu, size: Size, blocks: &[u8]) -> Result<Self, TextureError> {
        if size.is_empty() {
            return Err(TextureError::Empty);
        }
        if !gpu.samples_block_compression() {
            return Err(TextureError::NoBlockCompression);
        }
        let expected = block_bytes(size);
        if blocks.len() != expected {
            return Err(TextureError::WrongLength {
                expected,
                found: blocks.len(),
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
                label: Some("media-source-bc3"),
                size: wgpu::Extent3d {
                    width: size.width,
                    height: size.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                // Linear rather than sRGB, matching the program target, so a windowed output and
                // an off-screen reference render agree.
                format: wgpu::TextureFormat::Bc3RgbaUnorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            blocks,
        );

        Ok(Self {
            view: texture.create_view(&wgpu::TextureViewDescriptor::default()),
            texture,
            readable: false,
            size,
        })
    }

    /// A texture a pass draws into, which then reads back as an ordinary source.
    ///
    /// This is how a generated visualizer reaches the compositor: it renders here, and everything
    /// downstream treats the result exactly like a decoded frame.
    pub fn render_target(gpu: &Gpu, size: Size) -> Result<Self, TextureError> {
        if size.is_empty() {
            return Err(TextureError::Empty);
        }
        if !gpu.supports_resolution(size.width, size.height) {
            return Err(TextureError::TooLarge {
                width: size.width,
                height: size.height,
                limit: gpu.capabilities.max_texture_dimension,
            });
        }
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("media-generated-source"),
            size: wgpu::Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: VISUALIZER_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        Ok(Self {
            view: texture.create_view(&wgpu::TextureViewDescriptor::default()),
            texture,
            readable: true,
            size,
        })
    }

    /// Reads a generated source back as tightly packed 8-bit RGBA.
    pub fn read_rgba8(&self, gpu: &Gpu) -> Result<Vec<u8>, TextureError> {
        if !self.readable {
            return Err(TextureError::NotReadable);
        }
        Ok(crate::offscreen::read_rgba8(gpu, &self.texture, self.size))
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

/// What a generated source renders into. Linear, matching every other source, so a reference
/// render of a visualizer is byte-identical wherever it runs.
pub const VISUALIZER_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// The bytes a BC3 image of this size occupies. Block formats cover whole 4x4 tiles, so a
/// dimension that is not a multiple of four still pays for its partial tiles.
pub const fn block_bytes(size: Size) -> usize {
    size.width.div_ceil(4) as usize * size.height.div_ceil(4) as usize * 16
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
    #[error("this adapter does not sample BC textures; expand the blocks to RGBA instead")]
    NoBlockCompression,
    #[error("an uploaded source cannot be read back; only a generated one can")]
    NotReadable,
}
