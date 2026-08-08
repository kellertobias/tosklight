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
}

impl OffScreenOutput {
    pub fn new(gpu: &Gpu, size: Size) -> Self {
        let (texture, view) = target(&gpu.device, size);
        Self {
            gpu: gpu.clone(),
            texture,
            view,
            size,
        }
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
        let (texture, view) = target(&self.gpu.device, size);
        self.texture = texture;
        self.view = view;
        self.size = size;
    }

    /// Reads the rendered image back as tightly packed 8-bit RGBA.
    ///
    /// The copy itself needs 256-byte-aligned rows, so the padding is added for the transfer and
    /// removed again here; callers see width × height × 4 bytes and nothing else.
    pub fn read_image(&self) -> Vec<u8> {
        let unpadded_row = self.size.width as usize * 4;
        let alignment = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize;
        let padded_row = unpadded_row.div_ceil(alignment) * alignment;

        let buffer = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("media-readback"),
            size: (padded_row * self.size.height as usize) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("media-readback"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row as u32),
                    rows_per_image: Some(self.size.height),
                },
            },
            wgpu::Extent3d {
                width: self.size.width,
                height: self.size.height,
                depth_or_array_layers: 1,
            },
        );
        self.gpu.queue.submit([encoder.finish()]);

        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = self.gpu.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });

        let mapped = slice
            .get_mapped_range()
            .expect("the readback buffer was mapped and the device polled to completion");
        let mut pixels = Vec::with_capacity(unpadded_row * self.size.height as usize);
        for row in 0..self.size.height as usize {
            let start = row * padded_row;
            pixels.extend_from_slice(&mapped[start..start + unpadded_row]);
        }
        drop(mapped);
        buffer.unmap();
        pixels
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

fn target(device: &wgpu::Device, size: Size) -> (wgpu::Texture, wgpu::TextureView) {
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
        format: PROGRAM_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}
