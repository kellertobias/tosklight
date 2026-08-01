//! What the shaders are allowed to see, and where each pass reads it from.
//!
//! Bind group layouts, the pipeline layouts built out of them, and the groups themselves. They
//! are here rather than beside the renderer because they describe the *shape* of the interface
//! between the passes and the GPU, which is a different subject from what the renderer does with
//! it every frame.

use super::{MAX_SHADOWS, SHADOW_ATLAS_EDGE, SHADOW_DRAW_STRIDE};
use crate::buffers::DynamicBuffer;
use crate::targets::DEPTH_FORMAT;
use wgpu::{BufferUsages, ShaderStages};

/// Every bind group layout the renderer builds pipelines against.
pub(super) struct Layouts {
    pub scene: wgpu::BindGroupLayout,
    pub cull: wgpu::BindGroupLayout,
    pub depth: wgpu::BindGroupLayout,
    pub post: wgpu::BindGroupLayout,
    pub composite: wgpu::BindGroupLayout,
    pub shadow: wgpu::BindGroupLayout,
    pub shadow_draw: wgpu::BindGroupLayout,
    pub overlay: wgpu::BindGroupLayout,
}

impl Layouts {
    pub(super) fn new(device: &wgpu::Device, samples: u32) -> Self {
        Self {
            scene: scene_bind_group_layout(device, false),
            cull: scene_bind_group_layout(device, true),
            depth: depth_layout(device, samples),
            post: post_layout(device),
            composite: composite_layout(device),
            shadow: shadow_layout(device),
            shadow_draw: shadow_draw_layout(device),
            overlay: overlay_layout(device),
        }
    }
}

/// The pipeline layouts, which are what each pass actually binds.
pub(super) struct PipelineLayouts {
    pub surface: wgpu::PipelineLayout,
    pub beam: wgpu::PipelineLayout,
    pub laser: wgpu::PipelineLayout,
    pub shadow: wgpu::PipelineLayout,
    pub cull: wgpu::PipelineLayout,
    pub post: wgpu::PipelineLayout,
    pub composite: wgpu::PipelineLayout,
    pub overlay: wgpu::PipelineLayout,
}

impl PipelineLayouts {
    pub(super) fn new(device: &wgpu::Device, layouts: &Layouts) -> Self {
        let scene_layout = &layouts.scene;
        let depth_layout = &layouts.depth;
        let shadow_layout = &layouts.shadow;
        let cull_layout = &layouts.cull;
        let post_layout = &layouts.post;
        let composite_layout = &layouts.composite;
        let shadow_draw_layout = &layouts.shadow_draw;
        let surface_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("viz surface layout"),
                bind_group_layouts: &[Some(scene_layout), None, Some(shadow_layout)],
                immediate_size: 0,
            });
        let beam_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("viz beam layout"),
            bind_group_layouts: &[Some(scene_layout), Some(depth_layout), Some(shadow_layout)],
            immediate_size: 0,
        });
        let shadow_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("viz shadow layout"),
                bind_group_layouts: &[Some(shadow_draw_layout)],
                immediate_size: 0,
            });
        let cull_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("viz cull layout"),
            bind_group_layouts: &[Some(cull_layout)],
            immediate_size: 0,
        });
        let post_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("viz post layout"),
            bind_group_layouts: &[Some(post_layout)],
            immediate_size: 0,
        });
        let composite_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("viz composite layout"),
                bind_group_layouts: &[Some(post_layout), Some(composite_layout)],
                immediate_size: 0,
            });
        let laser_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("viz laser layout"),
                // Scene and depth, and deliberately not the shadow atlas: a laser is drawn as the path
                // it takes rather than as a light, so it neither casts a shadow nor samples one.
                bind_group_layouts: &[Some(scene_layout), Some(depth_layout)],
                immediate_size: 0,
            });
        let overlay_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("viz overlay layout"),
                bind_group_layouts: &[Some(&layouts.overlay)],
                immediate_size: 0,
            });
        Self {
            surface: surface_pipeline_layout,
            beam: beam_pipeline_layout,
            laser: laser_pipeline_layout,
            shadow: shadow_pipeline_layout,
            cull: cull_pipeline_layout,
            post: post_pipeline_layout,
            composite: composite_pipeline_layout,
            overlay: overlay_pipeline_layout,
        }
    }
}

