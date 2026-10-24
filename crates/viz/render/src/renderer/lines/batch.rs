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
use std::collections::HashMap;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct WireInstance {
    model: [[f32; 4]; 4],
    normal0: [f32; 4],
    normal1: [f32; 4],
    normal2: [f32; 4],
}
impl From<&MeshInstance> for WireInstance {
    fn from(value: &MeshInstance) -> Self {
        Self {
            model: value.model,
            normal0: value.normal0,
            normal1: value.normal1,
            normal2: value.normal2,
        }
    }
}

#[derive(Default)]
pub(super) struct Batch {
    layout: Vec<(MeshKind, usize)>,
    chunks: Vec<Chunk>,
    instances: u32,
}
struct Chunk {
    _static_buffers: [wgpu::Buffer; 5],
    instances: DynamicBuffer,
    instance_bytes: Vec<u8>,
    links: Vec<(usize, usize)>,
    group: wgpu::BindGroup,
    depth_vertices: u32,
    edge_vertices: u32,
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
                .map(|plan| Chunk::new(device, queue, pipelines, drawn, plan))
                .collect();
            self.chunks = chunks;
            self.instances = instances;
            self.layout = layout;
        } else if instances_changed {
            for chunk in &mut self.chunks {
                chunk.update(device, queue, drawn);
            }
        }
        Ok(())
    }
    pub(super) fn draw_depth(&self, pass: &mut wgpu::RenderPass<'_>) -> u32 {
        let mut draws = 0;
        for chunk in &self.chunks {
            if chunk.depth_vertices > 0 {
                pass.set_bind_group(1, &chunk.group, &[]);
                pass.draw(0..chunk.depth_vertices, 0..1);
                draws += 1;
            }
        }
        draws
    }
    pub(super) fn draw_edges(&self, pass: &mut wgpu::RenderPass<'_>) -> u32 {
        let mut draws = 0;
        for chunk in &self.chunks {
            if chunk.edge_vertices > 0 {
                pass.set_bind_group(1, &chunk.group, &[]);
                pass.draw(0..chunk.edge_vertices, 0..1);
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
            storage(
                device,
                "wireframe batch indices",
                bytemuck::cast_slice(&plan.indices),
            ),
            storage(
                device,
                "wireframe batch triangle references",
                bytemuck::cast_slice(&plan.triangles),
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
            depth_vertices: plan.triangles.len() as u32 * 3,
            edge_vertices: plan.edge_refs.len() as u32 * 6,
        };
        chunk.update(device, queue, drawn);
        chunk
    }
    fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        drawn: &[(MeshKind, Vec<MeshInstance>)],
    ) {
        let transforms: Vec<_> = self
            .links
            .iter()
            .map(|&(kind, instance)| WireInstance::from(&drawn[kind].1[instance]))
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
