//! Uniform-only unlit pipelines: actual depth, analytic feature edges and coloured guides.

pub(super) struct Pipelines {
    pub camera_layout: wgpu::BindGroupLayout,
    segment_layout: wgpu::BindGroupLayout,
    batch_layout: wgpu::BindGroupLayout,
    projected_read_layout: wgpu::BindGroupLayout,
    projected_write_layout: wgpu::BindGroupLayout,
    pub project: wgpu::ComputePipeline,
    pub depth: wgpu::RenderPipeline,
    pub edges: wgpu::RenderPipeline,
    pub guides: wgpu::RenderPipeline,
}
impl Pipelines {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let camera_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("wireframe camera only"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let segment_layout = storage_layout(
            device,
            "wireframe guides",
            wgpu::ShaderStages::VERTEX,
            true,
            1,
        );
        let batch_layout = storage_layout(
            device,
            "wireframe shared geometry and instance maps",
            wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::COMPUTE,
            true,
            6,
        );
        let projected_read_layout = storage_layout(
            device,
            "wireframe projected edges read",
            wgpu::ShaderStages::VERTEX,
            true,
            1,
        );
        let projected_write_layout = storage_layout(
            device,
            "wireframe projected edges write",
            wgpu::ShaderStages::COMPUTE,
            false,
            1,
        );
        let depth_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("wireframe batched actual depth"),
            bind_group_layouts: &[Some(&camera_layout), Some(&batch_layout)],
            immediate_size: 0,
        });
        let edge_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("wireframe batched preprojected edges"),
            bind_group_layouts: &[
                Some(&camera_layout),
                Some(&batch_layout),
                Some(&projected_read_layout),
            ],
            immediate_size: 0,
        });
        let project = projection_pipeline(
            device,
            &camera_layout,
            &batch_layout,
            &projected_write_layout,
        );
        let guide_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("wireframe guides"),
            bind_group_layouts: &[Some(&camera_layout), Some(&segment_layout)],
            immediate_size: 0,
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("wireframe no lighting"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../shaders/wireframe.wgsl").into()),
        });
        let depth = pipeline(
            device,
            &module,
            &depth_layout,
            format,
            "wireframe actual depth",
            "depth_vertex",
            "depth_fragment",
            &[],
            true,
        );
        let edges = pipeline(
            device,
            &module,
            &edge_layout,
            format,
            "wireframe feature edges",
            "edge_vertex",
            "line_fragment",
            &[],
            false,
        );
        let guides = pipeline(
            device,
            &module,
            &guide_layout,
            format,
            "wireframe aim/grid/selection",
            "guide_vertex",
            "line_fragment",
            &[],
            false,
        );
        Self {
            camera_layout,
            segment_layout,
            batch_layout,
            projected_read_layout,
            projected_write_layout,
            project,
            depth,
            edges,
            guides,
        }
    }
    pub(super) fn batch_group(
        &self,
        device: &wgpu::Device,
        buffers: &[&wgpu::Buffer; 6],
    ) -> wgpu::BindGroup {
        let entries = buffers
            .iter()
            .enumerate()
            .map(|(binding, buffer)| wgpu::BindGroupEntry {
                binding: binding as u32,
                resource: buffer.as_entire_binding(),
            })
            .collect::<Vec<_>>();
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("wireframe retained batch"),
            layout: &self.batch_layout,
            entries: &entries,
        })
    }
    pub fn segment_group(&self, device: &wgpu::Device, buffer: &wgpu::Buffer) -> wgpu::BindGroup {
        storage_group(
            device,
            "wireframe retained segments",
            &self.segment_layout,
            buffer,
        )
    }
    pub fn projected_groups(
        &self,
        device: &wgpu::Device,
        buffer: &wgpu::Buffer,
    ) -> (wgpu::BindGroup, wgpu::BindGroup) {
        (
            storage_group(
                device,
                "wireframe projected edges read",
                &self.projected_read_layout,
                buffer,
            ),
            storage_group(
                device,
                "wireframe projected edges write",
                &self.projected_write_layout,
                buffer,
            ),
        )
    }
}

fn storage_group(
    device: &wgpu::Device,
    label: &str,
    layout: &wgpu::BindGroupLayout,
    buffer: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    })
}

fn storage_layout(
    device: &wgpu::Device,
    label: &str,
    visibility: wgpu::ShaderStages,
    read_only: bool,
    count: u32,
) -> wgpu::BindGroupLayout {
    let entries = (0..count)
        .map(|binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        })
        .collect::<Vec<_>>();
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
        entries: &entries,
    })
}

fn projection_pipeline(
    device: &wgpu::Device,
    camera: &wgpu::BindGroupLayout,
    batch: &wgpu::BindGroupLayout,
    projected: &wgpu::BindGroupLayout,
) -> wgpu::ComputePipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("wireframe once per edge projection"),
        bind_group_layouts: &[Some(camera), Some(batch), Some(projected)],
        immediate_size: 0,
    });
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("wireframe edge projection compute"),
        source: wgpu::ShaderSource::Wgsl(
            include_str!("../../shaders/wireframe_compute.wgsl").into(),
        ),
    });
    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("wireframe edge projection compute"),
        layout: Some(&layout),
        module: &module,
        entry_point: Some("project_edges"),
        compilation_options: Default::default(),
        cache: None,
    })
}

#[allow(clippy::too_many_arguments)]
fn pipeline(
    device: &wgpu::Device,
    module: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    format: wgpu::TextureFormat,
    label: &str,
    vertex: &str,
    fragment: &str,
    buffers: &[Option<wgpu::VertexBufferLayout<'_>>],
    depth_only: bool,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some(vertex),
            buffers,
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some(fragment),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: (!depth_only).then_some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: if depth_only {
                    wgpu::ColorWrites::empty()
                } else {
                    wgpu::ColorWrites::ALL
                },
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: Some(depth_only),
            depth_compare: Some(if depth_only {
                wgpu::CompareFunction::Less
            } else {
                wgpu::CompareFunction::LessEqual
            }),
            stencil: Default::default(),
            bias: wgpu::DepthBiasState {
                constant: if depth_only { 0 } else { -2 },
                slope_scale: 0.0,
                clamp: 0.0,
            },
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

pub(super) fn depth(device: &wgpu::Device, size: (u32, u32)) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("wireframe single-sample actual depth"),
            size: wgpu::Extent3d {
                width: size.0.max(1),
                height: size.1.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
}
