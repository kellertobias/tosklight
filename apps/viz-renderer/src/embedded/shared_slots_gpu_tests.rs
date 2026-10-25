//! Opt-in shared-picture ownership proof; queue completion is not displayed-frame evidence.
use super::producer::Producer;
use std::{
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};
use viz_helper::protocol::{FrameId, SHARED_FRAME_SLOTS};
use viz_render::{Overlay, Renderer};
use viz_scene::{Aabb, RenderQuality, Scene, SceneValues, ViewConfiguration, ViewMode};
use viz_surface::SharedSurface;

const PERIOD: Duration = Duration::from_nanos(16_666_667);
const DEADLINE: Duration = Duration::from_secs(10);
type Picture = (FrameId, Arc<SharedSurface>);

fn enqueue(
    renderer: &mut Renderer,
    producer: &mut Producer,
    frame: FrameId,
    surface: Arc<SharedSurface>,
    colour: [f32; 3],
    completed: &mpsc::Sender<Picture>,
) {
    let scene = Scene {
        bounds: Aabb {
            min: glam::Vec3::splat(-1.0),
            max: glam::Vec3::splat(1.0),
        },
        ..Default::default()
    };
    let view = ViewConfiguration {
        mode: ViewMode::Full3d,
        quality: RenderQuality::High,
        ambient: 0.0,
        floor_grid: false,
        show_labels: false,
        background: Some(colour),
        ..Default::default()
    };
    let stats = renderer
        .render_into(
            surface.view(),
            &scene,
            &SceneValues::default(),
            &view,
            &Overlay::default(),
            0.0,
        )
        .expect("shared render must submit");
    // Sampling is optional on native adapters; check provenance shape without claiming that
    // the three submissions deterministically saturate the timestamp/readback state machine.
    assert_eq!(stats.gpu_sample_id.is_some(), stats.gpu_micros.is_some());
    producer.submitted(frame);
    let completed = completed.clone();
    renderer.queue().on_submitted_work_done(move || {
        // Move the exact backing through completion, as production draw_shared does.
        let _ = completed.send((frame, surface));
    });
}

