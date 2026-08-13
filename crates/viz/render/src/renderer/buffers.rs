//! The buffers one frame's values are uploaded into.
//!
//! Built in one place rather than inline where the renderer happens to need them first, so what a
//! frame actually carries to the GPU can be read as a list.

use super::{Globals, MAX_LIGHTS_PER_TILE};
use crate::buffers::DynamicBuffer;
use crate::instances::{
    BeamInstance, GpuLight, GpuMediaPanel, LaserInstance, LineVertex, MeshInstance, MeshKind,
};
use crate::targets::Targets;
use bytemuck::Zeroable;
use std::collections::HashMap;
use wgpu::BufferUsages;

/// Everything one frame's values are written into.
pub(super) struct SceneBuffers {
    pub globals: wgpu::Buffer,
    pub post_settings: wgpu::Buffer,
    pub lights: DynamicBuffer,
    pub tile_counts: DynamicBuffer,
    pub tile_lights: DynamicBuffer,
    /// One instance buffer per procedural mesh, plus the ones the beams, lasers, plan lines and
    /// overlay quads are drawn from.
    pub mesh_instances: HashMap<MeshKind, DynamicBuffer>,
    pub beam_instances: DynamicBuffer,
    pub laser_instances: DynamicBuffer,
    pub line_vertices: DynamicBuffer,
    pub overlay_quads: DynamicBuffer,
    pub media_panels: DynamicBuffer,
}

impl SceneBuffers {
    pub(super) fn new(device: &wgpu::Device, queue: &wgpu::Queue, targets: &Targets) -> Self {
        let globals_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("viz globals"),
            size: size_of::<Globals>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let post_settings = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("viz post settings"),
            size: 16,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut lights = DynamicBuffer::new(
            device,
            "viz lights",
            BufferUsages::STORAGE,
            (size_of::<GpuLight>() * 512) as u64,
        );
        lights.upload(device, queue, &[GpuLight::zeroed()]);
        let mut tile_counts = DynamicBuffer::new(
            device,
            "viz tile counts",
            BufferUsages::STORAGE,
            (targets.tile_count() * 4).max(256) as u64,
        );
        tile_counts.ensure(device, (targets.tile_count() * 4).max(256) as u64);
        let mut tile_lights = DynamicBuffer::new(
            device,
            "viz tile lights",
            BufferUsages::STORAGE,
            (targets.tile_count() * MAX_LIGHTS_PER_TILE * 4).max(256) as u64,
        );
        tile_lights.ensure(
            device,
            (targets.tile_count() * MAX_LIGHTS_PER_TILE * 4).max(256) as u64,
        );
        let mut mesh_instances = HashMap::new();
        for kind in MeshKind::PROCEDURAL {
            mesh_instances.insert(
                kind,
                DynamicBuffer::new(
                    device,
                    "viz mesh instances",
                    BufferUsages::VERTEX,
                    (size_of::<MeshInstance>() * 1024) as u64,
                ),
            );
        }
        Self {
            globals: globals_buffer,
            post_settings,
            lights,
            tile_counts,
            tile_lights,
            mesh_instances,
            beam_instances: DynamicBuffer::new(
                device,
                "viz beam instances",
                BufferUsages::VERTEX,
                (size_of::<BeamInstance>() * 1024) as u64,
            ),
            laser_instances: DynamicBuffer::new(
                device,
                "viz laser instances",
                BufferUsages::VERTEX,
                // A single laser can run to hundreds of segments a frame, so this starts far
                // larger than the beam buffer and still grows on demand.
                (size_of::<LaserInstance>() * 8192) as u64,
            ),
            line_vertices: DynamicBuffer::new(
                device,
                "viz line vertices",
                BufferUsages::VERTEX,
                (size_of::<LineVertex>() * 2048) as u64,
            ),
            overlay_quads: DynamicBuffer::new(
                device,
                "viz overlay quads",
                BufferUsages::VERTEX,
                (size_of::<crate::overlay::OverlayQuad>() * 4096) as u64,
            ),
            media_panels: DynamicBuffer::new(
                device,
                "viz media panels",
                BufferUsages::VERTEX,
                (size_of::<GpuMediaPanel>() * 1024) as u64,
            ),
        }
    }
}