/// The shadow atlas, its comparison sampler, and the buffers one shadow draw reads.
pub(super) struct ShadowResources {
    pub atlas: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub matrices: DynamicBuffer,
    pub draws: wgpu::Buffer,
}

impl ShadowResources {
    pub(super) fn new(device: &wgpu::Device) -> Self {
        let shadow_atlas = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("viz shadow atlas"),
                size: wgpu::Extent3d {
                    width: SHADOW_ATLAS_EDGE,
                    height: SHADOW_ATLAS_EDGE,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default());
        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("viz shadow"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::Less),
            ..Default::default()
        });
        let shadow_matrices = DynamicBuffer::new(
            device,
            "viz shadow matrices",
            BufferUsages::STORAGE,
            (MAX_SHADOWS * 64) as u64,
        );
        // One aligned slot per shadow-casting light, selected by dynamic offset while drawing.
        let shadow_draws = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("viz shadow draws"),
            size: (SHADOW_DRAW_STRIDE * MAX_SHADOWS as u64).max(256),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            atlas: shadow_atlas,
            sampler: shadow_sampler,
            matrices: shadow_matrices,
            draws: shadow_draws,
        }
    }
}

fn depth_layout(device: &wgpu::Device, samples: u32) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("viz depth"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: samples > 1,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D3,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

fn post_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("viz post"),
        entries: &[
            texture_entry(0),
            sampler_entry(1),
            uniform_entry(2, ShaderStages::FRAGMENT),
        ],
    })
}

fn composite_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("viz composite"),
        entries: &[texture_entry(0), sampler_entry(1)],
    })
}

/// Shadow sampling: the atlas, a comparison sampler, and one matrix per shadow-casting light.
/// Both the surface pass and the volumetric pass read it, so a beam is broken by the same
/// geometry that darkens the floor under it.
fn shadow_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("viz shadow"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // The rig's gobo artwork, one layer per piece of glass. It sits with the shadow
            // atlas because these are exactly the two things both the surface pass and the
            // volumetric pass have to sample to know what a beam is doing.
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2Array,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

fn shadow_draw_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("viz shadow draw"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: std::num::NonZeroU64::new(16),
                },
                count: None,
            },
        ],
    })
}

fn overlay_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("viz overlay"),
        entries: &[
            uniform_entry(0, ShaderStages::VERTEX),
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                count: None,
            },
        ],
    })
}

/// The group both lighting passes read to know what a beam is doing: the shadow atlas it is
/// broken by, and the glass it is shining through.
#[allow(clippy::too_many_arguments)]
pub(super) fn build_shadow_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    shadow_atlas: &wgpu::TextureView,
    shadow_sampler: &wgpu::Sampler,
    shadow_matrices: &DynamicBuffer,
    gobo_atlas: &wgpu::TextureView,
    gobo_sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("viz shadow"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(shadow_atlas),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(shadow_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: shadow_matrices.buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(gobo_atlas),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::Sampler(gobo_sampler),
            },
        ],
    })
}

pub(super) fn scene_bind_group_layout(
    device: &wgpu::Device,
    writable: bool,
) -> wgpu::BindGroupLayout {
    let visibility = if writable {
        ShaderStages::COMPUTE
    } else {
        ShaderStages::VERTEX_FRAGMENT
    };
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("viz scene"),
        entries: &[
            uniform_entry(0, visibility),
            storage_entry(1, visibility, true),
            storage_entry(2, visibility, !writable),
            storage_entry(3, visibility, !writable),
        ],
    })
}

pub(super) fn build_scene_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    globals: &wgpu::Buffer,
    lights: &DynamicBuffer,
    tile_counts: &DynamicBuffer,
    tile_lights: &DynamicBuffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("viz scene"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: globals.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: lights.buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: tile_counts.buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: tile_lights.buffer.as_entire_binding(),
            },
        ],
    })
}

pub(super) fn post_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    texture: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    settings: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("viz post"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(texture),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: settings.as_entire_binding(),
            },
        ],
    })
}

fn uniform_entry(binding: u32, visibility: ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn storage_entry(
    binding: u32,
    visibility: ShaderStages,
    read_only: bool,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn sampler_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}
