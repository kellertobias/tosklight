//! The layer compositor.
//!
//! Layers draw into a program target in order, lowest first, with normal alpha blending. The
//! master pass then tints, dims, and flips that finished composite onto the output.

use bytemuck::{Pod, Zeroable};
use media_domain::geometry::{Size, layer_transform};
use media_domain::{LayerState, MaskSource, MasterState, OutputId, Timestamp, geometry};

use crate::feedback::FeedbackProcessor;
use crate::gpu::Gpu;
use crate::texture::SourceTexture;

/// The most layers one output composites. The eight-layer personality is the larger of the two
/// supported products.
pub const MAX_LAYERS: usize = 8;

/// The program target's format. Linear rather than sRGB, so a reference render is byte-identical
/// wherever it runs.
pub const PROGRAM_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// One layer to draw this frame.
pub struct LayerDraw<'a> {
    pub state: &'a LayerState,
    pub source: &'a SourceTexture,
    /// The layer's mask, when its address resolved to one. A mask that is selected but has not
    /// loaded is `None`, and a missing mask means no mask — never a black layer.
    pub mask: Option<&'a SourceTexture>,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct LayerUniform {
    center: [f32; 2],
    size: [f32; 2],
    rotation: [f32; 2],
    output: [f32; 2],
    tint: [f32; 4],
    controls: [f32; 4],
    mask: [f32; 4],
    mask_source: [f32; 4],
    effect_types: [u32; 4],
    effect_mixes: [f32; 4],
    effect_parameters: [[f32; 4]; 4],
    /// Fifth typed parameter for effects that need it, one value per slot.
    effect_parameter_tail: [f32; 4],
    effect_seeds: [f32; 4],
    /// Authoritative playback seconds, output width/height, spare.
    effect_clock: [f32; 4],
    /// Transient beat-scan event base positions and strength-derived line counts. Each vec4 maps
    /// one event across effect slots 1..4; these never enter persisted layer state.
    beat_scan_positions: [[f32; 4]; 16],
    beat_scan_counts: [[f32; 4]; 16],
}

