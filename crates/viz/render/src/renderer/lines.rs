//! Full-resolution, single-sample technical edges. No light/material/post-processing resources.
mod buffers;
mod geometry;
mod pipelines;
#[cfg(test)]
mod tests;

use super::{FrameStats, context::FrameContext};
use crate::{
    buffers::DynamicBuffer,
    camera::ResolvedCamera,
    instances::{MeshKind, TechnicalLinesInstances},
};
use bytemuck::{Pod, Zeroable};
use std::collections::HashMap;
use viz_scene::{Scene, SceneValues, ViewConfiguration};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CameraUniform {
    view_projection: [[f32; 4]; 4],
    eye: [f32; 4],
    screen: [f32; 4],
    ink: [f32; 4],
}

/// One retained adjacency edge, or one coloured guide segment. Shared storage layout; the two
/// entry points give the final two vectors their appropriate normals/colours meaning.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Segment {
    start: [f32; 4],
    end: [f32; 4],
    a: [f32; 4],
    b: [f32; 4],
}

pub(super) struct LinesRenderer {
    pipelines: pipelines::Pipelines,
    camera: wgpu::Buffer,
    camera_group: wgpu::BindGroup,
    camera_bytes: Vec<u8>,
    depth: wgpu::TextureView,
    size: (u32, u32),
    geometry: HashMap<MeshKind, geometry::Geometry>,
    instances: TechnicalLinesInstances,
    uploaded_revision: Option<u64>,
    assets_changed: bool,
    guides: DynamicBuffer,
    guide_group: wgpu::BindGroup,
    guide_bytes: Vec<u8>,
}

