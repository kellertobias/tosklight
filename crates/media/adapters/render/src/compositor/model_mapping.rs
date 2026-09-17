//! Layers mapped onto numbered 3D models.
//!
//! A layer whose `3D model` selection resolves draws in two steps before the program composite:
//!
//! 1. **Look.** The layer renders exactly as its flat quad would — source, effects, tint,
//!    greyscale, mask and dimmer — but untransformed, filling an intermediate texture at the
//!    source's aspect ratio. That texture is the image the model's texture coordinates address.
//! 2. **Mesh.** The model draws with a depth buffer into an output-sized texture, sampling the look
//!    through its UVs, placed by the layer's position, scale, pan, tilt and roll on the output's
//!    fixed camera (see [`media_domain::model_projection`]).
//!
//! The compositor then draws that output-sized texture as an ordinary, untransformed flat layer
//! that keeps the original layer's blend mode and strobe. Blend, strobe and the opacity cycle
//! (which rides in the dimmer, already inside the look) therefore apply to the mapped result
//! through exactly the code a flat layer uses. A layer whose selected model is not installed is
//! mapped onto the built-in Plane — never onto any other model, and never left black.
//!
//! The built-in Plane — in any slot, and as that fallback — always has the output's aspect ratio.
//! Its look is the flat layer's own picture on an output-shaped canvas (scaling mode included,
//! placement left to the mesh), and it draws on a card that, unturned at scale 1, covers exactly
//! the output. A clip on the Plane at Pan 0 / Tilt 0 therefore looks exactly like the flat layer,
//! and the Plane follows a resolution change on the next frame.
//!
//! Flat (model 0) at Pan 0 and Tilt 0 never enters this path. Once Pan or Tilt turns a Flat
//! layer, it draws on a flat card: a quad sized so that, unturned, it projects exactly onto the
//! flat layer's own rectangle, including its scaling mode.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use media_domain::geometry::{Size, scaling_mode_factor};
use media_domain::model_projection::{Matrix4, model_view_projection};
use media_domain::{
    BlendMode, BuiltinModel, LayerState, MaskState, ModelGeometry, ModelMapping, ModelVertex,
    OutputId, ScalingMode, Timestamp, Tint,
};
use wgpu::util::DeviceExt as _;

use super::{LayerDraw, LayerUniform, MAX_LAYERS, bind_group};
use crate::feedback::FeedbackProcessor;
use crate::gpu::Gpu;
use crate::texture::{SourceTexture, VISUALIZER_FORMAT};

/// Uploads one mesh, or says why this GPU cannot hold it.
fn upload(gpu: &Gpu, label: &str, geometry: &Arc<ModelGeometry>) -> Result<GpuModel, String> {
    let limit = gpu.device.limits().max_buffer_size;
    let vertex_bytes = (geometry.vertices.len() * std::mem::size_of::<GpuVertex>()) as u64;
    let index_bytes = (geometry.indices.len() * std::mem::size_of::<u32>()) as u64;
    if geometry.indices.len() < 3 || geometry.vertices.is_empty() {
        return Err("the model has no triangles".to_owned());
    }
    if vertex_bytes > limit || index_bytes > limit {
        return Err(format!(
            "the model needs a {vertex_bytes}-byte buffer; this GPU allows {limit}"
        ));
    }
    let vertices: Vec<GpuVertex> = geometry
        .vertices
        .iter()
        .map(|vertex| GpuVertex {
            position: vertex.position,
            normal: vertex.normal,
            uv: vertex.uv,
        })
        .collect();
    let whole = geometry.indices.len() / 3 * 3;
    Ok(GpuModel {
        geometry: Arc::clone(geometry),
        vertices: gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("media-model-{label}-vertices")),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
        indices: gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("media-model-{label}-indices")),
                contents: bytemuck::cast_slice(&geometry.indices[..whole]),
                usage: wgpu::BufferUsages::INDEX,
            }),
        index_count: whole as u32,
    })
}

