//! Off-screen output and readback.
//!
//! An off-screen output renders every pass exactly as a windowed one does — same pipelines, same
//! blending, same master pass — so a reference render proves what a display would show. Readback
//! is also how the CITP preview frame is taken, which is why it is a separate step the caller
//! asks for rather than something every frame pays for.

use media_domain::geometry::Size;

use crate::compositor::PROGRAM_FORMAT;
use crate::gpu::Gpu;

/// A render target with no window.
pub struct OffScreenOutput {
    gpu: Gpu,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    size: Size,
    format: wgpu::TextureFormat,
}

impl OffScreenOutput {
    pub fn new(gpu: &Gpu, size: Size) -> Self {
        Self::with_format(gpu, size, PROGRAM_FORMAT)
    }

    /// A target in a stated format.
    ///
    /// A pipeline is built for one attachment format, so a target that a windowed output's master
    /// pass will draw into has to match that window's surface — not the program format.
    pub fn with_format(gpu: &Gpu, size: Size, format: wgpu::TextureFormat) -> Self {
        let (texture, view) = target(&gpu.device, size, format);
        Self {
            gpu: gpu.clone(),
            texture,
            view,
            size,
            format,
        }
    }

    pub const fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    pub const fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub const fn size(&self) -> Size {
        self.size
    }

    pub fn resize(&mut self, size: Size) {
        if size == self.size || size.is_empty() {
            return;
        }
        let (texture, view) = target(&self.gpu.device, size, self.format);
        self.texture = texture;
        self.view = view;
        self.size = size;
    }

    /// Reads the rendered image back as tightly packed 8-bit RGBA.
    pub fn read_image(&self) -> Vec<u8> {
        read_rgba8(&self.gpu, &self.texture, self.size)
    }

    /// The pixel at a position, as 8-bit RGBA. Convenience for reference-render assertions.
    pub fn pixel(image: &[u8], size: Size, x: u32, y: u32) -> [u8; 4] {
        let index = (y as usize * size.width as usize + x as usize) * 4;
        [
            image[index],
            image[index + 1],
            image[index + 2],
            image[index + 3],
        ]
    }
}

/// Reads any 8-bit RGBA texture back as tightly packed pixels.
///
/// The copy itself needs 256-byte-aligned rows, so the padding is added for the transfer and
/// removed again here; callers see width × height × 4 bytes and nothing else.
pub fn read_rgba8(gpu: &Gpu, texture: &wgpu::Texture, size: Size) -> Vec<u8> {
    {
        let unpadded_row = size.width as usize * 4;
        let alignment = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize;
        let padded_row = unpadded_row.div_ceil(alignment) * alignment;

        let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("media-readback"),
            size: (padded_row * size.height as usize) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("media-readback"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row as u32),
                    rows_per_image: Some(size.height),
                },
            },
            wgpu::Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
        );
        gpu.queue.submit([encoder.finish()]);

        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = gpu.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });

        let mapped = slice
            .get_mapped_range()
            .expect("the readback buffer was mapped and the device polled to completion");
        let mut pixels = Vec::with_capacity(unpadded_row * size.height as usize);
        for row in 0..size.height as usize {
            let start = row * padded_row;
            pixels.extend_from_slice(&mapped[start..start + unpadded_row]);
        }
        drop(mapped);
        buffer.unmap();
        pixels
    }
}

fn target(
    device: &wgpu::Device,
    size: Size,
    format: wgpu::TextureFormat,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("media-offscreen"),
        size: wgpu::Extent3d {
            width: size.width.max(1),
            height: size.height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}
