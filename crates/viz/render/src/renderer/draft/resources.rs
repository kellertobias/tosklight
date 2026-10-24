use super::super::{Globals, SceneBuffers, Targets, build_scene_group};
use crate::targets::HDR_FORMAT;

pub(in crate::renderer) struct DraftPasses {
    pub preparation: super::preparation::Preparation,
    pub size: (u32, u32),
    pub beam: wgpu::TextureView,
    pub globals: wgpu::Buffer,
    pub scene: wgpu::BindGroup,
    pub upsample: wgpu::RenderPipeline,
    pub upsample_group: wgpu::BindGroup,
    settings: wgpu::Buffer,
    upsample_layout: wgpu::BindGroupLayout,
}

pub(super) fn half_size(size: (u32, u32)) -> (u32, u32) {
    (size.0.max(1).div_ceil(2), size.1.max(1).div_ceil(2))
}
fn beam_target(device: &wgpu::Device, size: (u32, u32)) -> wgpu::TextureView {
    let (width, height) = half_size(size);
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("Draft half-resolution beam target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: HDR_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&Default::default())
}
impl DraftPasses {
    pub(in crate::renderer) fn new(
        device: &wgpu::Device,
        size: (u32, u32),
        scene_layout: &wgpu::BindGroupLayout,
        buffers: &SceneBuffers,
        targets: &Targets,
    ) -> Self {
        let globals = uniform(device, "Draft beam globals", size_of::<Globals>() as u64);
        let settings = uniform(device, "Draft upsample settings", 16);
        let scene = build_scene_group(
            device,
            scene_layout,
            &globals,
            &buffers.lights,
            &buffers.tile_counts,
            &buffers.tile_lights,
        );
        let beam = beam_target(device, size);
        let upsample_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Draft depth-aware upsample layout"),
            entries: &[
                texture_entry(0, wgpu::TextureSampleType::Float { filterable: false }),
                texture_entry(1, wgpu::TextureSampleType::Depth),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Draft upsample pipeline layout"),
            bind_group_layouts: &[Some(&upsample_layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Draft depth-aware premultiplied upsample"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../shaders/draft_upsample.wgsl").into(),
            ),
        });
        let upsample = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Draft beam upsample"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR_FORMAT,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
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
        let upsample_group =
            upsample_group(device, &upsample_layout, &beam, &targets.depth, &settings);
        Self {
            preparation: Default::default(),
            size,
            beam,
            globals,
            scene,
            upsample,
            upsample_group,
            settings,
            upsample_layout,
        }
    }
    pub(in crate::renderer) fn resize(&mut self, device: &wgpu::Device, targets: &Targets) {
        let size = (targets.width, targets.height);
        if self.size == size {
            return;
        }
        self.size = size;
        self.beam = beam_target(device, size);
        self.upsample_group = upsample_group(
            device,
            &self.upsample_layout,
            &self.beam,
            &targets.depth,
            &self.settings,
        );
    }
    pub(in crate::renderer) fn write_globals(&self, queue: &wgpu::Queue, globals: &Globals) {
        let mut beam = *globals;
        let (width, height) = half_size(self.size);
        beam.screen = [
            width as f32,
            height as f32,
            1.0 / width as f32,
            1.0 / height as f32,
        ];
        queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&beam));
        queue.write_buffer(
            &self.settings,
            0,
            bytemuck::cast_slice(&[globals.params[2], globals.params[3], 0.0, 0.0]),
        );
    }
}
fn uniform(device: &wgpu::Device, label: &str, size: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}
fn texture_entry(binding: u32, sample_type: wgpu::TextureSampleType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type,
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}
fn upsample_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    beam: &wgpu::TextureView,
    depth: &wgpu::TextureView,
    settings: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Draft upsample inputs"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(beam),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(depth),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: settings.as_entire_binding(),
            },
        ],
    })
}
#[cfg(test)]
mod tests {
    #[test]
    fn half_beams_cover_odd_physical_sizes_without_reducing_geometry() {
        assert_eq!(super::half_size((1920, 1080)), (960, 540));
        assert_eq!(super::half_size((1921, 1081)), (961, 541));
        assert_eq!(super::half_size((0, 1)), (1, 1));
    }
}
