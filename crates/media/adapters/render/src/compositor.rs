//! The layer compositor.
//!
//! Layers draw into a program target in order, lowest first, with normal alpha blending. The
//! master pass then tints, dims, and flips that finished composite onto the output.

use media_domain::display_region::DisplayRegion;
use media_domain::geometry::Size;
use media_domain::{BlendMode, LayerState, MasterState, OutputId, Timestamp, strobe_lit};

use crate::feedback::FeedbackProcessor;
use crate::gpu::Gpu;
use crate::texture::SourceTexture;

mod blend;
mod model_mapping;
mod pipelines;
mod uniforms;
pub use model_mapping::ModelGeometries;

use blend::{backdrop_layout, backdrop_region, backdrop_target, blend_pipeline};
use pipelines::{bind_group, layer_pass, pipeline, program_target, uniform_and_texture_layout};
use uniforms::{LayerUniform, MasterUniform};

/// The most layers one output composites. The eight-layer personality is the larger of the two
/// supported products.
pub const MAX_LAYERS: usize = 8;

/// The program target's format. Linear rather than sRGB, so a reference render is byte-identical
/// wherever it runs.
pub const PROGRAM_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// One layer to draw this frame.
#[derive(Clone, Copy)]
pub struct LayerDraw<'a> {
    pub state: &'a LayerState,
    pub source: &'a SourceTexture,
    /// The layer's mask, when its address resolved to one. A mask that is selected but has not
    /// loaded is `None`, and a missing mask means no mask — never a black layer.
    pub mask: Option<&'a SourceTexture>,
}

