//! Draft-only final output pass. Lasers retain the established beam/HDR/laser ordering.
use super::resources::texture_entry;

pub(super) struct FusedComposite {
    pipeline: wgpu::RenderPipeline,
    group: wgpu::BindGroup,
    layout: wgpu::BindGroupLayout,
}
impl FusedComposite {
    pub(super) fn new(
        device: &wgpu::Device,
        upsample: &wgpu::BindGroupLayout,
        hdr: &wgpu::TextureView,
        format: wgpu::TextureFormat,
    ) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Draft fused opaque layout"),
            entries: &[texture_entry(
                0,
                wgpu::TextureSampleType::Float { filterable: false },
            )],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Draft fused composite layout"),
            bind_group_layouts: &[Some(upsample), Some(&layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Draft depth-aware fused composite"),
            source: wgpu::ShaderSource::Wgsl(
                concat!(
                    include_str!("../../shaders/draft_upsample.wgsl"),
                    "\n",
                    include_str!("../../shaders/draft_composite.wgsl"),
                )
                .into(),
            ),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Draft fused beam composite"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("composite_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let group = source_group(device, &layout, hdr);
        Self {
            pipeline,
            group,
            layout,
        }
    }
    pub(super) fn resize(&mut self, device: &wgpu::Device, hdr: &wgpu::TextureView) {
        self.group = source_group(device, &self.layout, hdr);
    }
    pub(super) fn encode(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        upsample: &wgpu::BindGroup,
        output: &wgpu::TextureView,
        timer: Option<&crate::timing::GpuTimer>,
    ) {
        let timing = timer.and_then(|timer| timer.render_writes(crate::timing::GpuPass::Composite));
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Draft fused upsample and tonemap"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: timing,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, upsample, &[]);
        pass.set_bind_group(1, &self.group, &[]);
        pass.draw(0..3, 0..1);
    }
}
fn source_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    hdr: &wgpu::TextureView,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Draft fused opaque source"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(hdr),
        }],
    })
}
