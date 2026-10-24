//! Portable bounded geometry batching. Only transforms change between layout updates.
mod plan;
#[cfg(test)]
mod tests;

use super::{buffers, geometry::Geometry, pipelines::Pipelines};
use crate::{
    buffers::DynamicBuffer,
    instances::{MeshInstance, MeshKind},
    renderer::RenderError,
};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use std::collections::HashMap;
use wgpu::util::DeviceExt;

/// Only projection and eye affect instance records; ink and physical line width stay uniform.
#[derive(Clone, Copy, PartialEq)]
pub(super) struct CameraState {
    pub clip_from_world: Mat4,
    pub eye_world: Vec3,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct WireInstance {
    clip_from_local: [[f32; 4]; 4],
    /// w=0 retains every silhouette candidate when a singular inverse cannot classify it.
    local_eye: [f32; 4],
}
impl WireInstance {
    fn new(value: &MeshInstance, camera: CameraState) -> Self {
        let model = Mat4::from_cols_array_2d(&value.model);
        let determinant = model.determinant();
        // The exact-zero guard also permits glam's checked builds to handle collapsed models.
        let local_eye = if model.is_finite() && determinant.is_finite() && determinant != 0.0 {
            let inverse = model.inverse();
            let eye = inverse.transform_point3(camera.eye_world);
            if inverse.is_finite() && eye.is_finite() {
                eye.extend(1.0).to_array()
            } else {
                [0.0; 4]
            }
        } else {
            [0.0; 4]
        };
        Self {
            clip_from_local: (camera.clip_from_world * model).to_cols_array_2d(),
            local_eye,
        }
    }
}

fn transforms_changed(previous: Option<CameraState>, camera: CameraState, instances: bool) -> bool {
    instances || previous != Some(camera)
}

#[derive(Default)]
pub(super) struct Batch {
    layout: Vec<(MeshKind, usize)>,
    chunks: Vec<Chunk>,
    instances: u32,
    camera: Option<CameraState>,
}
struct Chunk {
    _static_buffers: [wgpu::Buffer; 5],
    instances: DynamicBuffer,
    instance_bytes: Vec<u8>,
    links: Vec<(usize, usize)>,
    group: wgpu::BindGroup,
    depth_indices: u32,
    edge_indices: wgpu::Buffer,
    edge_index_count: u32,
}

impl Batch {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pipelines: &Pipelines,
        geometry: &HashMap<MeshKind, Geometry>,
        drawn: &[(MeshKind, Vec<MeshInstance>)],
        assets_changed: bool,
        instances_changed: bool,
        camera: CameraState,
    ) -> Result<(), RenderError> {
        if device.limits().max_storage_buffers_per_shader_stage < 6 {
            return Err(failure(
                "the GPU exposes fewer than six vertex storage bindings",
            ));
        }
        let layout: Vec<_> = drawn
            .iter()
            .map(|(kind, instances)| (*kind, instances.len()))
            .collect();
        if assets_changed || layout != self.layout {
            let limit = device
                .limits()
                .max_storage_buffer_binding_size
                .min(device.limits().max_buffer_size);
            let plans = plan::build(
                drawn,
                |kind| {
                    geometry.get(&kind).map(|g| plan::Asset {
                        data: &g.data,
                        edges: &g.edges,
                    })
                },
                limit,
            )?;
            let instances = plans.iter().try_fold(0u32, |total, plan| {
                total
                    .checked_add(
                        u32::try_from(plan.links.len())
                            .map_err(|_| failure("instance count exceeds u32"))?,
                    )
                    .ok_or_else(|| failure("instance count exceeds u32"))
            })?;
            let chunks = plans
                .into_iter()
                .map(|plan| Chunk::new(device, queue, pipelines, drawn, plan, camera))
                .collect();
            self.chunks = chunks;
            self.instances = instances;
            self.layout = layout;
        } else if transforms_changed(self.camera, camera, instances_changed) {
            for chunk in &mut self.chunks {
                chunk.update(device, queue, drawn, camera);
            }
        }
        self.camera = Some(camera);
        Ok(())
    }
    pub(super) fn draw_depth(&self, pass: &mut wgpu::RenderPass<'_>) -> u32 {
        let mut draws = 0;
        for chunk in &self.chunks {
            if chunk.depth_indices > 0 {
                pass.set_bind_group(1, &chunk.group, &[]);
                pass.set_index_buffer(
                    chunk._static_buffers[2].slice(..),
                    wgpu::IndexFormat::Uint32,
                );
                pass.draw_indexed(0..chunk.depth_indices, 0, 0..1);
                draws += 1;
            }
        }
        draws
    }
    pub(super) fn draw_edges(&self, pass: &mut wgpu::RenderPass<'_>) -> u32 {
        let mut draws = 0;
        for chunk in &self.chunks {
            if chunk.edge_index_count > 0 {
                pass.set_bind_group(1, &chunk.group, &[]);
                pass.set_index_buffer(chunk.edge_indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..chunk.edge_index_count, 0, 0..1);
                draws += 1;
            }
        }
        draws
    }
    pub(super) fn instance_count(&self) -> u32 {
        self.instances
    }
}