fn finish(
    renderer: &Renderer,
    producer: &mut Producer,
    completed: &mpsc::Receiver<Picture>,
    expected: &[FrameId],
) -> Vec<Picture> {
    let deadline = Instant::now() + DEADLINE;
    let mut pictures = Vec::new();
    while pictures.len() < expected.len() {
        renderer.device().poll(wgpu::PollType::Poll).unwrap();
        while let Ok(picture) = completed.try_recv() {
            assert!(
                expected.contains(&picture.0),
                "unexpected completion identity"
            );
            assert!(producer.completed(picture.0), "duplicate/stale completion");
            pictures.push(picture);
        }
        assert!(
            Instant::now() < deadline,
            "GPU completion exceeded 10 seconds"
        );
        if pictures.len() < expected.len() {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    let mut actual: Vec<_> = pictures.iter().map(|p| p.0).collect();
    let mut expected = expected.to_vec();
    actual.sort_by_key(|frame| frame.sequence);
    expected.sort_by_key(|frame| frame.sequence);
    assert_eq!(actual, expected);
    pictures
}

fn centre_pixels(renderer: &Renderer, surfaces: &[Arc<SharedSurface>]) -> Vec<[u8; 4]> {
    let mut encoder = renderer
        .device()
        .create_command_encoder(&Default::default());
    let buffers: Vec<_> = surfaces
        .iter()
        .map(|surface| {
            let buffer = renderer.device().create_buffer(&wgpu::BufferDescriptor {
                label: Some("shared ownership centre pixel"),
                size: 256,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let (width, height) = surface.size();
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: surface.texture(),
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: width / 2,
                        y: height / 2,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(256),
                        rows_per_image: Some(1),
                    },
                },
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );
            buffer
        })
        .collect();
    renderer.queue().submit([encoder.finish()]);
    let (send, receive) = mpsc::channel();
    for (index, buffer) in buffers.iter().enumerate() {
        let send = send.clone();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = send.send((index, result));
            });
    }
    let deadline = Instant::now() + DEADLINE;
    let mut mapped = vec![false; buffers.len()];
    while mapped.iter().any(|ready| !ready) {
        renderer.device().poll(wgpu::PollType::Poll).unwrap();
        while let Ok((index, result)) = receive.try_recv() {
            result.expect("pixel readback must map");
            assert!(!mapped[index]);
            mapped[index] = true;
        }
        assert!(
            Instant::now() < deadline,
            "pixel readback exceeded 10 seconds"
        );
        if mapped.iter().any(|ready| !ready) {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    buffers
        .iter()
        .map(|buffer| {
            let bytes = buffer
                .slice(..)
                .get_mapped_range()
                .expect("mapped readback range");
            let pixel = bytes[..4].try_into().unwrap();
            drop(bytes);
            buffer.unmap();
            pixel
        })
        .collect()
}

fn assert_colour(pixel: [u8; 4], channels: [bool; 3]) {
    for (value, enabled) in pixel[..3].iter().zip(channels) {
        assert!(
            if enabled { *value > 100 } else { *value < 8 },
            "wrong retained pixel: {pixel:?}"
        );
    }
    assert_eq!(pixel[3], 255);
}

fn assert_replacement_preserves_old_pictures(
    renderer: &mut Renderer,
    producer: &mut Producer,
    surfaces: &[Arc<SharedSurface>],
    stale: FrameId,
    now: Instant,
    send: &mpsc::Sender<Picture>,
    receive: &mpsc::Receiver<Picture>,
) {
    let retained_pixels = centre_pixels(renderer, surfaces);
    assert!(producer.resize(now).unwrap());
    renderer.resize(80, 56);
    let replacement = Arc::new(viz_surface::create(renderer.device(), 80, 56).unwrap());
    let current = producer.begin(now).unwrap();
    assert_ne!(current.generation, stale.generation);
    enqueue(
        renderer,
        producer,
        current,
        replacement.clone(),
        [0.0, 1.0, 1.0],
        send,
    );
    assert!(
        !producer.release(stale),
        "old generation cannot free new reservation"
    );
    let newest = finish(renderer, producer, receive, &[current]);
    assert_eq!(centre_pixels(renderer, surfaces), retained_pixels);
    assert_colour(
        centre_pixels(renderer, &[replacement])[0],
        [false, true, true],
    );
    assert!(producer.release(current));
    drop(newest);
}

#[test]
#[ignore = "requires explicitly serialized native GPU acceptance"]
fn three_shared_pictures_survive_renderer_reuse_and_generation_replacement() {
    assert_eq!(SHARED_FRAME_SLOTS, 3);
    let mut renderer = Renderer::headless(64, 48).expect("native GPU adapter required");
    let origin = Instant::now();
    let mut producer = Producer::new(true, origin);
    let surfaces: Vec<_> = (0..3)
        .map(|_| {
            Arc::new(
                viz_surface::create(renderer.device(), 64, 48)
                    .expect("native shared surface required"),
            )
        })
        .collect();
    let weak: Vec<_> = surfaces.iter().map(Arc::downgrade).collect();
    let (send, receive) = mpsc::channel();
    let mut frames = Vec::new();
    // No explicit poll or wait between these submissions. Renderer::render internally polls
    // nonblocking at entry; this public-API test does not claim to suppress that internal poll.
    for (index, colour) in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
        .into_iter()
        .enumerate()
    {
        let frame = producer.begin(origin + PERIOD * index as u32).unwrap();
        assert_eq!(frame.slot as usize, index);
        enqueue(
            &mut renderer,
            &mut producer,
            frame,
            surfaces[index].clone(),
            colour,
            &send,
        );
        frames.push(frame);
    }
    assert!(!producer.available(origin + PERIOD * 3));
    assert!(!producer.resize(origin + PERIOD * 3).unwrap());
    let first = finish(&renderer, &mut producer, &receive, &frames);
    let pixels = centre_pixels(&renderer, &surfaces);
    for (pixel, channels) in pixels.into_iter().zip([
        [true, false, false],
        [false, true, false],
        [false, false, true],
    ]) {
        assert_colour(pixel, channels);
    }
    assert!(!producer.available(origin + PERIOD * 3));
    assert!(!producer.release(FrameId {
        sequence: 999,
        ..frames[1]
    }));
    assert!(producer.release(frames[1]));
    let yellow = producer.begin(origin + PERIOD * 3).unwrap();
    assert_eq!(yellow.slot, frames[1].slot);
    enqueue(
        &mut renderer,
        &mut producer,
        yellow,
        surfaces[1].clone(),
        [1.0, 1.0, 0.0],
        &send,
    );
    let reused = finish(&renderer, &mut producer, &receive, &[yellow]);
    assert!(
        !producer.release(frames[1]),
        "old sequence cannot free reused slot"
    );
    assert!(!producer.available(origin + PERIOD * 4));
    let retained_pixels = centre_pixels(&renderer, &surfaces);
    assert_colour(retained_pixels[0], [true, false, false]);
    assert_colour(retained_pixels[1], [true, true, false]);
    assert_colour(retained_pixels[2], [false, false, true]);
    assert_replacement_preserves_old_pictures(
        &mut renderer,
        &mut producer,
        &surfaces,
        frames[0],
        origin + PERIOD * 4,
        &send,
        &receive,
    );
    assert!(weak.iter().all(|backing| backing.upgrade().is_some()));
    drop((first, reused, surfaces));
    assert!(
        weak.iter().all(|backing| backing.upgrade().is_none()),
        "old backing ownership must end"
    );
}