/// The models an output can map layers onto, by slot. Shared, immutable CPU meshes: each output
/// uploads its own GPU copy because each output may own its own device.
pub type ModelGeometries = BTreeMap<u8, Arc<ModelGeometry>>;

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// The look texture's longest side never exceeds this, whatever the output size.
const MAX_LOOK_DIMENSION: u32 = 4096;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuVertex {
    position: [f32; 3],
    normal: [f32; 3],
    uv: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MeshUniform {
    model_view_projection: [[f32; 4]; 4],
}

struct GpuModel {
    geometry: Arc<ModelGeometry>,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
}

struct MappedSlot {
    look: Option<SourceTexture>,
    mapped: Option<SourceTexture>,
    look_uniform: wgpu::Buffer,
    mesh_uniform: wgpu::Buffer,
    /// The flat stand-in the compositor draws this frame.
    composite: LayerState,
    active: bool,
}

/// What a frame's mapping pass borrows from the compositor.
pub(super) struct MappingContext<'a> {
    pub gpu: &'a Gpu,
    pub look_pipeline: &'a wgpu::RenderPipeline,
    pub layer_layout: &'a wgpu::BindGroupLayout,
    pub sampler: &'a wgpu::Sampler,
    pub feedback: &'a FeedbackProcessor,
    pub no_mask: &'a SourceTexture,
    pub output: Size,
    pub output_id: OutputId,
    pub now: Timestamp,
}

/// What a projected layer draws on this frame.
#[derive(Clone, Copy)]
enum Surface<'a> {
    /// An installed model other than the built-in Plane, through its own texture coordinates.
    Model(&'a GpuModel),
    /// A turned Flat layer: the card, sized to the flat layer's rectangle.
    FlatCard,
    /// The built-in Plane, selected or as the fallback: the card, sized to the output.
    Plane,
}

impl<'a> Surface<'a> {
    fn select(
        mapping: ModelMapping,
        models: &'a HashMap<u8, GpuModel>,
        plane: &Arc<ModelGeometry>,
    ) -> Self {
        if mapping.is_flat() {
            return Self::FlatCard;
        }
        match models.get(&mapping.model) {
            Some(model) if !Arc::ptr_eq(&model.geometry, plane) => Self::Model(model),
            _ => Self::Plane,
        }
    }