impl Chunk {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pipelines: &Pipelines,
        drawn: &[(MeshKind, Vec<MeshInstance>)],
        plan: plan::ChunkPlan,
        camera: CameraState,
    ) -> Self {
        let static_buffers = [
            storage(
                device,
                "wireframe batch geometry edges",
                bytemuck::cast_slice(&plan.edges),
            ),
            storage(
                device,
                "wireframe batch positions",
                bytemuck::cast_slice(&plan.positions),
            ),
            depth_indices(device, bytemuck::cast_slice(&plan.indices)),
            storage(
                device,
                "wireframe batch virtual vertices",
                bytemuck::cast_slice(&plan.vertices),
            ),
            storage(
                device,
                "wireframe batch edge references",
                bytemuck::cast_slice(&plan.edge_refs),
            ),
        ];
        let instances = DynamicBuffer::new(
            device,
            "wireframe batch transforms",
            wgpu::BufferUsages::STORAGE,
            (plan.links.len() * size_of::<WireInstance>()) as u64,
        );
        let group = pipelines.batch_group(
            device,
            &[
                &static_buffers[0],
                &instances.buffer,
                &static_buffers[1],
                &static_buffers[2],
                &static_buffers[3],
                &static_buffers[4],
            ],
        );
        let mut chunk = Self {
            _static_buffers: static_buffers,
            instances,
            instance_bytes: Vec::new(),
            links: plan.links,
            group,
            depth_indices: plan.indices.len() as u32,
            edge_indices: edge_indices(device, bytemuck::cast_slice(&plan.edge_indices)),
            edge_index_count: plan.edge_indices.len() as u32,
        };
        chunk.update(device, queue, drawn, camera);
        chunk
    }
    fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        drawn: &[(MeshKind, Vec<MeshInstance>)],
        camera: CameraState,
    ) {
        let transforms: Vec<_> = self
            .links
            .iter()
            .map(|&(kind, instance)| WireInstance::new(&drawn[kind].1[instance], camera))
            .collect();
        // Layout never changes in this branch, so capacity/group identity stays fixed.
        buffers::upload_changed(
            device,
            queue,
            &mut self.instances,
            &mut self.instance_bytes,
            &transforms,
        );
    }
}
fn storage(device: &wgpu::Device, label: &str, bytes: &[u8]) -> wgpu::Buffer {
    let empty = [0u8; 16];
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: if bytes.is_empty() { &empty } else { bytes },
        usage: wgpu::BufferUsages::STORAGE,
    })
}
fn failure(detail: &str) -> RenderError {
    RenderError::Surface(format!("wireframe batching: {detail}"))
}

fn depth_indices(device: &wgpu::Device, bytes: &[u8]) -> wgpu::Buffer {
    let empty = [0u8; 16];
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("wireframe batch depth indices"),
        contents: if bytes.is_empty() { &empty } else { bytes },
        // Preserve the common six-binding batch layout; only the index assembler reads this
        // buffer. The depth shader reads the virtual-vertex map directly via vertex_index.
        usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::STORAGE,
    })
}

fn edge_indices(device: &wgpu::Device, bytes: &[u8]) -> wgpu::Buffer {
    let empty = [0u8; 16];
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("wireframe batch AA quad indices"),
        contents: if bytes.is_empty() { &empty } else { bytes },
        usage: wgpu::BufferUsages::INDEX,
    })
}
