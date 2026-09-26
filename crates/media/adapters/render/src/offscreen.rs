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
    readback: std::sync::Mutex<Option<wgpu::Buffer>>,
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
            readback: std::sync::Mutex::new(None),
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
        *self
            .readback
            .get_mut()
            .expect("readback lock is not poisoned") = None;
        self.size = size;
    }

    /// Reads the rendered image back as tightly packed 8-bit RGBA.
    pub fn read_image(&self) -> Vec<u8> {
        self.try_read_image()
            .expect("off-screen readback succeeded")
    }

    /// Reports failed transfers and discards their buffer so a later capture can retry.
    pub fn try_read_image(&self) -> Result<Vec<u8>, ReadbackError> {
        let mut readback = self.readback.lock().expect("readback lock is not poisoned");
        let result = read_rgba8_with_buffer(&self.gpu, &self.texture, self.size, &mut readback);
        if result.is_err() {
            *readback = None;
        }
        result
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
    read_rgba8_with_buffer(gpu, texture, size, &mut None).expect("off-screen readback succeeded")
}

#[derive(Debug, thiserror::Error)]
#[error("GPU readback failed: {0}")]
pub struct ReadbackError(pub String);

fn read_rgba8_with_buffer(
    gpu: &Gpu,
    texture: &wgpu::Texture,
    size: Size,
    readback: &mut Option<wgpu::Buffer>,
) -> Result<Vec<u8>, ReadbackError> {
    if size.is_empty() {
        return Err(ReadbackError("empty capture size".into()));
    }
    let unpadded_row = size.width as usize * 4;
    let alignment = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize;
    let padded_row = unpadded_row.div_ceil(alignment) * alignment;
    let buffer_size = (padded_row * size.height as usize) as u64;
    if buffer_size > gpu.device.limits().max_buffer_size {
        return Err(ReadbackError("capture exceeds the GPU buffer limit".into()));
    }
    checked_transfer(gpu, || {
        let buffer = readback.get_or_insert_with(|| {
            gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("media-readback"),
                size: buffer_size,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            })
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
                buffer,
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
    })?;

    let buffer = readback
        .as_ref()
        .expect("checked transfer created the buffer");
    let slice = buffer.slice(..);
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    checked_transfer(gpu, || {
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
    })?;
    if let Err(error) = gpu.device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: Some(std::time::Duration::from_secs(5)),
    }) {
        buffer.unmap();
        return Err(ReadbackError(error.to_string()));
    }
    let result = receiver
        .try_recv()
        .map_err(|error| ReadbackError(error.to_string()))
        .and_then(|result| result.map_err(|error| ReadbackError(error.to_string())));
    if let Err(error) = result {
        buffer.unmap();
        return Err(error);
    }

    let mapped = slice
        .get_mapped_range()
        .map_err(|error| ReadbackError(error.to_string()))?;
    let mut pixels = Vec::with_capacity(unpadded_row * size.height as usize);
    for row in 0..size.height as usize {
        let start = row * padded_row;
        pixels.extend_from_slice(&mapped[start..start + unpadded_row]);
    }
    drop(mapped);
    buffer.unmap();
    Ok(pixels)
}

fn checked_transfer<T>(gpu: &Gpu, transfer: impl FnOnce() -> T) -> Result<T, ReadbackError> {
    let validation = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let internal = gpu.device.push_error_scope(wgpu::ErrorFilter::Internal);
    let memory = gpu.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
    let result = transfer();
    let errors = [
        pollster::block_on(memory.pop()),
        pollster::block_on(internal.pop()),
        pollster::block_on(validation.pop()),
    ];
    if let Some(error) = errors.into_iter().flatten().next() {
        return Err(ReadbackError(error.to_string()));
    }
    Ok(result)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_readback_is_reported_and_replaced_on_next_capture() {
        let gpu = Gpu::off_screen().expect("test GPU");
        let output = OffScreenOutput::new(&gpu, Size::new(3, 2));
        let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let invalid = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("media-readback"),
            size: 512,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });
        assert!(pollster::block_on(scope.pop()).is_some());
        *output.readback.lock().unwrap() = Some(invalid);
        assert!(output.try_read_image().is_err());
        assert!(output.readback.lock().unwrap().is_none());
        assert_eq!(output.try_read_image().unwrap().len(), 3 * 2 * 4);
    }

    #[test]
    fn resizing_and_repeated_padded_readbacks_work() {
        let gpu = Gpu::off_screen().expect("test GPU");
        let mut output = OffScreenOutput::new(&gpu, Size::new(3, 2));
        for size in [Size::new(3, 2), Size::new(320, 180), Size::new(5, 3)] {
            output.resize(size);
            for _ in 0..3 {
                assert_eq!(
                    output.try_read_image().unwrap().len(),
                    size.width as usize * size.height as usize * 4
                );
            }
        }
    }
}