    fn mesh(self, flat_card: Option<&'a GpuModel>) -> Option<&'a GpuModel> {
        match self {
            Self::Model(model) => Some(model),
            Self::FlatCard | Self::Plane => flat_card,
        }
    }

    /// The proportions of the look texture: the output's for the Plane, the source's otherwise.
    fn look_shape(self, source: Size, output: Size) -> Size {
        match self {
            Self::Plane => output,
            Self::Model(_) | Self::FlatCard => source,
        }
    }

    /// The look's layer state and the space it is laid out in. The Plane's look is laid out in
    /// output pixels; its texture has the output's proportions, so a size capped below the output
    /// shows the same picture.
    fn look(self, layer: &LayerState, look_size: Size, output: Size) -> (LayerState, Size) {
        match self {
            Self::Plane => (plane_look_state(layer), output),
            Self::Model(_) | Self::FlatCard => (look_state(layer), look_size),
        }
    }
}

pub(super) struct ModelMapper {
    models: HashMap<u8, GpuModel>,
    /// The shared built-in Plane mesh, recognised in any slot so it draws output-proportioned.
    plane: Arc<ModelGeometry>,
    /// The `±1` card the turned Flat layer and the built-in Plane draw on.
    flat_card: Option<GpuModel>,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    depth: Option<(wgpu::TextureView, Size)>,
    slots: Vec<MappedSlot>,
}

impl ModelMapper {
    pub(super) fn new(gpu: &Gpu) -> Self {
        let device = &gpu.device;
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("media-model"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let slots = (0..MAX_LAYERS)
            .map(|index| MappedSlot {
                look: None,
                mapped: None,
                look_uniform: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(&format!("media-model-look-{index}")),
                    size: std::mem::size_of::<LayerUniform>() as u64,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                mesh_uniform: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(&format!("media-model-mesh-{index}")),
                    size: std::mem::size_of::<MeshUniform>() as u64,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                composite: LayerState::default(),
                active: false,
            })
            .collect();
        Self {
            models: HashMap::new(),
            plane: BuiltinModel::DEFAULT.geometry(),
            flat_card: upload(gpu, "flat-card", &Arc::new(flat_card())).ok(),
            pipeline: mesh_pipeline(device, &layout),
            layout,
            depth: None,
            slots,
        }
    }

    /// Makes exactly `models` available. A mesh already uploaded from the same shared geometry is
    /// kept; anything else is uploaded or dropped. Returns the slots that could not be uploaded and
    /// why — those layers are mapped onto the Plane.
    pub(super) fn install(&mut self, gpu: &Gpu, models: &ModelGeometries) -> Vec<(u8, String)> {
        self.models.retain(|slot, installed| {
            models
                .get(slot)
                .is_some_and(|geometry| Arc::ptr_eq(geometry, &installed.geometry))
        });
        let mut rejected = Vec::new();
        for (slot, geometry) in models {
            if self.models.contains_key(slot) {
                continue;
            }
            match upload(gpu, &slot.to_string(), geometry) {
                Ok(model) => {
                    self.models.insert(*slot, model);
                }
                Err(reason) => rejected.push((*slot, reason)),
            }
        }
        rejected
    }

    /// Renders every mapped layer's look and mesh for this frame.
    pub(super) fn prepare(
        &mut self,
        context: &MappingContext<'_>,
        encoder: &mut wgpu::CommandEncoder,
        layers: &[LayerDraw<'_>],
    ) {
        for slot in &mut self.slots {
            slot.active = false;
        }
        for (index, layer) in layers.iter().take(MAX_LAYERS).enumerate() {
            let mapping = layer.state.model;
            if !mapping.is_projected() || !layer.state.draws() {
                continue;
            }
            let surface = Surface::select(mapping, &self.models, &self.plane);
            let Some(model) = surface.mesh(self.flat_card.as_ref()) else {
                continue;
            };
            let gpu = context.gpu;
            let look_size = look_size(
                surface.look_shape(layer.source.size(), context.output),
                context.output,
                gpu.capabilities.max_texture_dimension,
            );
            if self
                .depth
                .as_ref()
                .is_none_or(|(_, size)| *size != context.output)
            {
                self.depth = depth_target(gpu, context.output).map(|view| (view, context.output));
            }
            let Some((depth, _)) = self.depth.as_ref() else {
                continue;
            };
            let slot = &mut self.slots[index];
            if !ensure_target(&mut slot.look, gpu, look_size)
                || !ensure_target(&mut slot.mapped, gpu, context.output)
            {
                continue;
            }
            let (Some(look), Some(mapped)) = (slot.look.as_ref(), slot.mapped.as_ref()) else {
                continue;
            };

            let (look_state, look_space) = surface.look(layer.state, look_size, context.output);
            let uniform = LayerUniform::new(
                &look_state,
                layer.source.size(),
                look_space,
                layer.mask,
                context.output_id,
                context.now,
            );
            gpu.queue
                .write_buffer(&slot.look_uniform, 0, bytemuck::bytes_of(&uniform));
            let look_group = bind_group(
                &gpu.device,
                context.layer_layout,
                &slot.look_uniform,
                context.feedback.source(layer),
                context.sampler,
                &layer.mask.unwrap_or(context.no_mask).view,
            );
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("media-model-look"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &look.view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(context.look_pipeline);
                pass.set_bind_group(0, &look_group, &[]);
                pass.draw(0..6, 0..1);
            }

            gpu.queue.write_buffer(
                &slot.mesh_uniform,
                0,
                bytemuck::bytes_of(&MeshUniform {
                    model_view_projection: mesh_placement(layer, surface, context.output),
                }),
            );
            let mesh_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("media-model"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: slot.mesh_uniform.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&look.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(context.sampler),
                    },
                ],
            });
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("media-model-mesh"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &mapped.view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: depth,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Discard,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &mesh_group, &[]);
                pass.set_vertex_buffer(0, model.vertices.slice(..));
                pass.set_index_buffer(model.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..model.index_count, 0, 0..1);
            }

            slot.composite = composite_state(layer.state);
            slot.active = true;
        }
    }

    /// The layers the compositor draws this frame: a mapped layer becomes its flat stand-in, and
    /// every other layer is passed through untouched.
    pub(super) fn substitute<'a>(&'a self, layers: &[LayerDraw<'a>]) -> Vec<LayerDraw<'a>> {
        layers
            .iter()
            .enumerate()
            .map(
                |(index, layer)| match self.slots.get(index).filter(|slot| slot.active) {
                    Some(MappedSlot {
                        mapped: Some(mapped),
                        composite,
                        ..
                    }) => LayerDraw {
                        state: composite,
                        source: mapped,
                        mask: None,
                    },
                    _ => *layer,
                },
            )
            .collect()
    }
}

fn ensure_target(target: &mut Option<SourceTexture>, gpu: &Gpu, size: Size) -> bool {
    if target.as_ref().map(SourceTexture::size) != Some(size) {
        *target = SourceTexture::render_target(gpu, size).ok();
    }
    target.is_some()
}