impl LayerUniform {
    fn new(
        layer: &LayerState,
        source: Size,
        output: Size,
        has_mask: bool,
        output_id: OutputId,
        now: Timestamp,
    ) -> Self {
        let transform = layer_transform(layer, source, output);
        let (sin, cos) = transform.rotation_degrees.to_radians().sin_cos();
        let mut effect_types = [0; 4];
        let mut effect_mixes = [0.0; 4];
        let mut effect_parameters = [[0.0; 4]; 4];
        let mut effect_parameter_tail = [0.0; 4];
        let mut effect_seeds = [0.0; 4];
        let mut beat_scan_positions = [[-2.0; 4]; 16];
        let mut beat_scan_counts = [[0.0; 4]; 16];
        for (index, effect) in layer.effects.iter().enumerate() {
            if let Some(parameters) = effect.analog_tv_parameters() {
                effect_types[index] = 1;
                effect_mixes[index] = effect.mix.clamp(0.0, 1.0);
                effect_parameters[index] = parameters.as_array();
                effect_seeds[index] = effect_seed(output_id, effect.seed, index);
            } else if let Some(parameters) = effect.digital_tv_parameters() {
                let values = parameters.as_array();
                effect_types[index] = 2;
                effect_mixes[index] = effect.mix.clamp(0.0, 1.0);
                effect_parameters[index].copy_from_slice(&values[..4]);
                effect_parameter_tail[index] = values[4];
                effect_seeds[index] = effect_seed(output_id, effect.seed, index);
            } else if let Some(parameters) = effect.blur_parameters() {
                effect_types[index] = 3;
                effect_mixes[index] = effect.mix.clamp(0.0, 1.0);
                effect_parameters[index][0] = parameters.amount;
            } else if let Some(parameters) = effect.kaleidoscope_parameters() {
                effect_types[index] = 4;
                effect_mixes[index] = effect.mix.clamp(0.0, 1.0);
                effect_parameters[index][0] = f32::from(parameters.repetitions);
                effect_parameters[index][1] = parameters.angle_degrees;
            } else if let Some(parameters) = effect.rasterize_parameters() {
                effect_types[index] = 5;
                effect_mixes[index] = effect.mix.clamp(0.0, 1.0);
                effect_parameters[index][0] = parameters.mode.parameter();
                effect_parameters[index][1] = parameters.dot_size;
            } else if let Some(parameters) = effect.beat_scan_parameters() {
                effect_types[index] = 6;
                effect_mixes[index] = effect.mix.clamp(0.0, 1.0);
                effect_parameters[index] = parameters.as_array();
                for (event, values) in effect.parameters[4..].chunks_exact(2).take(16).enumerate() {
                    beat_scan_positions[event][index] = values[0];
                    beat_scan_counts[event][index] = values[1];
                }
            }
        }
        Self {
            center: [transform.center.x, transform.center.y],
            size: [transform.size.0, transform.size.1],
            rotation: [cos, sin],
            output: [output.width as f32, output.height as f32],
            // Layer dimmer becomes the alpha of the layer tint.
            tint: [
                layer.tint.red,
                layer.tint.green,
                layer.tint.blue,
                layer.dimmer,
            ],
            controls: [layer.grayscale, 0.0, 0.0, 0.0],
            // A mask that is selected but not loaded reports no opacity, so the layer draws
            // unmasked rather than vanishing while its mask is on its way.
            mask: [
                layer.mask.scale_x,
                layer.mask.scale_y,
                f32::from(u8::from(layer.mask.invert)),
                if has_mask && layer.mask.is_active() {
                    layer.mask.opacity
                } else {
                    0.0
                },
            ],
            mask_source: [
                f32::from(u8::from(layer.mask.source == MaskSource::Alpha)),
                0.0,
                0.0,
                0.0,
            ],
            effect_types,
            effect_mixes,
            effect_parameters,
            effect_parameter_tail,
            effect_seeds,
            effect_clock: [
                (now.as_micros() as f64 / 1_000_000.0) as f32,
                output.width as f32,
                output.height as f32,
                0.0,
            ],
            beat_scan_positions,
            beat_scan_counts,
        }
    }
}