/// One output's GPU pipelines and its program target.
pub struct Compositor {
    gpu: Gpu,
    size: Size,
    program: wgpu::Texture,
    program_view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    layer_pipeline: wgpu::RenderPipeline,
    overlay_pipeline: wgpu::RenderPipeline,
    layer_layout: wgpu::BindGroupLayout,
    layer_uniforms: Vec<wgpu::Buffer>,
    /// Draws a layer whose blend mode is not Normal. It replaces the pixels under the layer with
    /// the finished blend, reading what is below from [`Compositor::backdrop`].
    blend_pipeline: wgpu::RenderPipeline,
    blend_layout: wgpu::BindGroupLayout,
    blend_uniforms: Vec<wgpu::Buffer>,
    /// A copy of the program target taken just before a blended layer draws. Only the layer's
    /// screen bounding box is copied, and only for layers that are not Normal, so an output of
    /// Normal layers pays nothing for blend modes.
    backdrop: wgpu::Texture,
    backdrop_view: wgpu::TextureView,
    /// A transient operator overlay is not one of the eight authored media layers. Keeping its
    /// uniform separate means a full eight-layer output can still explain how to leave full
    /// screen without displacing show content.
    overlay_uniform: wgpu::Buffer,
    master_pipeline: wgpu::RenderPipeline,
    master_layout: wgpu::BindGroupLayout,
    master_uniform: wgpu::Buffer,
    feedback: FeedbackProcessor,
    /// Stands in wherever a mask is not selected. Opaque white: read as luminance or as alpha it
    /// says "let everything through", so a shader needs no branch for the common case.
    no_mask: SourceTexture,
    /// Layers mapped onto 3D models. See [`model_mapping`].
    models: model_mapping::ModelMapper,
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
        let overlay_pipeline = pipeline(
            device,
            "media-operator-overlay",
            &layer_layout,
            include_str!("shaders/layer.wgsl"),
            output_format,
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
        let blend_layout = backdrop_layout(device);
        let blend_pipeline = blend_pipeline(device, &layer_layout, &blend_layout);
        let blend_uniforms = (0..MAX_LAYERS)
            .map(|index| {
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(&format!("media-layer-blend-{index}")),
                    size: std::mem::size_of::<[u32; 4]>() as u64,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            })
            .collect();
        let (backdrop, backdrop_view) = backdrop_target(device, size);
        let overlay_uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("media-operator-overlay"),
            size: std::mem::size_of::<LayerUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

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
            overlay_pipeline,
            layer_layout,
            layer_uniforms,
            blend_pipeline,
            blend_layout,
            blend_uniforms,
            backdrop,
            backdrop_view,
            overlay_uniform,
            master_pipeline,
            master_layout,
            master_uniform,
            feedback: FeedbackProcessor::new(gpu),
            no_mask,
            models: model_mapping::ModelMapper::new(gpu),
        }
    }

    /// Makes exactly these 3D models available to this output's layers. Returns the slots that
    /// could not be uploaded and why; a layer selecting one draws flat.
    pub fn set_models(&mut self, models: &ModelGeometries) -> Vec<(u8, String)> {
        self.models.install(&self.gpu, models)
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
        let (backdrop, backdrop_view) = backdrop_target(&self.gpu.device, size);
        self.backdrop = backdrop;
        self.backdrop_view = backdrop_view;
        self.size = size;
    }

    /// Composites one frame onto `target`.
    ///
    /// Layers draw lowest first, so layer 8 lands above layer 1 wherever it is opaque. A layer
    /// that does not draw — dimmer at zero, nothing selected, or a source that failed to load —
    /// contributes nothing rather than contributing black.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        layers: &[LayerDraw<'_>],
        master: &MasterState,
        master_mask: Option<&SourceTexture>,
        target: &wgpu::TextureView,
        output_id: OutputId,
        now: Timestamp,
        region: Option<&DisplayRegion>,
    ) {
        self.render_internal(
            layers,
            master,
            master_mask,
            target,
            output_id,
            now,
            false,
            true,
            region,
            None,
        );
    }

    /// Composites the authored layers and then a transient operator-only overlay.
    ///
    /// The overlay is deliberately outside [`MAX_LAYERS`]: it must remain visible when all eight
    /// media layers are occupied, and it never participates in source feedback.
    #[allow(clippy::too_many_arguments)]
    pub fn render_with_overlay(
        &mut self,
        layers: &[LayerDraw<'_>],
        master: &MasterState,
        master_mask: Option<&SourceTexture>,
        target: &wgpu::TextureView,
        output_id: OutputId,
        now: Timestamp,
        region: Option<&DisplayRegion>,
        overlay: LayerDraw<'_>,
    ) {
        self.render_internal(
            layers,
            master,
            master_mask,
            target,
            output_id,
            now,
            false,
            true,
            region,
            Some(overlay),
        );
    }

    /// Renders one layer exactly as Program does, while retaining its transparency for a layer
    /// preview instead of flattening it onto Program's black output background.
    pub fn render_layer_preview(
        &mut self,
        layer: LayerDraw<'_>,
        target: &wgpu::TextureView,
        output_id: OutputId,
        now: Timestamp,
    ) {
        self.render_internal(
            &[layer],
            &MasterState::default(),
            None,
            target,
            output_id,
            now,
            true,
            false,
            None,
            None,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn render_internal(
        &mut self,
        layers: &[LayerDraw<'_>],
        master: &MasterState,
        master_mask: Option<&SourceTexture>,
        target: &wgpu::TextureView,
        output_id: OutputId,
        now: Timestamp,
        preserve_alpha: bool,
        advance_feedback: bool,
        region: Option<&DisplayRegion>,
        overlay: Option<LayerDraw<'_>>,
    ) {
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("media-frame"),
            });
        if advance_feedback {
            self.feedback.advance(&mut encoder, layers, now);
        }
        // A layer mapped onto a 3D model renders its look onto the mesh first. From here on it is
        // an ordinary flat layer, so blend mode, strobe, and opacity apply exactly as they do to
        // any other layer.
        self.prepare_models(&mut encoder, layers, output_id, now);
        let mapped_layers = self.models.substitute(layers);
        self.draw_layers(
            &mut encoder,
            mapped_layers.as_slice(),
            output_id,
            now,
            preserve_alpha,
        );
        self.master_pass(
            &mut encoder,
            master,
            master_mask,
            target,
            preserve_alpha,
            region,
        );
        if let Some(overlay) = overlay.filter(|overlay| overlay.state.draws()) {
            self.overlay_pass(&mut encoder, target, output_id, now, overlay);
        }
        self.gpu.queue.submit([encoder.finish()]);
    }

    /// Renders every model-mapped layer's look onto its mesh, ready for
    /// [`model_mapping::ModelMapper::substitute`] to swap in the flat model render.
    fn prepare_models(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        layers: &[LayerDraw<'_>],
        output_id: OutputId,
        now: Timestamp,
    ) {
        self.models.prepare(
            &model_mapping::MappingContext {
                gpu: &self.gpu,
                look_pipeline: &self.layer_pipeline,
                layer_layout: &self.layer_layout,
                sampler: &self.sampler,
                feedback: &self.feedback,
                no_mask: &self.no_mask,
                output: self.size,
                output_id,
                now,
            },
            encoder,
            layers,
        );
    }

    /// Draws the layers into the program target, lowest first.
    ///
    /// Normal layers share one pass. A blended layer ends it, copies what is below it out of the
    /// program target, and opens a new pass that later Normal layers continue in.
    fn draw_layers(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        layers: &[LayerDraw<'_>],
        output_id: OutputId,
        now: Timestamp,
        preserve_alpha: bool,
    ) {
        // Transparent black first: an output with no layers shows nothing, and a preview of it is
        // honest rather than an error card.
        let mut load = wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT);
        let mut pass: Option<wgpu::RenderPass<'static>> = None;
        // The clock the effects read, so a strobe and an effect agree on the instant.
        let seconds = now.as_micros() as f64 / 1_000_000.0;

        for (index, layer) in layers.iter().take(MAX_LAYERS).enumerate() {
            // An unlit strobe half period contributes nothing, exactly like a layer that does not
            // draw.
            if !layer.state.draws() || !strobe_lit(layer.state.strobe_hz, seconds) {
                continue;
            }
            // A layer preview shows the layer alone, so there is nothing to blend against.
            let blend = if preserve_alpha {
                BlendMode::Normal
            } else {
                layer.state.blend
            };
            let backdrop = match blend {
                BlendMode::Normal => None,
                _ => match backdrop_region(layer.state, layer.source.size(), self.size) {
                    Some(region) => Some(region),
                    // Entirely off the output: the layer covers no pixel.
                    None => continue,
                },
            };
            let group = self.layer_bind_group(index, layer, output_id, now);

            let Some(region) = backdrop else {
                let pass =
                    pass.get_or_insert_with(|| layer_pass(encoder, &self.program_view, &mut load));
                pass.set_pipeline(&self.layer_pipeline);
                pass.set_bind_group(0, &group, &[]);
                pass.draw(0..6, 0..1);
                continue;
            };

            drop(pass.take());
            let backdrop_group = self.capture_backdrop(encoder, &mut load, index, blend, region);
            let pass = pass.insert(layer_pass(encoder, &self.program_view, &mut load));
            pass.set_pipeline(&self.blend_pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.set_bind_group(1, &backdrop_group, &[]);
            pass.draw(0..6, 0..1);
        }
        if pass.is_none() && matches!(load, wgpu::LoadOp::Clear(_)) {
            drop(layer_pass(encoder, &self.program_view, &mut load));
        }
    }

    /// Writes one layer's uniform into its slot and binds it with the texture and mask it samples.
    fn layer_bind_group(
        &self,
        index: usize,
        layer: &LayerDraw<'_>,
        output_id: OutputId,
        now: Timestamp,
    ) -> wgpu::BindGroup {
        let uniform = LayerUniform::new(
            layer.state,
            layer.source.size(),
            self.size,
            layer.mask,
            output_id,
            now,
        );
        self.gpu
            .queue
            .write_buffer(&self.layer_uniforms[index], 0, bytemuck::bytes_of(&uniform));

        // The texture a layer samples changes whenever its source does, so the group is built
        // per frame. Eight small groups is a rounding error next to the draw; the video slice can
        // cache them per session if measurement says otherwise.
        bind_group(
            &self.gpu.device,
            &self.layer_layout,
            &self.layer_uniforms[index],
            self.feedback.source(layer),
            &self.sampler,
            &layer.mask.unwrap_or(&self.no_mask).view,
        )
    }

    /// Copies the program-target pixels under a blended layer into the backdrop and binds that
    /// backdrop with the layer's blend mode. No layer pass may be open while this runs.
    fn capture_backdrop(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        load: &mut wgpu::LoadOp<wgpu::Color>,
        index: usize,
        blend: BlendMode,
        region: (u32, u32, u32, u32),
    ) -> wgpu::BindGroup {
        if matches!(load, wgpu::LoadOp::Clear(_)) {
            // Nothing has cleared the program yet; the backdrop must not read last frame.
            drop(layer_pass(encoder, &self.program_view, load));
        }
        let origin = wgpu::Origin3d {
            x: region.0,
            y: region.1,
            z: 0,
        };
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.program,
                mip_level: 0,
                origin,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &self.backdrop,
                mip_level: 0,
                origin,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: region.2,
                height: region.3,
                depth_or_array_layers: 1,
            },
        );
        self.gpu.queue.write_buffer(
            &self.blend_uniforms[index],
            0,
            bytemuck::bytes_of(&[blend.index(), 0, 0, 0]),
        );
        self.gpu
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("media-layer-backdrop"),
                layout: &self.blend_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&self.backdrop_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: self.blend_uniforms[index].as_entire_binding(),
                    },
                ],
            })
    }

    fn overlay_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        output_id: OutputId,
        now: Timestamp,
        overlay: LayerDraw<'_>,
    ) {
        let uniform = LayerUniform::new(
            overlay.state,
            overlay.source.size(),
            self.size,
            overlay.mask,
            output_id,
            now,
        );
        self.gpu
            .queue
            .write_buffer(&self.overlay_uniform, 0, bytemuck::bytes_of(&uniform));
        let group = bind_group(
            &self.gpu.device,
            &self.layer_layout,
            &self.overlay_uniform,
            &overlay.source.view,
            &self.sampler,
            &overlay.mask.unwrap_or(&self.no_mask).view,
        );
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("media-operator-overlay"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.overlay_pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.draw(0..6, 0..1);
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
        // A readback is of the canvas itself, so no screen's slice applies to it.
        self.master_pass(&mut encoder, master, master_mask, target, false, None);
        self.gpu.queue.submit([encoder.finish()]);
    }

    fn master_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        master: &MasterState,
        master_mask: Option<&SourceTexture>,
        target: &wgpu::TextureView,
        preserve_alpha: bool,
        region: Option<&DisplayRegion>,
    ) {
        let device = &self.gpu.device;
        self.gpu.queue.write_buffer(
            &self.master_uniform,
            0,
            bytemuck::bytes_of(&MasterUniform::new(
                master,
                master_mask,
                preserve_alpha,
                region,
            )),
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

#[cfg(test)]
mod tests;
