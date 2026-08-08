//! Rendering generated visualizers.
//!
//! A visualizer is a source: this renders one into an output-sized texture, and the compositor
//! then treats it exactly like a decoded video frame. Nothing downstream knows the difference,
//! which is what makes a visualizer maskable, tintable, and transformable for free.
//!
//! Every visualizer is its own shader module. That costs twenty compilations at startup and buys
//! the thing the contract asks for: when a backend cannot compile one effect, that effect is
//! reported by name instead of taking the other nineteen down with it.

use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};
use media_domain::audio::{Analysis, BANDS, WAVEFORM_POINTS};
use media_domain::geometry::Size;
use media_domain::visualizer::{ALL_KINDS, VisualizerKind, VisualizerParameters};

use crate::gpu::Gpu;
use crate::texture::{SourceTexture, TextureError};

const PRELUDE: &str = include_str!("shaders/visualizers/prelude.wgsl");

/// The source of one kind, ready to be appended to the prelude.
const fn body(kind: VisualizerKind) -> &'static str {
    match kind {
        VisualizerKind::EqualizerBars => include_str!("shaders/visualizers/equalizer-bars.wgsl"),
        VisualizerKind::WaveformOscilloscope => {
            include_str!("shaders/visualizers/waveform-oscilloscope.wgsl")
        }
        VisualizerKind::CircularSpectrum => {
            include_str!("shaders/visualizers/circular-spectrum.wgsl")
        }
        VisualizerKind::WaveTerrain => include_str!("shaders/visualizers/wave-terrain.wgsl"),
        VisualizerKind::PulsingCircles => include_str!("shaders/visualizers/pulsing-circles.wgsl"),
        VisualizerKind::MorphingPolygon => {
            include_str!("shaders/visualizers/morphing-polygon.wgsl")
        }
        VisualizerKind::MinimalistShapes => {
            include_str!("shaders/visualizers/minimalist-shapes.wgsl")
        }
        VisualizerKind::Kaleidoscope => include_str!("shaders/visualizers/kaleidoscope.wgsl"),
        VisualizerKind::BeatExplosions => include_str!("shaders/visualizers/beat-explosions.wgsl"),
        VisualizerKind::DancingSwarm => include_str!("shaders/visualizers/dancing-swarm.wgsl"),
        VisualizerKind::Starfield => include_str!("shaders/visualizers/starfield.wgsl"),
        VisualizerKind::LightningTendrils => {
            include_str!("shaders/visualizers/lightning-tendrils.wgsl")
        }
        VisualizerKind::RadiatingRays => include_str!("shaders/visualizers/radiating-rays.wgsl"),
        VisualizerKind::StrobeFlash => include_str!("shaders/visualizers/strobe-flash.wgsl"),
        VisualizerKind::ColorCycling => include_str!("shaders/visualizers/color-cycling.wgsl"),
        VisualizerKind::CrossingLines => include_str!("shaders/visualizers/crossing-lines.wgsl"),
        VisualizerKind::DigitalGlitch => include_str!("shaders/visualizers/digital-glitch.wgsl"),
        VisualizerKind::CrtScanline => include_str!("shaders/visualizers/crt-scanline.wgsl"),
        VisualizerKind::RotatingShape => include_str!("shaders/visualizers/rotating-shape.wgsl"),
        VisualizerKind::FractalMorph => include_str!("shaders/visualizers/fractal-morph.wgsl"),
    }
}

/// The whole source of one visualizer.
pub fn shader_source(kind: VisualizerKind) -> String {
    format!("{PRELUDE}\n{}", body(kind))
}

/// What a visualizer knows about this instant.
#[derive(Debug, Clone, Copy)]
pub struct VisualizerFrame<'a> {
    /// Seconds since the server started. Continuous, so an animation never jumps.
    pub seconds: f32,
    pub analysis: &'a Analysis,
    /// `1.0` on the frame a beat lands, falling back toward zero after it.
    pub beat: f32,
    pub bpm: f32,
    /// Where this instant sits between beats, `0.0..1.0`.
    pub beat_phase: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct VisualizerUniform {
    resolution: [f32; 4],
    audio0: [f32; 4],
    audio1: [f32; 4],
    primary: [f32; 4],
    secondary: [f32; 4],
    params0: [f32; 4],
    params1: [f32; 4],
    params2: [f32; 4],
    params3: [f32; 4],
    flags: [f32; 4],
}