fn effect_seed(output: OutputId, seed: u32, slot: usize) -> f32 {
    let mut hash = 2_166_136_261_u32;
    for byte in output
        .as_uuid()
        .as_bytes()
        .iter()
        .copied()
        .chain(seed.to_le_bytes())
        .chain((slot as u32).to_le_bytes())
    {
        hash = (hash ^ u32::from(byte)).wrapping_mul(16_777_619);
    }
    (hash & 0x00ff_ffff) as f32 / 0x00ff_ffff as f32
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MasterUniform {
    tint: [f32; 4],
    flip: [f32; 2],
    mask: [f32; 2],
}

impl MasterUniform {
    fn new(master: &MasterState, mask: Option<&SourceTexture>) -> Self {
        let (horizontal, vertical) = geometry::flip_signs(master.flip_mirror);
        Self {
            tint: [
                master.tint.red,
                master.tint.green,
                master.tint.blue,
                master.dimmer,
            ],
            flip: [horizontal, vertical],
            // The output-level mask is a library mask, so it reads luminance: an operator paints
            // one in white on black and expects white to pass.
            mask: [
                if mask.is_some() && master.has_mask() {
                    1.0
                } else {
                    0.0
                },
                0.0,
            ],
        }
    }
}

/// One output's GPU pipelines and its program target.
pub struct Compositor {
    gpu: Gpu,
    size: Size,
    program: wgpu::Texture,
    program_view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    layer_pipeline: wgpu::RenderPipeline,
    layer_layout: wgpu::BindGroupLayout,
    layer_uniforms: Vec<wgpu::Buffer>,
    master_pipeline: wgpu::RenderPipeline,
    master_layout: wgpu::BindGroupLayout,
    master_uniform: wgpu::Buffer,
    feedback: FeedbackProcessor,
    /// Stands in wherever a mask is not selected. Opaque white: read as luminance or as alpha it
    /// says "let everything through", so a shader needs no branch for the common case.
    no_mask: SourceTexture,
}

impl Compositor {
    pub fn new(gpu: &Gpu, size: Size, output_format: wgpu::TextureFormat) -> Self {
        let device = &gpu.device;

        let layer_layout = uniform_and_texture_layout(device, "media-layer");
        let master_layout = uniform_and_texture_layout(device, "media-master");

        let layer_pipeline = pipeline(
            device,
            "media-layer",
            &layer_layout,
            include_str!("shaders/layer.wgsl"),
            PROGRAM_FORMAT,
            Some(wgpu::BlendState::ALPHA_BLENDING),
        );
        let master_pipeline = pipeline(
            device,
            "media-master",
            &master_layout,
            include_str!("shaders/master.wgsl"),
            output_format,
            None,
        );

        let layer_uniforms = (0..MAX_LAYERS)
            .map(|index| {
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(&format!("media-layer-{index}")),
                    size: std::mem::size_of::<LayerUniform>() as u64,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            })
            .collect();

        let master_uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("media-master"),
            size: std::mem::size_of::<MasterUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let (program, program_view) = program_target(device, size);
        let no_mask = SourceTexture::solid(gpu, Size::new(1, 1), [255, 255, 255, 255])
            .expect("a one-pixel white texture is within every adapter's limits");

        Self {
            gpu: gpu.clone(),
            size,
            program,
            program_view,
            sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("media-source"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            layer_pipeline,
            layer_layout,
            layer_uniforms,
            master_pipeline,
            master_layout,
            master_uniform,
            feedback: FeedbackProcessor::new(gpu),
            no_mask,
        }
    }

    pub const fn size(&self) -> Size {
        self.size
    }

    /// Rebuilds the program target for a new resolution.
    ///
    /// Only this output is affected. A monitor change, a refresh-rate change, sleep and wake, or
    /// a lost surface recreate one output; the others keep presenting.
    pub fn resize(&mut self, size: Size) {
        if size == self.size || size.is_empty() {
            return;
        }
        let (program, view) = program_target(&self.gpu.device, size);
        self.program = program;
        self.program_view = view;
        self.size = size;
    }

    /// Composites one frame onto `target`.
    ///
    /// Layers draw lowest first, so layer 8 lands above layer 1 wherever it is opaque. A layer
    /// that does not draw — dimmer at zero, nothing selected, or a source that failed to load —
    /// contributes nothing rather than contributing black.
    pub fn render(
        &mut self,
        layers: &[LayerDraw<'_>],
        master: &MasterState,
        master_mask: Option<&SourceTexture>,
        target: &wgpu::TextureView,
        output_id: OutputId,
        now: Timestamp,
    ) {
        let device = &self.gpu.device;
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("media-frame"),
        });
        self.feedback.advance(&mut encoder, layers, now);

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("media-layers"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.program_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // Transparent black: an output with no layers shows nothing, and a
                        // preview of it is honest rather than an error card.
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.layer_pipeline);

            for (index, layer) in layers.iter().take(MAX_LAYERS).enumerate() {
                if !layer.state.draws() {
                    continue;
                }
                let uniform = LayerUniform::new(
                    layer.state,
                    layer.source.size(),
                    self.size,
                    layer.mask.is_some(),
                    output_id,
                    now,
                );
                self.gpu.queue.write_buffer(
                    &self.layer_uniforms[index],
                    0,
                    bytemuck::bytes_of(&uniform),
                );

                // The texture a layer samples changes whenever its source does, so the group is
                // built per frame. Eight small groups is a rounding error next to the draw; the
                // video slice can cache them per session if measurement says otherwise.
                let group = bind_group(
                    device,
                    &self.layer_layout,
                    &self.layer_uniforms[index],
                    self.feedback.source(layer),
                    &self.sampler,
                    &layer.mask.unwrap_or(&self.no_mask).view,
                );
                pass.set_bind_group(0, &group, &[]);
                pass.draw(0..6, 0..1);
            }
        }

        self.master_pass(&mut encoder, master, master_mask, target);
        self.gpu.queue.submit([encoder.finish()]);
    }

    /// Runs the master pass again into a second target.
    ///
    /// This is how a CITP preview is taken: the composite is already on the GPU, so a smaller
    /// target gives a filtered scale-down for free rather than costing a CPU resample of a
    /// full-size readback. It happens only when a console is subscribed.
    pub fn render_master_into(
        &mut self,
        master: &MasterState,
        master_mask: Option<&SourceTexture>,
        target: &wgpu::TextureView,
    ) {
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("media-preview"),
            });
        self.master_pass(&mut encoder, master, master_mask, target);
        self.gpu.queue.submit([encoder.finish()]);
    }

    fn master_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        master: &MasterState,
        master_mask: Option<&SourceTexture>,
        target: &wgpu::TextureView,
    ) {
        let device = &self.gpu.device;
        self.gpu.queue.write_buffer(
            &self.master_uniform,
            0,
            bytemuck::bytes_of(&MasterUniform::new(master, master_mask)),
        );
        let group = bind_group(
            device,
            &self.master_layout,
            &self.master_uniform,
            &self.program_view,
            &self.sampler,
            &master_mask.unwrap_or(&self.no_mask).view,
        );
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("media-master"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.master_pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.draw(0..6, 0..1);
    }
}

