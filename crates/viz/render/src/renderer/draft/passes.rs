use super::super::ShadedRenderer;
use crate::timing::GpuTimer;

impl ShadedRenderer {
    pub(in crate::renderer) fn draft_beam_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        timer: Option<&GpuTimer>,
    ) {
        let draft = self
            .draft
            .as_ref()
            .expect("Draft pass requires Draft resources");
        let timing = timer.and_then(|timer| timer.render_opening(crate::timing::GpuPass::Beams));
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Draft half-resolution beams"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &draft.beam,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: timing,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.beam_pipeline);
            pass.set_bind_group(0, &draft.scene, &[]);
            pass.set_bind_group(1, &self.depth_bind_group, &[]);
            pass.set_bind_group(2, &self.shadow_bind_group, &[]);
            pass.set_vertex_buffer(0, self.cone.vertices.slice(..));
            pass.set_vertex_buffer(1, self.beam_instances.buffer.slice(..));
            pass.set_index_buffer(self.cone.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..self.cone.index_count, 0, 0..self.beam_instances.length);
        }
        let timing = timer.and_then(|timer| timer.render_closing(crate::timing::GpuPass::Beams));
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Draft depth-aware beam upsample"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.targets.hdr,
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
        pass.set_pipeline(&draft.upsample);
        pass.set_bind_group(0, &draft.upsample_group, &[]);
        pass.draw(0..3, 0..1);
    }
}
