//! Offscreen submission and asynchronous readback. Polling never waits for the GPU.
use super::{CapturedImage, RenderError, Renderer};
use std::sync::{Arc, mpsc};
use viz_scene::{Scene, SceneValues, ViewConfiguration};

/// One submitted capture. Keep it until readback completes; the caller progresses device callbacks
/// with `Device::poll(PollType::Poll)` or its normal rendering loop.
///
/// Dropping this handle does not wait: dropping its buffer cancels a pending map. wgpu retains
/// resources referenced by submitted GPU commands until execution completes.
pub struct PendingCapture {
    backing: Arc<Readback>,
    completion: Option<mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>>,
    width: u32,
    height: u32,
    padded: u32,
    bgra: bool,
}

struct Readback {
    _texture: wgpu::Texture,
    buffer: wgpu::Buffer,
}

impl PendingCapture {
    /// Return `None` while mapping is pending, or collect one RGBA image after completion.
    /// Call only until this returns an image or error; no GPU polling or waiting occurs here.
    pub fn try_complete(&mut self) -> Result<Option<CapturedImage>, RenderError> {
        if !mapping_ready(&mut self.completion)? {
            return Ok(None);
        }
        let mapped = self
            .backing
            .buffer
            .slice(..)
            .get_mapped_range()
            .map_err(|error| RenderError::Surface(format!("capture readback: {error}")))?;
        let rgba = unpack_rows(&mapped, self.width, self.height, self.padded, self.bgra)?;
        drop(mapped);
        self.backing.buffer.unmap();
        Ok(Some(CapturedImage {
            width: self.width,
            height: self.height,
            rgba,
        }))
    }
}

impl Renderer {
    /// Submit an offscreen frame and start its readback without waiting for GPU completion.
    /// The returned handle owns its output texture and staging buffer independently of resizes.
    pub fn begin_capture(
        &mut self,
        scene: &Scene,
        values: &SceneValues,
        view: &ViewConfiguration,
        overlay: &crate::overlay::Overlay,
        time_seconds: f32,
    ) -> Result<PendingCapture, RenderError> {
        let device = self.gpu.device.clone();
        let width = self.gpu.config.width;
        let height = self.gpu.config.height;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("viz capture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.gpu.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        self.capture_request = Some(texture.create_view(&wgpu::TextureViewDescriptor::default()));
        self.render(scene, values, view, overlay, time_seconds)?;
        let padded = (width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("viz capture readback"),
            size: u64::from(padded) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("viz capture copy"),
        });
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.gpu.queue.submit(Some(encoder.finish()));
        let backing = Arc::new(Readback {
            _texture: texture,
            buffer,
        });
        // Never capture backing in this callback: wgpu stores it in BufferMapState::Waiting;
        // retaining the public Buffer here would prevent CoreBuffer::drop from cancelling the map.
        // Core's buffer_drop unmaps/cancels without waiting, while ActiveSubmission retains GPU
        // resources used by submitted commands (wgpu-core device/global.rs and device/life.rs).
        let (completed, completion) = mpsc::channel();
        backing
            .buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = completed.send(result);
            });
        Ok(PendingCapture {
            backing,
            completion: Some(completion),
            width,
            height,
            padded,
            bgra: matches!(
                self.gpu.format,
                wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
            ),
        })
    }
}

fn mapping_ready(
    completion: &mut Option<mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>>,
) -> Result<bool, RenderError> {
    let Some(receiver) = completion.as_ref() else {
        return Err(RenderError::Surface(
            "capture readback already collected".to_owned(),
        ));
    };
    match receiver.try_recv() {
        Err(mpsc::TryRecvError::Empty) => Ok(false),
        result => {
            *completion = None;
            result
                .map_err(|_| {
                    RenderError::Surface("capture mapping callback disconnected".to_owned())
                })?
                .map_err(|error| RenderError::Surface(format!("capture mapping: {error}")))?;
            Ok(true)
        }
    }
}

fn unpack_rows(
    mapped: &[u8],
    width: u32,
    height: u32,
    padded: u32,
    bgra: bool,
) -> Result<Vec<u8>, RenderError> {
    let row_bytes = width as usize * 4;
    let stride = padded as usize;
    let length = row_bytes
        .checked_mul(height as usize)
        .ok_or_else(|| RenderError::Surface("capture image size overflow".to_owned()))?;
    if stride == 0 || stride < row_bytes || mapped.len() < stride.saturating_mul(height as usize) {
        return Err(RenderError::Surface(
            "capture readback has incomplete rows".to_owned(),
        ));
    }
    let mut pixels = Vec::with_capacity(length);
    for row in mapped.chunks_exact(stride).take(height as usize) {
        pixels.extend_from_slice(&row[..row_bytes]);
    }
    if bgra {
        for pixel in pixels.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
    }
    Ok(pixels)
}