fn depth_target(gpu: &Gpu, size: Size) -> Option<wgpu::TextureView> {
    if !gpu.supports_resolution(size.width, size.height) {
        return None;
    }
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("media-model-depth"),
        size: wgpu::Extent3d {
            width: size.width,
            height: size.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    Some(texture.create_view(&wgpu::TextureViewDescriptor::default()))
}

/// The look texture: the source's aspect ratio, its longest side matching the output's.
fn look_size(source: Size, output: Size, adapter_limit: u32) -> Size {
    let long = output
        .width
        .max(output.height)
        .clamp(1, MAX_LOOK_DIMENSION.min(adapter_limit.max(1)));
    if source.is_empty() {
        return Size::new(long, long);
    }
    let scaled = |short: u32, longest: u32| {
        ((f64::from(long) * f64::from(short) / f64::from(longest)).round() as u32).max(1)
    };
    if source.width >= source.height {
        Size::new(long, scaled(source.height, source.width))
    } else {
        Size::new(scaled(source.width, source.height), long)
    }
}

fn mesh_placement(layer: &LayerDraw<'_>, surface: Surface<'_>, output: Size) -> Matrix4 {
    match surface {
        Surface::Model(_) => model_view_projection(layer.state, output),
        Surface::FlatCard => {
            let placement = flat_card_placement(layer.state, layer.source.size(), output);
            model_view_projection(&placement, output)
        }
        Surface::Plane => model_view_projection(&plane_placement(layer.state, output), output),
    }
}

/// The layer with its scale widened by the output's aspect ratio, so the `±1` card spans the
/// whole output at scale 1: the world spans `±1` vertically and `±aspect` horizontally. Scale X/Y
/// stay relative to that output-shaped Plane.
fn plane_placement(layer: &LayerState, output: Size) -> LayerState {
    if output.is_empty() {
        return layer.clone();
    }
    LayerState {
        scale_x: layer.scale_x * output.width as f32 / output.height as f32,
        ..layer.clone()
    }
}

/// A square quad facing the camera, `±1` on both axes and deliberately not normalized; its
/// placement scale makes it the flat layer's rectangle.
fn flat_card() -> ModelGeometry {
    let vertex = |x: f32, y: f32, u: f32, v: f32| ModelVertex {
        position: [x, y, 0.0],
        normal: [0.0, 0.0, 1.0],
        uv: [u, v],
    };
    ModelGeometry {
        vertices: vec![
            vertex(-1.0, -1.0, 0.0, 1.0),
            vertex(1.0, -1.0, 1.0, 1.0),
            vertex(1.0, 1.0, 1.0, 0.0),
            vertex(-1.0, 1.0, 0.0, 0.0),
        ],
        indices: vec![0, 1, 2, 0, 2, 3],
    }
}

/// The layer with its scale replaced so the flat card covers what the flat quad would: the world
/// spans `±1` vertically and `±aspect` horizontally, so the card's half-width in world units is
/// the flat layer's width over the output height.
fn flat_card_placement(layer: &LayerState, source: Size, output: Size) -> LayerState {
    if output.is_empty() {
        return layer.clone();
    }
    let (fit_x, fit_y) = scaling_mode_factor(layer.scaling_mode, source, output);
    let height = output.height as f32;
    LayerState {
        scale_x: layer.scale_x * fit_x * source.width as f32 / height,
        scale_y: layer.scale_y * fit_y * source.height as f32 / height,
        ..layer.clone()
    }
}

/// The layer's look without its placement: it fills the look texture exactly. Blend and strobe
/// belong to the final composite, not to the look.
fn look_state(layer: &LayerState) -> LayerState {
    LayerState {
        position_x: 0.0,
        position_y: 0.0,
        rotation: 0.0,
        scale_x: 1.0,
        scale_y: 1.0,
        scaling_mode: ScalingMode::Stretch,
        blend: BlendMode::Normal,
        strobe_hz: None,
        model: ModelMapping::default(),
        ..layer.clone()
    }
}

/// The Plane's look: the flat layer's picture on an output-shaped canvas — its scaling mode kept,
/// its placement left to the mesh — so the unturned Plane at scale 1 is the flat layer.
fn plane_look_state(layer: &LayerState) -> LayerState {
    LayerState {
        scaling_mode: layer.scaling_mode,
        ..look_state(layer)
    }
}

/// The flat stand-in that composites the mapped image. Everything the look already applied is
/// neutral here, so nothing is applied twice; blend mode and strobe carry over.
fn composite_state(layer: &LayerState) -> LayerState {
    LayerState {
        position_x: 0.0,
        position_y: 0.0,
        rotation: 0.0,
        scale_x: 1.0,
        scale_y: 1.0,
        scaling_mode: ScalingMode::Stretch,
        dimmer: 1.0,
        tint: Tint::WHITE,
        grayscale: 0.0,
        blur: 0.0,
        mask: MaskState::default(),
        effects: Default::default(),
        effect_banks: Default::default(),
        model: ModelMapping::default(),
        ..layer.clone()
    }
}

fn mesh_pipeline(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> wgpu::RenderPipeline {
    const ATTRIBUTES: [wgpu::VertexAttribute; 3] =
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2];
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("media-model"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/model.wgsl").into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("media-model"),
        bind_group_layouts: &[Some(layout)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("media-model"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vertex"),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<GpuVertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &ATTRIBUTES,
            })],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fragment"),
            targets: &[Some(wgpu::ColorTargetState {
                format: VISUALIZER_FORMAT,
                // The depth test decides what is visible; the pixel is written, not blended.
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        // Both faces draw: a flat screen model is as visible from behind as from the front.
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use media_domain::{EffectSlot, MediaAddress};

    #[test]
    fn the_look_keeps_the_source_aspect_at_the_output_resolution() {
        let output = Size::new(1920, 1080);
        assert_eq!(
            look_size(Size::new(1280, 720), output, 8192),
            Size::new(1920, 1080)
        );
        assert_eq!(
            look_size(Size::new(500, 1000), output, 8192),
            Size::new(960, 1920)
        );
        assert_eq!(
            look_size(Size::new(1, 1), output, 8192),
            Size::new(1920, 1920)
        );
        assert_eq!(
            look_size(Size::new(8000, 8000), Size::new(8000, 8000), 2048),
            Size::new(2048, 2048),
            "never beyond the adapter"
        );
    }

    #[test]
    fn look_and_composite_split_the_layer_without_applying_anything_twice() {
        let mut layer = LayerState {
            address: MediaAddress::new(1, 1),
            position_x: 0.5,
            rotation: 30.0,
            scale_x: 2.0,
            dimmer: 0.5,
            tint: Tint::new(1.0, 0.0, 0.0),
            grayscale: 0.4,
            blend: BlendMode::Add,
            strobe_hz: Some(4.0),
            model: ModelMapping {
                model: 3,
                pan: 10.0,
                tilt: 5.0,
            },
            ..Default::default()
        };
        layer.effects[0] = EffectSlot::blur();

        let look = look_state(&layer);
        assert_eq!(
            (look.position_x, look.rotation, look.scale_x),
            (0.0, 0.0, 1.0)
        );
        assert_eq!(look.scaling_mode, ScalingMode::Stretch);
        assert_eq!((look.dimmer, look.grayscale), (0.5, 0.4));
        assert_eq!(look.effects[0], layer.effects[0]);
        assert_eq!((look.blend, look.strobe_hz), (BlendMode::Normal, None));

        let composite = composite_state(&layer);
        assert_eq!((composite.dimmer, composite.grayscale), (1.0, 0.0));
        assert_eq!(composite.tint, Tint::WHITE);
        assert!(
            composite
                .effects
                .iter()
                .all(|effect| effect.effect_type.is_none())
        );
        assert_eq!(
            (composite.blend, composite.strobe_hz),
            (BlendMode::Add, Some(4.0))
        );
        assert!(composite.model.is_flat());
        assert_eq!(composite.address, layer.address, "it still draws");
    }

    #[test]
    fn the_plane_spans_the_output_and_keeps_the_scaling_mode() {
        let layer = LayerState {
            scale_x: 0.5,
            scale_y: 2.0,
            position_x: 0.3,
            scaling_mode: ScalingMode::Fit,
            ..Default::default()
        };
        let wide = plane_placement(&layer, Size::new(1920, 1080));
        assert!((wide.scale_x - 0.5 * 16.0 / 9.0).abs() < 1e-5);
        assert_eq!((wide.scale_y, wide.position_x), (2.0, 0.3));
        let classic = plane_placement(&layer, Size::new(1024, 768));
        assert!((classic.scale_x - 0.5 * 4.0 / 3.0).abs() < 1e-5);

        let look = plane_look_state(&layer);
        assert_eq!(look.scaling_mode, ScalingMode::Fit);
        assert_eq!(
            (look.scale_x, look.scale_y, look.position_x),
            (1.0, 1.0, 0.0)
        );
    }

    #[test]
    fn the_model_shader_is_valid_wgsl() {
        let module = naga::front::wgsl::parse_str(include_str!("../shaders/model.wgsl")).unwrap();
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap();
    }

    #[test]
    fn the_vertex_and_uniform_layouts_match_the_shader() {
        assert_eq!(std::mem::size_of::<GpuVertex>(), 32);
        assert_eq!(std::mem::size_of::<MeshUniform>(), 64);
    }
}
