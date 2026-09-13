//! Per-layer blend modes: the backdrop copy a non-Normal layer reads and the pipeline that
//! writes its finished blend.

use media_domain::LayerState;
use media_domain::geometry::{Size, layer_transform};

use super::PROGRAM_FORMAT;

/// The program-target pixels a layer's quad can touch, as `(x, y, width, height)`, with a pixel
/// of margin for rasterization rounding. `None` when the quad lies entirely off the output.
pub(super) fn backdrop_region(
    layer: &LayerState,
    source: Size,
    output: Size,
) -> Option<(u32, u32, u32, u32)> {
    let corners = layer_transform(layer, source, output).corners();
    let (mut left, mut top, mut right, mut bottom) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for corner in corners {
        left = left.min(corner.x);
        top = top.min(corner.y);
        right = right.max(corner.x);
        bottom = bottom.max(corner.y);
    }
    if !(left.is_finite() && top.is_finite() && right.is_finite() && bottom.is_finite()) {
        return None;
    }
    let clamp = |value: f32, limit: u32| value.clamp(0.0, limit as f32) as u32;
    let (left, right) = (
        clamp(left.floor() - 1.0, output.width),
        clamp(right.ceil() + 1.0, output.width),
    );
    let (top, bottom) = (
        clamp(top.floor() - 1.0, output.height),
        clamp(bottom.ceil() + 1.0, output.height),
    );
    (right > left && bottom > top).then_some((left, top, right - left, bottom - top))
}

pub(super) fn backdrop_target(
    device: &wgpu::Device,
    size: Size,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("media-layer-backdrop"),
        size: wgpu::Extent3d {
            width: size.width.max(1),
            height: size.height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: PROGRAM_FORMAT,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

pub(super) fn backdrop_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("media-layer-backdrop"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    })
}

/// The layer shader's `fragment_blend` entry point, writing its finished pixel without
/// fixed-function blending.
pub(super) fn blend_pipeline(
    device: &wgpu::Device,
    layer_layout: &wgpu::BindGroupLayout,
    backdrop_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let label = "media-layer-blend";
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/layer.wgsl").into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &[Some(layer_layout), Some(backdrop_layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vertex"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fragment_blend"),
            targets: &[Some(wgpu::ColorTargetState {
                format: PROGRAM_FORMAT,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}
