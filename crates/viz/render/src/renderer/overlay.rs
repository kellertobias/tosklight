//! Full-resolution text and UI rendering, shared by both backends.
use super::{DynamicBuffer, OverlayAtlas};

pub(super) struct OverlayRenderer {
    atlas: OverlayAtlas,
    pipeline: wgpu::RenderPipeline,
    quads: DynamicBuffer,
}

impl OverlayRenderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        icon: Option<&[u8]>,
    ) -> Self {
        let layout = super::layouts::overlay_layout(device);
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("viz common overlay layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("viz common overlay"),
            source: wgpu::ShaderSource::Wgsl(super::OVERLAY_WGSL.into()),
        });
        Self {
            atlas: OverlayAtlas::new(device, queue, &layout, icon),
            pipeline: super::pipelines::overlay_pipeline(device, &pipeline_layout, &module, format),
            quads: DynamicBuffer::new(
                device,
                "viz overlay quads",
                wgpu::BufferUsages::VERTEX,
                (size_of::<crate::overlay::OverlayQuad>() * 4096) as u64,
            ),
        }
    }
    /// The status line, Quick Settings and plan labels, drawn over the finished picture.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn encode(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        size: (u32, u32),
        overlay: &crate::overlay::Overlay,
        output: &wgpu::TextureView,
        timer: Option<&crate::timing::GpuTimer>,
    ) {
        self.quads.upload(device, queue, &overlay.quads);
        queue.write_buffer(
            &self.atlas.globals,
            0,
            bytemuck::cast_slice(&[
                size.0 as f32,
                size.1 as f32,
                1.0 / size.0 as f32,
                1.0 / size.1 as f32,
            ]),
        );
        let timing = timer.and_then(|timer| timer.render_writes(crate::timing::GpuPass::Overlay));
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("viz overlay"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: timing,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.atlas.bind_group, &[]);
        pass.set_vertex_buffer(0, self.quads.buffer.slice(..));
        pass.draw(0..6, 0..self.quads.length);
        drop(pass);
    }
}