impl VisualizerUniform {
    fn new(size: Size, parameters: &VisualizerParameters, frame: &VisualizerFrame<'_>) -> Self {
        let parameters = parameters.clamped();
        let analysis = frame.analysis;
        Self {
            resolution: [
                size.width as f32,
                size.height as f32,
                size.width as f32 / size.height.max(1) as f32,
                frame.seconds,
            ],
            audio0: [
                analysis.bass,
                analysis.mid,
                analysis.treble,
                analysis.energy,
            ],
            audio1: [analysis.peak, frame.beat, frame.bpm, frame.beat_phase],
            primary: [
                parameters.primary.red,
                parameters.primary.green,
                parameters.primary.blue,
                1.0,
            ],
            secondary: [
                parameters.secondary.red,
                parameters.secondary.green,
                parameters.secondary.blue,
                1.0,
            ],
            params0: [
                parameters.count as f32,
                parameters.size,
                parameters.speed,
                parameters.amount,
            ],
            params1: [
                parameters.radius,
                parameters.thickness,
                parameters.reactivity,
                parameters.decay,
            ],
            params2: [
                parameters.zoom,
                parameters.iterations as f32,
                parameters.threshold,
                parameters.smoothing,
            ],
            params3: [
                parameters.gravity,
                parameters.lifetime,
                parameters.curvature,
                f32::from(parameters.mode),
            ],
            flags: [
                f32::from(u8::from(parameters.mirror)),
                f32::from(u8::from(parameters.filled)),
                f32::from(u8::from(parameters.wireframe)),
                0.0,
            ],
        }
    }
}

/// Why a visualizer cannot be drawn on this machine.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VisualizerError {
    #[error("{kind} could not be compiled by this graphics backend: {detail}")]
    Compilation { kind: String, detail: String },
    #[error(transparent)]
    Texture(#[from] TextureError),
}

/// One output's visualizer pipelines and per-layer targets.
pub struct VisualizerRenderer {
    gpu: Gpu,
    size: Size,
    layout: wgpu::BindGroupLayout,
    pipelines: HashMap<VisualizerKind, wgpu::RenderPipeline>,
    uniform: wgpu::Buffer,
    analysis: wgpu::Texture,
    analysis_view: wgpu::TextureView,
    targets: HashMap<usize, SourceTexture>,
}

impl VisualizerRenderer {
    pub fn new(gpu: &Gpu, size: Size) -> Self {
        let device = &gpu.device;
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("media-visualizer"),
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
                // Read with `textureLoad`, never sampled, so no filtering support is required of
                // a 32-bit float texture on any backend.
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("media-visualizer"),
            size: std::mem::size_of::<VisualizerUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let analysis = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("media-analysis"),
            size: wgpu::Extent3d {
                width: WAVEFORM_POINTS as u32,
                height: 2,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let analysis_view = analysis.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            gpu: gpu.clone(),
            size,
            layout,
            pipelines: HashMap::new(),
            uniform,
            analysis,
            analysis_view,
            targets: HashMap::new(),
        }
    }

    pub fn resize(&mut self, size: Size) {
        if size == self.size || size.is_empty() {
            return;
        }
        self.size = size;
        // Targets are output-sized by definition, so they are all stale at once.
        self.targets.clear();
    }

    /// Compiles every visualizer and reports each one's outcome.
    ///
    /// Called at startup so a backend that cannot build an effect says which, rather than an
    /// operator selecting an address and getting nothing.
    pub fn validate(&mut self) -> Vec<(VisualizerKind, Result<(), VisualizerError>)> {
        ALL_KINDS
            .into_iter()
            .map(|kind| (kind, self.ensure_pipeline(kind).map(|_| ())))
            .collect()
    }

    /// Draws one visualizer into the texture belonging to `layer`.
    pub fn render(
        &mut self,
        layer: usize,
        kind: VisualizerKind,
        parameters: &VisualizerParameters,
        frame: &VisualizerFrame<'_>,
    ) -> Result<&SourceTexture, VisualizerError> {
        self.ensure_pipeline(kind)?;
        self.upload_analysis(frame.analysis);
        self.gpu.queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::bytes_of(&VisualizerUniform::new(self.size, parameters, frame)),
        );

        if !self.targets.contains_key(&layer) {
            let target = SourceTexture::render_target(&self.gpu, self.size)?;
            self.targets.insert(layer, target);
        }