impl LinesRenderer {
    pub(super) fn new(context: &FrameContext<'_>) -> Self {
        let device = context.device;
        let pipelines = pipelines::Pipelines::new(device, context.format);
        let camera = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("wireframe camera"),
            size: size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("wireframe camera"),
            layout: &pipelines.camera_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera.as_entire_binding(),
            }],
        });
        let guides =
            DynamicBuffer::new(device, "wireframe guides", wgpu::BufferUsages::STORAGE, 256);
        let guide_group = pipelines.segment_group(device, &guides.buffer);
        Self {
            pipelines,
            camera,
            camera_group,
            camera_bytes: Vec::new(),
            depth: pipelines::depth(device, context.size),
            size: context.size,
            geometry: HashMap::new(),
            instances: TechnicalLinesInstances::default(),
            uploaded_revision: None,
            assets_changed: false,
            guides,
            guide_group,
            guide_bytes: Vec::new(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn encode(
        &mut self,
        context: &FrameContext<'_>,
        encoder: &mut wgpu::CommandEncoder,
        scene: &Scene,
        values: &SceneValues,
        view: &ViewConfiguration,
        _time_seconds: f32,
        crowd_amount: f32,
    ) -> FrameStats {
        self.prepare(context, scene, values, view, crowd_amount);
        self.draw(context, encoder, view)
    }

    pub(super) fn assets_changed(&self) -> bool {
        self.assets_changed
    }

    fn prepare(
        &mut self,
        context: &FrameContext<'_>,
        scene: &Scene,
        values: &SceneValues,
        view: &ViewConfiguration,
        crowd_amount: f32,
    ) {
        if self.size != context.size {
            self.depth = pipelines::depth(context.device, context.size);
            self.size = context.size;
        }
        let camera = ResolvedCamera::resolve(
            &view.camera,
            view.mode,
            context.size.0 as f32 / context.size.1.max(1) as f32,
            scene.bounds,
        );
        let uniform = CameraUniform {
            view_projection: camera.view_projection.to_cols_array_2d(),
            eye: camera
                .position
                .extend(if camera.orthographic { 1.0 } else { 0.0 })
                .to_array(),
            screen: [context.size.0 as f32, context.size.1 as f32, 1.25, 0.0],
            ink: {
                let c = view.theme.symbol_ink();
                [c[0], c[1], c[2], 1.0]
            },
        };
        let bytes = bytemuck::bytes_of(&uniform);
        if self.camera_bytes != bytes {
            context.queue.write_buffer(&self.camera, 0, bytes);
            self.camera_bytes.clear();
            self.camera_bytes.extend_from_slice(bytes);
        }
        let style = technical_style(view, camera.position, crowd_amount);
        self.instances.update(scene, values, &style);
        let revision = self.instances.revision();
        let frame = self.instances.frame();
        // Content identity is deliberately checked even if the instance pose did not change:
        // replacing a GLB in a reused model slot must never show the preceding asset's edges.
        let replaced = geometry::ensure(
            &mut self.geometry,
            context,
            &self.pipelines,
            scene,
            &frame.meshes,
        );
        self.assets_changed = replaced;
        if replaced || self.uploaded_revision != Some(revision) {
            for (kind, instances) in &frame.meshes {
                if let Some(mesh) = self.geometry.get_mut(kind) {
                    buffers::upload_changed(
                        context.device,
                        context.queue,
                        &mut mesh.instances,
                        &mut mesh.instance_bytes,
                        instances,
                    );
                }
            }
            let segments = frame
                .lines
                .chunks_exact(2)
                .map(|pair| Segment {
                    start: [
                        pair[0].position[0],
                        pair[0].position[1],
                        pair[0].position[2],
                        1.0,
                    ],
                    end: [
                        pair[1].position[0],
                        pair[1].position[1],
                        pair[1].position[2],
                        1.0,
                    ],
                    a: pair[0].colour,
                    b: pair[1].colour,
                })
                .collect::<Vec<_>>();
            if buffers::upload_changed(
                context.device,
                context.queue,
                &mut self.guides,
                &mut self.guide_bytes,
                &segments,
            ) {
                self.guide_group = self
                    .pipelines
                    .segment_group(context.device, &self.guides.buffer);
            }
            self.uploaded_revision = Some(revision);
        }
    }

    fn draw(
        &self,
        context: &FrameContext<'_>,
        encoder: &mut wgpu::CommandEncoder,
        view: &ViewConfiguration,
    ) -> FrameStats {
        let background = view.background_colour();
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("wireframe depth and analytic edges"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: context.output,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: background[0] as f64,
                        g: background[1] as f64,
                        b: background[2] as f64,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: context
                .timer
                .and_then(|t| t.render_writes(crate::timing::GpuPass::Opaque)),
            occlusion_query_set: None,
            multiview_mask: None,
        });
        let mut draw_calls = 0;
        let mut instances_total = 0;
        pass.set_bind_group(0, &self.camera_group, &[]);
        // Actual triangles only write depth. They hide rear edges without drawing shaded solids,
        // proxy boxes, or a second full-screen colour buffer.
        pass.set_pipeline(&self.pipelines.depth);
        for mesh in self.geometry.values().filter(|m| m.instances.length > 0) {
            pass.set_vertex_buffer(0, mesh.mesh.vertices.slice(..));
            pass.set_vertex_buffer(1, mesh.instances.buffer.slice(..));
            pass.set_index_buffer(mesh.mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.mesh.index_count, 0, 0..mesh.instances.length);
            draw_calls += 1;
            instances_total += mesh.instances.length;
        }
        pass.set_pipeline(&self.pipelines.edges);
        for mesh in self
            .geometry
            .values()
            .filter(|m| m.instances.length > 0 && m.edge_count > 0)
        {
            pass.set_bind_group(1, &mesh.edge_group, &[]);
            pass.set_vertex_buffer(0, mesh.instances.buffer.slice(..));
            pass.draw(0..mesh.edge_count * 6, 0..mesh.instances.length);
            draw_calls += 1;
        }
        if self.guides.length > 0 {
            pass.set_pipeline(&self.pipelines.guides);
            pass.set_bind_group(1, &self.guide_group, &[]);
            pass.draw(0..self.guides.length * 6, 0..1);
            draw_calls += 1;
        }
        drop(pass);
        FrameStats {
            instances: instances_total,
            draw_calls,
            render_scale: 1.0,
            ..Default::default()
        }
    }
}

fn technical_style(
    view: &ViewConfiguration,
    camera: glam::Vec3,
    crowd_amount: f32,
) -> crate::instances::FrameStyle {
    crate::instances::FrameStyle {
        quality: viz_scene::RenderQuality::Draft,
        camera_position: Some(camera),
        fixture_models: true,
        draw_beams: false,
        draw_aim_lines: true,
        emitter_apertures: false,
        scenery_surfaces: true,
        aim_guides: view.show_aim_guides,
        floor_grid: view.floor_grid,
        scenery: |kind| viz_scene::ViewMode::Lines3d.draws_scenery(kind),
        ink: view.theme.ink().into(),
        faint_ink: view.theme.faint_ink().into(),
        symbol_ink: view.theme.symbol_ink().into(),
        beam_ink: view.theme.beam_ink().into(),
        selected_ink: view.theme.selected_ink().into(),
        group_selected_ink: view.theme.group_selected_ink().into(),
        crowd_amount,
        crowd_person_budget: 0,
        effect_particle_budget: 0,
        media_content: false,
        ..Default::default()
    }
}