fn program_target(device: &wgpu::Device, size: Size) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("media-program"),
        size: wgpu::Extent3d {
            width: size.width.max(1),
            height: size.height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: PROGRAM_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

fn uniform_and_texture_layout(device: &wgpu::Device, label: &str) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
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
            // The mask. Always bound — a layer without one gets the white stand-in.
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
        ],
    })
}

fn bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    uniform: &wgpu::Buffer,
    texture: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    mask: &wgpu::TextureView,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(texture),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(mask),
            },
        ],
    })
}

fn pipeline(
    device: &wgpu::Device,
    label: &str,
    layout: &wgpu::BindGroupLayout,
    source: &str,
    format: wgpu::TextureFormat,
    blend: Option<wgpu::BlendState>,
) -> wgpu::RenderPipeline {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &[Some(layout)],
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
            entry_point: Some("fragment"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend,
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

#[cfg(test)]
mod tests {
    use super::*;
    use media_domain::{ScalingMode, Tint};

    #[test]
    fn the_layer_uniform_matches_the_geometry_the_domain_computed() {
        let layer = LayerState {
            scale_x: 2.0,
            rotation: 90.0,
            scaling_mode: ScalingMode::Original,
            dimmer: 0.5,
            tint: Tint::new(1.0, 0.0, 0.0),
            grayscale: 0.25,
            ..Default::default()
        };
        let source = Size::new(100, 50);
        let output = Size::new(1920, 1080);
        let uniform = LayerUniform::new(
            &layer,
            source,
            output,
            false,
            OutputId::default(),
            Timestamp::ZERO,
        );
        let transform = layer_transform(&layer, source, output);

        assert_eq!(uniform.center, [transform.center.x, transform.center.y]);
        assert_eq!(uniform.size, [transform.size.0, transform.size.1]);
        assert_eq!(uniform.output, [1920.0, 1080.0]);
        assert!(
            (uniform.rotation[0] - 0.0).abs() < 1e-6,
            "cosine of a quarter turn"
        );
        assert!(
            (uniform.rotation[1] - 1.0).abs() < 1e-6,
            "sine of a quarter turn"
        );
        assert_eq!(
            uniform.tint,
            [1.0, 0.0, 0.0, 0.5],
            "dimmer rides in the tint's alpha"
        );
        assert_eq!(uniform.controls[0], 0.25);
    }

    #[test]
    fn the_uniforms_are_the_size_the_shaders_declare() {
        assert_eq!(std::mem::size_of::<LayerUniform>(), 240);
        assert_eq!(std::mem::size_of::<MasterUniform>(), 32);
    }

    #[test]
    fn the_master_uniform_carries_the_flip_as_a_per_axis_sign() {
        let master = MasterState {
            flip_mirror: media_domain::FlipMirror::Horizontal,
            dimmer: 0.75,
            ..Default::default()
        };
        let uniform = MasterUniform::new(&master, None);
        assert_eq!(uniform.flip, [-1.0, 1.0]);
        assert_eq!(uniform.tint[3], 0.75);
    }
}
