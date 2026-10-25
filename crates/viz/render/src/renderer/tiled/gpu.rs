//! Explicitly serialized native GPU comparison against the prior inline bounds calculation.
use super::super::*;
use crate::{
    gpu::Gpu,
    instances::GpuLight,
    renderer::{CULL_WGSL, Globals, build_scene_group, layouts::Layouts},
};
use bytemuck::Zeroable;
use glam::{Mat4, Vec3};
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};
use wgpu::util::DeviceExt;

struct Kernels {
    bounds: wgpu::ComputePipeline,
    cached: wgpu::ComputePipeline,
    inline: wgpu::ComputePipeline,
}
impl Kernels {
    fn new(device: &Device, layouts: &Layouts, draft: bool) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("exact cull comparison"),
            source: wgpu::ShaderSource::Wgsl(CULL_WGSL.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("cull parity layout"),
            bind_group_layouts: &[Some(&layouts.cull), Some(&layouts.bounds)],
            immediate_size: 0,
        });
        let make = |entry, precomputed: bool| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: Some(&layout),
                module: &module,
                entry_point: Some(entry),
                compilation_options: wgpu::PipelineCompilationOptions {
                    constants: &[
                        ("DRAFT_CONE_BOUNDS", if draft { 1.0 } else { 0.0 }),
                        ("PRECOMPUTED_BOUNDS", if precomputed { 1.0 } else { 0.0 }),
                    ],
                    ..Default::default()
                },
                cache: None,
            })
        };
        Self {
            bounds: make("bounds_main", true),
            cached: make("cull_main", true),
            inline: make("cull_main", false),
        }
    }
}
fn fixture(count: u32, wide: bool, offscreen: bool) -> Vec<GpuLight> {
    (0..count)
        .map(|index| {
            let mut light = GpuLight::zeroed();
            let x = if offscreen || (count == 65 && index % 3 == 0) {
                10_000.0
            } else if count == 65 && index % 3 == 2 {
                3.0
            } else {
                0.0
            };
            light.position_range = [x, 0.0, 0.0, 10.0];
            light.direction_cos_outer = [0.0, 0.0, -1.0, if wide { 0.2 } else { 0.95 }];
            light
        })
        .collect()
}
struct Inputs {
    group: wgpu::BindGroup,
    counts: DynamicBuffer,
    indices: DynamicBuffer,
    readback: wgpu::Buffer,
    tiles: (u32, u32),
    total: u32,
}
impl Inputs {
    fn new(
        gpu: &Gpu,
        layouts: &Layouts,
        lights: &[GpuLight],
        size: (u32, u32),
        camera: Vec3,
    ) -> Self {
        let device = &gpu.device;
        let tiles = (size.0.div_ceil(16), size.1.div_ceil(16));
        let total = tiles.0 * tiles.1;
        let mut globals = Globals::zeroed();
        globals.view_projection =
            (Mat4::perspective_rh(0.9, size.0 as f32 / size.1 as f32, 0.05, 1000.0)
                * Mat4::look_at_rh(camera, camera + Vec3::NEG_Z, Vec3::Y))
            .to_cols_array_2d();
        globals.screen = [size.0 as f32, size.1 as f32, 0.05, 1000.0];
        globals.params2 = [lights.len() as f32, 12.0, 0.0, tiles.0 as f32];
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cull parity globals"),
            contents: bytemuck::bytes_of(&globals),
            usage: BufferUsages::UNIFORM,
        });
        let mut light_buffer =
            DynamicBuffer::new(device, "cull parity lights", BufferUsages::STORAGE, 256);
        let dummy = [GpuLight::zeroed()];
        light_buffer.upload(
            device,
            &gpu.queue,
            if lights.is_empty() { &dummy } else { lights },
        );
        let counts = DynamicBuffer::new(
            device,
            "cull parity counts",
            BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            u64::from(total) * 4,
        );
        let indices = DynamicBuffer::new(
            device,
            "cull parity indices",
            BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            u64::from(total) * 256 * 4,
        );
        let group = build_scene_group(
            device,
            &layouts.cull,
            &uniform,
            &light_buffer,
            &counts,
            &indices,
        );
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cull parity readback"),
            size: u64::from(total) * 257 * 4,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Self {
            group,
            counts,
            indices,
            readback,
            tiles,
            total,
        }
    }
}
fn run(
    gpu: &Gpu,
    layouts: &Layouts,
    kernels: &Kernels,
    scratch: &mut ScreenBounds,
    lights: &[GpuLight],
    size: (u32, u32),
    camera: Vec3,
    precomputed: bool,
) -> Vec<u32> {
    run_with_timing(
        gpu,
        layouts,
        kernels,
        scratch,
        lights,
        size,
        camera,
        precomputed,
        None,
    )
    .0
}
#[allow(clippy::too_many_arguments)]
fn run_with_timing(
    gpu: &Gpu,
    layouts: &Layouts,
    kernels: &Kernels,
    scratch: &mut ScreenBounds,
    lights: &[GpuLight],
    size: (u32, u32),
    camera: Vec3,
    precomputed: bool,
    timing: Option<&timing::Recorder>,
) -> (Vec<u32>, Option<timing::Sample>) {
    let device = &gpu.device;
    let Inputs {
        group,
        counts,
        indices,
        readback,
        tiles,
        total,
    } = Inputs::new(gpu, layouts, lights, size, camera);
    scratch.ensure(device, lights.len() as u32);
    let mut encoder = device.create_command_encoder(&Default::default());
    if precomputed {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("diagnostic bounds"),
            timestamp_writes: timing.map(|t| t.writes(true, false)),
        });
        pass.set_pipeline(&kernels.bounds);
        pass.set_bind_group(0, &group, &[]);
        pass.set_bind_group(1, &scratch.group, &[]);
        pass.dispatch_workgroups((lights.len() as u32).div_ceil(64).max(1), 1, 1);
    }
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("diagnostic cull"),
            timestamp_writes: timing.map(|t| t.writes(!precomputed, true)),
        });
        pass.set_pipeline(if precomputed {
            &kernels.cached
        } else {
            &kernels.inline
        });
        pass.set_bind_group(0, &group, &[]);
        pass.set_bind_group(1, &scratch.group, &[]);
        pass.dispatch_workgroups(tiles.0, tiles.1, 1);
    }
    encoder.copy_buffer_to_buffer(&counts.buffer, 0, &readback, 0, u64::from(total) * 4);
    encoder.copy_buffer_to_buffer(
        &indices.buffer,
        0,
        &readback,
        u64::from(total) * 4,
        u64::from(total) * 256 * 4,
    );
    gpu.queue.submit(Some(encoder.finish()));
    // Mapping proves the measured submission finished. Metal queries are resolved afterward
    // in a second submission, exactly as the production timer requires.
    let membership = mapped_u32(gpu, &readback);
    (membership, timing.map(|t| t.completed_sample(gpu)))
}
fn mapped_u32(gpu: &Gpu, readback: &wgpu::Buffer) -> Vec<u32> {
    let device = &gpu.device;
    let slice = readback.slice(..);
    let (send, receive) = mpsc::sync_channel(1);
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = send.send(result);
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        device.poll(wgpu::PollType::Poll).unwrap();
        if let Ok(result) = receive.try_recv() {
            result.unwrap();
            break;
        }
        assert!(
            Instant::now() < deadline,
            "bounded native cull readback timed out"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    let mapped = slice.get_mapped_range().unwrap();
    let result = bytemuck::cast_slice::<u8, u32>(&mapped).to_vec();
    drop(mapped);
    readback.unmap();
    result
}
fn assert_membership(a: &[u32], b: &[u32], count: usize) {
    let tiles = a.len() / 257;
    assert_eq!(a.len(), b.len());
    assert_eq!(
        &a[..tiles],
        &b[..tiles],
        "all tile counts, including overflow, agree exactly"
    );
    for tile in 0..tiles {
        let n = a[tile] as usize;
        assert!(n <= count);
        if n > 256 {
            continue;
        } // Both surface paths deliberately fall back to ALL lights.
        let offset = tiles + tile * 256;
        let mut left = a[offset..offset + n].to_vec();
        let mut right = b[offset..offset + n].to_vec();
        left.sort_unstable();
        right.sort_unstable();
        assert_eq!(left, right, "tile {tile} membership differs");
    }
}
#[test]
#[ignore = "requires explicitly serialized native GPU acceptance"]
fn cached_bounds_match_inline_across_quality_camera_plane_offscreen_empty_overflow_and_resize() {
    let gpu = Gpu::headless(32, 32).expect("native GPU required");
    eprintln!(
        "TL874 native cull parity adapter: {} [{}], timestamps={}",
        gpu.adapter_name, gpu.backend, gpu.timestamps
    );
    let layouts = Layouts::new(&gpu.device, 1);
    let mut scratch = ScreenBounds::new(&gpu.device, &layouts.bounds);
    for draft in [false, true] {
        let kernels = Kernels::new(&gpu.device, &layouts, draft);
        for (count, wide, offscreen, size, camera) in [
            (0, false, false, (17, 19), Vec3::new(0.0, 0.0, 4.0)),
            (1, false, true, (65, 33), Vec3::new(0.0, 0.0, 4.0)),
            (65, false, false, (513, 289), Vec3::new(0.0, 0.0, 4.0)),
            (300, true, false, (65, 33), Vec3::ZERO),
            (1, false, false, (19, 17), Vec3::new(0.0, 0.0, 2.0)),
            (0, false, false, (33, 65), Vec3::ZERO),
        ] {
            let lights = fixture(count, wide, offscreen);
            let cached = run(
                &gpu,
                &layouts,
                &kernels,
                &mut scratch,
                &lights,
                size,
                camera,
                true,
            );
            let inline = run(
                &gpu,
                &layouts,
                &kernels,
                &mut scratch,
                &lights,
                size,
                camera,
                false,
            );
            assert_membership(&cached, &inline, count as usize);
            let tiles = cached.len() / 257;
            if count == 0 || offscreen {
                assert!(cached[..tiles].iter().all(|&n| n == 0));
            }
            if count == 300 {
                assert!(
                    cached[..tiles].iter().all(|&n| n == 300),
                    "camera-plane overflow remains complete"
                );
            }
            if count == 65 {
                assert!(cached[..tiles].iter().any(|&n| n > 0));
            }
        }
    }
}

#[path = "gpu/timing.rs"]
mod timing;