#[cfg(test)]
mod tests {
    use super::{Arc, mapping_ready, unpack_rows};
    use std::sync::mpsc;

    #[test]
    fn pending_mapping_never_reports_ready_before_callback_or_twice() {
        let (completed, receiver) = mpsc::channel();
        let mut completion = Some(receiver);
        assert!(!mapping_ready(&mut completion).unwrap());
        completed.send(Ok(())).unwrap();
        assert!(mapping_ready(&mut completion).unwrap());
        assert!(mapping_ready(&mut completion).is_err());
    }

    #[test]
    fn mapping_failure_and_disconnection_never_allow_pixel_collection() {
        let (completed, receiver) = mpsc::channel();
        let mut completion = Some(receiver);
        completed.send(Err(wgpu::BufferAsyncError)).unwrap();
        assert!(mapping_ready(&mut completion).is_err());
        assert!(completion.is_none());
        let (completed, receiver) = mpsc::channel();
        let mut completion = Some(receiver);
        drop(completed);
        assert!(mapping_ready(&mut completion).is_err());
    }

    #[test]
    #[ignore = "requires a coordinated GPU test run"]
    fn dropping_pending_readback_cancels_map_without_device_poll() {
        use super::Renderer;
        use std::time::Duration;
        use viz_scene::{Scene, SceneValues, ViewConfiguration};
        let mut renderer = Renderer::headless(32, 24).unwrap();
        let pending = renderer
            .begin_capture(
                &Scene::default(),
                &SceneValues::default(),
                &ViewConfiguration::default(),
                &crate::overlay::Overlay::default(),
                0.0,
            )
            .unwrap();
        let super::PendingCapture {
            backing,
            completion,
            ..
        } = pending;
        let completion = completion.unwrap();
        let weak_backing = Arc::downgrade(&backing);
        let already_completed = completion.try_recv().ok();
        // Dropping the public buffer must synchronously abort its map. This fails if the map
        // callback captures backing and prevents CoreBuffer::drop, even with no further polls.
        drop(backing);
        assert!(
            weak_backing.upgrade().is_none(),
            "map callback must not retain public backing"
        );
        if already_completed.is_none() {
            // Native wgpu-core aborts synchronously; an already-finished map may report success.
            let _result = completion.recv_timeout(Duration::from_secs(1)).unwrap();
        }
        drop(renderer);
    }

    /// GPU integration coverage; run explicitly alongside the embedded ownership tests.
    #[test]
    #[ignore = "requires a coordinated GPU test run"]
    fn pending_capture_keeps_original_extent_across_resize_and_matches_sync_capture() {
        use super::Renderer;
        use std::time::{Duration, Instant};
        use viz_scene::{Scene, SceneValues, ViewConfiguration};
        let mut renderer = Renderer::headless(32, 24).unwrap();
        let scene = Scene::default();
        let values = SceneValues::default();
        let view = ViewConfiguration::default();
        let overlay = crate::overlay::Overlay::default();
        let mut pending = renderer
            .begin_capture(&scene, &values, &view, &overlay, 0.0)
            .unwrap();
        renderer.resize(64, 48);
        let deadline = Instant::now() + Duration::from_secs(10);
        let image = loop {
            renderer.device().poll(wgpu::PollType::Poll).unwrap();
            if let Some(image) = pending.try_complete().unwrap() {
                break image;
            }
            assert!(Instant::now() < deadline, "asynchronous readback timed out");
            std::thread::sleep(Duration::from_millis(1));
        };
        assert_eq!(
            (image.width, image.height, image.rgba.len()),
            (32, 24, 32 * 24 * 4)
        );
        renderer.resize(32, 24);
        let sync = renderer
            .capture(&scene, &values, &view, &overlay, 0.0)
            .unwrap();
        assert_eq!(image.rgba, sync.rgba);
    }

    #[test]
    fn readback_removes_each_padded_row_and_converts_bgra() {
        let rows = [
            3, 2, 1, 4, 7, 6, 5, 8, 99, 99, 99, 99, 11, 10, 9, 12, 15, 14, 13, 16, 99, 99, 99, 99,
        ];
        assert_eq!(
            unpack_rows(&rows, 2, 2, 12, true).unwrap(),
            (1..=16).collect::<Vec<_>>()
        );
    }

    #[test]
    fn rgba_readback_keeps_channel_order_and_rejects_partial_rows() {
        assert_eq!(
            unpack_rows(&[1, 2, 3, 4, 0, 0, 0, 0], 1, 1, 8, false).unwrap(),
            [1, 2, 3, 4]
        );
        assert!(unpack_rows(&[1, 2, 3, 4], 1, 2, 4, false).is_err());
        assert!(unpack_rows(&[1, 2, 3, 4], 2, 1, 4, false).is_err());
    }
}
