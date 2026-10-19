//! Execute the production haze interval helpers on the GPU and compare every sample with
//! production point occlusion. This covers the haze path without opening a renderer window.
use super::*;
use crate::instances::{GpuCurtainBlocker, GpuCurtainShadow, GpuLight};
use glam::{Mat4, Quat, Vec3};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Case {
    origin: [f32; 4],
    direction: [f32; 4],
    samples: [f32; 4],
    control: [f32; 4],
}

fn source() -> String {
    let light = COMMON_WGSL
        .split("struct Light {")
        .nth(1)
        .unwrap()
        .split("@group(2)")
        .next()
        .unwrap();
    let curtains = COMMON_WGSL
        .split("struct CurtainBlocker {")
        .nth(1)
        .unwrap()
        .split("/// How much of")
        .next()
        .unwrap();
    let helpers = BEAM_WGSL
        .split("fn step_prefix(")
        .nth(1)
        .unwrap()
        .split("@fragment")
        .next()
        .unwrap();
    format!(
        r#"
struct Light {{{light}
struct Globals {{ params4: vec4<f32>, }};
var<private> globals: Globals;
struct CurtainBlocker {{{curtains}
fn step_prefix({helpers}
struct Case {{ origin: vec4<f32>, direction: vec4<f32>, samples: vec4<f32>, control: vec4<f32>, }};
@group(0) @binding(1) var<storage,read> cases: array<Case>;
@group(0) @binding(2) var<storage,read_write> results: array<vec4<u32>>;
@compute @workgroup_size(1)
fn test_main(@builtin(global_invocation_id) invocation: vec3<u32>) {{
    let item = cases[invocation.x];
    if (item.control.y > 0.5) {{
        let mask = step_range(u32(item.samples.x), u32(item.samples.y));
        results[invocation.x] = vec4<u32>(mask, 0u, 0u);
        return;
    }}
    globals.params4.w = 1.0;
    var light: Light;
    light.gate.w = item.control.x;
    let steps = u32(item.samples.z);
    let mask = curtain_blocked_steps(light, item.origin.xyz, item.direction.xyz,
        item.samples.x, item.samples.y, steps);
    var reference = vec2<u32>(0u);
    for (var index = 0u; index < steps; index += 1u) {{
        let position = item.origin.xyz + item.direction.xyz
            * (item.samples.x + f32(index) * item.samples.y);
        if (curtain_blocks_segment(light, position)) {{
            if (index < 32u) {{ reference.x |= 1u << index; }}
            else {{ reference.y |= 1u << (index - 32u); }}
        }}
    }}
    results[invocation.x] = vec4<u32>(mask, reference);
}}
"#
    )
    .replace("@group(2) @binding(5)", "@group(0) @binding(0)")
}

fn rectangle(centre: Vec3, rotation: Quat) -> GpuCurtainBlocker {
    GpuCurtainBlocker {
        world_to_local: Mat4::from_rotation_translation(rotation, centre)
            .inverse()
            .to_cols_array_2d(),
        half_size: [1.0, 2.0, 0.0, 0.0],
    }
}

#[test]
fn native_gpu_curtain_interval_masks_match_point_occlusion() {
    let Ok(gpu) = Gpu::headless(1, 1) else {
        eprintln!("No native GPU available; curtain mask compute test skipped");
        return;
    };
    let mut records: Vec<GpuCurtainShadow> = Vec::new();
    let mut cases = Vec::new();
    let mut names = Vec::new();
    // Every possible half-open 64-bit range proves bit 0/31/32/63 and empty/full ranges.
    for first in 0..=64u32 {
        for end in first..=64 {
            cases.push(Case {
                origin: [0.0; 4],
                direction: [0.0; 4],
                samples: [first as f32, end as f32, 0.0, 0.0],
                control: [0.0, 1.0, 0.0, 0.0],
            });
            names.push(format!("range {first}..{end}"));
        }
    }
    let range_count = cases.len();
    for rotated in [false, true] {
        let rotation = if rotated {
            Quat::from_euler(glam::EulerRot::XYZ, 0.3, 0.8, -0.2)
        } else {
            Quat::IDENTITY
        };
        for side in [-1.0, 1.0] {
            let mut light = GpuLight::zeroed();
            light.position_range = [
                (rotation * Vec3::new(0.0, 0.0, 2.0 * side)).x,
                (rotation * Vec3::new(0.0, 0.0, 2.0 * side)).y,
                (rotation * Vec3::new(0.0, 0.0, 2.0 * side)).z,
                1000.0,
            ];
            // A hemispherical field retains every candidate, including more than64 curtains.
            light.direction_cos_outer = [0.0, 0.0, 1.0, 0.0];
            for geometry in [0, 1, 2] {
                let mut blockers = vec![rectangle(Vec3::ZERO, rotation)];
                if geometry == 1 {
                    blockers.push(rectangle(rotation * Vec3::new(0.5, 0.0, -0.3), rotation));
                }
                if geometry == 2 {
                    blockers = (0..70)
                        .map(|i| {
                            rectangle(rotation * Vec3::new(100.0 + i as f32, 0.0, 0.0), rotation)
                        })
                        .collect();
                    blockers.push(rectangle(Vec3::ZERO, rotation));
                }
                let mut lights = [light];
                let packed = super::curtain_occlusion::pack(&mut lights, &blockers);
                let gate = records.len() as f32 + lights[0].gate[3];
                records.extend(packed);
                for steps in [12, 24, 40, 64] {
                    for (name, origin, direction) in [
                        (
                            "crossing",
                            Vec3::new(0.2, 0.1, 3.0 * side),
                            Vec3::new(0.0, 0.0, -side),
                        ),
                        (
                            "opening",
                            Vec3::new(8.0, 0.1, 3.0 * side),
                            Vec3::new(0.0, 0.0, -side),
                        ),
                        (
                            "inside",
                            Vec3::new(0.1, 0.1, -2.0 * side),
                            Vec3::new(0.05, 0.0, -side).normalize(),
                        ),
                        (
                            "grazing",
                            Vec3::new(0.99, 0.1, 3.0 * side),
                            Vec3::new(0.03, 0.0, -side).normalize(),
                        ),
                        (
                            "leaving",
                            Vec3::new(0.0, 0.1, -2.0 * side),
                            Vec3::new(1.0, 0.0, 0.2 * side).normalize(),
                        ),
                    ] {
                        let origin = rotation * origin;
                        let direction = rotation * direction;
                        cases.push(Case {
                            origin: origin.extend(0.0).to_array(),
                            direction: direction.extend(0.0).to_array(),
                            samples: [0.013, 0.137, steps as f32, 0.0],
                            control: [gate, 0.0, 0.0, 0.0],
                        });
                        names.push(format!(
                            "{name} rotated={rotated} side={side} geometry={geometry} steps={steps}"
                        ));
                    }
                }
            }
        }
    }
    // A nearly parallel view ray crosses cloth after ten metres. Treating every slope
    // below 1e-8 as parallel would incorrectly mark every sample visible. Both lamp sides
    // exercise the cloth plane orientation; the very broad cloth keeps the tangent inside it.
    for sign in [-1.0, 1.0] {
        let mut blocker = rectangle(Vec3::ZERO, Quat::IDENTITY);
        blocker.half_size[0] = 100.0;
        let mut light = GpuLight::zeroed();
        light.position_range = [0.0, 0.0, -2.0 * sign, 1000.0];
        light.direction_cos_outer = [0.0, 0.0, 1.0, 0.0];
        let mut lights = [light];
        let packed = super::curtain_occlusion::pack(&mut lights, &[blocker]);
        let gate = records.len() as f32 + lights[0].gate[3];
        records.extend(packed);
        for steps in [12, 24, 40, 64] {
            cases.push(Case {
                origin: [0.0, 0.0, -1e-8 * sign, 0.0],
                direction: [1.0, 0.0, 1e-9 * sign, 0.0],
                samples: [0.013, 20.0 / (steps - 1) as f32, steps as f32, 0.0],
                control: [gate, 0.0, 0.0, 0.0],
            });
            names.push(format!("near-parallel sign={sign} steps={steps}"));
        }
    }
    let device = &gpu.device;
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("production curtain mask helpers"),
        source: wgpu::ShaderSource::Wgsl(source().into()),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("curtain mask test"),
        layout: None,
        module: &module,
        entry_point: Some("test_main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let storage = |label, contents| {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents,
            usage: wgpu::BufferUsages::STORAGE,
        })
    };
    let blockers = storage("test curtain volumes", bytemuck::cast_slice(&records));
    let inputs = storage("test curtain rays", bytemuck::cast_slice(&cases));
    let bytes = cases.len() as u64 * 16;
    let output = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("test masks"),
        size: bytes,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("test masks readback"),
        size: bytes,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("curtain mask test data"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: blockers.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: inputs.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: output.as_entire_binding(),
            },
        ],
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.dispatch_workgroups(cases.len() as u32, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&output, 0, &readback, 0, bytes);
    gpu.queue.submit(Some(encoder.finish()));
    let slice = readback.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        })
        .unwrap();
    let mapped = slice.get_mapped_range().unwrap();
    let results: &[[u32; 4]] = bytemuck::cast_slice(&mapped);
    for (index, actual) in results.iter().enumerate() {
        if index < range_count {
            let first = cases[index].samples[0] as u32;
            let end = cases[index].samples[1] as u32;
            let expected = (first..end).fold(0u64, |mask, bit| mask | (1u64 << bit));
            assert_eq!(
                [actual[0], actual[1]],
                [expected as u32, (expected >> 32) as u32],
                "{}",
                names[index]
            );
        } else {
            assert_eq!(
                [actual[0], actual[1]],
                [actual[2], actual[3]],
                "{}",
                names[index]
            );
            if names[index].starts_with("near-parallel") {
                assert_ne!([actual[0], actual[1]], [0, 0], "{}", names[index]);
                // The camera starts in front of the cloth before crossing it; bit zero stays lit.
                assert_eq!(actual[0] & 1, 0, "{}", names[index]);
            }
            if names[index].starts_with("opening") {
                assert_eq!([actual[0], actual[1]], [0, 0], "{}", names[index]);
            }
            if names[index].starts_with("inside") {
                assert_ne!([actual[0], actual[1]], [0, 0], "{}", names[index]);
            }
        }
    }
}