        let device = &self.gpu.device;
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("media-visualizer"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.analysis_view),
                },
            ],
        });

        let target = &self.targets[&layer];
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("media-visualizer"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("media-visualizer"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // Transparent, so a visualizer that draws only shapes composites over
                        // whatever is beneath it instead of blacking the layers below out.
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipelines[&kind]);
            pass.set_bind_group(0, &group, &[]);
            pass.draw(0..6, 0..1);
        }
        self.gpu.queue.submit([encoder.finish()]);

        Ok(&self.targets[&layer])
    }

    /// One layer's most recently rendered visualizer, if it has ever rendered one.
    pub fn target(&self, layer: usize) -> Option<&SourceTexture> {
        self.targets.get(&layer)
    }

    fn ensure_pipeline(&mut self, kind: VisualizerKind) -> Result<(), VisualizerError> {
        if self.pipelines.contains_key(&kind) {
            return Ok(());
        }
        let device = &self.gpu.device;
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(kind.label()),
            source: wgpu::ShaderSource::Wgsl(shader_source(kind).into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(kind.label()),
            bind_group_layouts: &[Some(&self.layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(kind.label()),
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
                    format: crate::texture::VISUALIZER_FORMAT,
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
        });

        if let Some(error) = pollster::block_on(scope.pop()) {
            return Err(VisualizerError::Compilation {
                kind: kind.label().to_owned(),
                detail: error.to_string(),
            });
        }
        self.pipelines.insert(kind, pipeline);
        Ok(())
    }

    /// Puts the newest analysis where the shaders can read it.
    fn upload_analysis(&self, analysis: &Analysis) {
        let mut rows = vec![0.0f32; WAVEFORM_POINTS * 2];
        for (slot, value) in rows[..WAVEFORM_POINTS]
            .iter_mut()
            .zip(analysis.waveform.iter())
        {
            *slot = *value;
        }
        for (slot, value) in rows[WAVEFORM_POINTS..WAVEFORM_POINTS + BANDS]
            .iter_mut()
            .zip(analysis.spectrum.iter())
        {
            *slot = *value;
        }

        self.gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.analysis,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(&rows),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(WAVEFORM_POINTS as u32 * 4),
                rows_per_image: Some(2),
            },
            wgpu::Extent3d {
                width: WAVEFORM_POINTS as u32,
                height: 2,
                depth_or_array_layers: 1,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use media_domain::Tint;

    #[test]
    fn every_visualizer_has_its_own_shader_and_declares_one_shade_function() {
        for kind in ALL_KINDS {
            let source = shader_source(kind);
            assert!(
                source.contains("fn shade("),
                "{} contributes no shade function",
                kind.label()
            );
            assert!(
                source.contains("@fragment"),
                "{} lost the shared entry point",
                kind.label()
            );
        }
    }

    #[test]
    fn no_two_visualizers_share_a_shader_body() {
        // A copy-paste that left two kinds pointing at one file would otherwise ship as twenty
        // entries in a menu and eighteen distinct effects.
        let mut seen = std::collections::HashSet::new();
        for kind in ALL_KINDS {
            assert!(
                seen.insert(body(kind)),
                "{} reuses another visualizer's shader",
                kind.label()
            );
        }
    }

    #[test]
    fn the_uniform_is_the_size_the_prelude_declares() {
        assert_eq!(std::mem::size_of::<VisualizerUniform>(), 160);
    }

    #[test]
    fn the_uniform_clamps_what_a_configuration_file_may_hold() {
        let analysis = Analysis::default();
        let frame = VisualizerFrame {
            seconds: 2.0,
            analysis: &analysis,
            beat: 1.0,
            bpm: 128.0,
            beat_phase: 0.25,
        };
        let parameters = VisualizerParameters {
            count: 0,
            zoom: f32::INFINITY,
            primary: Tint::new(1.0, 0.0, 0.0),
            mirror: true,
            ..Default::default()
        };

        let uniform = VisualizerUniform::new(Size::new(1920, 1080), &parameters, &frame);
        assert_eq!(uniform.params0[0], 1.0, "a count of zero draws nothing");
        assert!(uniform.params2[0].is_finite());
        assert_eq!(uniform.primary[0], 1.0);
        assert_eq!(uniform.flags[0], 1.0);
        assert_eq!(uniform.resolution[3], 2.0);
        assert!((uniform.resolution[2] - 1920.0 / 1080.0).abs() < 1e-6);
    }
}
