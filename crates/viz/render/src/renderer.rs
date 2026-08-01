//! The render core: one frame of the semantic scene into one presented image.

use crate::buffers::{DynamicBuffer, GpuMesh};
use crate::gpu::{Gpu, PresentationSurface};
use crate::instances::{FrameInstances, MeshInstance, MeshKind};
use crate::mesh::{self, Vertex};
use crate::targets::{MAX_LIGHTS_PER_TILE, TILE_SIZE, Targets};
use bytemuck::{Pod, Zeroable};
use std::collections::HashMap;
use viz_scene::{Scene, SceneValues, ViewConfiguration};
use wgpu::BufferUsages;

const COMMON_WGSL: &str = include_str!("shaders/common.wgsl");
const SURFACE_WGSL: &str = include_str!("shaders/surface.wgsl");
const BEAM_WGSL: &str = include_str!("shaders/beam.wgsl");
const LASER_WGSL: &str = include_str!("shaders/laser.wgsl");
const LINES_WGSL: &str = include_str!("shaders/lines.wgsl");
const CULL_WGSL: &str = include_str!("shaders/cull.wgsl");
const POST_WGSL: &str = include_str!("shaders/post.wgsl");
const OVERLAY_WGSL: &str = include_str!("shaders/overlay.wgsl");
const SHADOW_WGSL: &str = include_str!("shaders/shadow.wgsl");
/// Bindings the depth-only pass needs, kept beside it rather than in the shared prelude, which
/// the passes that sample the atlas use instead.
/// The single-sampled scene-depth declaration in `beam.wgsl`, swapped for the multisampled type
/// when the shaded passes are multisampled.
const SCENE_DEPTH_BINDING: &str = "var scene_depth: texture_depth_2d;";

fn scene_depth_binding(samples: u32) -> &'static str {
    if samples > 1 {
        "var scene_depth: texture_depth_multisampled_2d;"
    } else {
        SCENE_DEPTH_BINDING
    }
}

const SHADOW_PRELUDE_WGSL: &str = r#"
struct ShadowDraw { index: u32, pad0: u32, pad1: u32, pad2: u32 };
@group(0) @binding(0) var<storage, read> shadow_matrices: array<mat4x4<f32>>;
@group(0) @binding(1) var<uniform> shadow_draw: ShadowDraw;
fn shadow_index() -> u32 { return shadow_draw.index; }
"#;

/// Edge of the shadow atlas, and how it is divided. Sixteen maps of 512 pixels is enough for the
/// lights an operator is actually looking at, and fits in four megabytes.
const SHADOW_ATLAS_EDGE: u32 = 2048;
const SHADOW_TILE_EDGE: u32 = 512;
const SHADOW_TILES_PER_ROW: u32 = SHADOW_ATLAS_EDGE / SHADOW_TILE_EDGE;
const MAX_SHADOWS: usize = (SHADOW_TILES_PER_ROW * SHADOW_TILES_PER_ROW) as usize;
/// Uniform buffers with a dynamic offset must be aligned; 256 bytes is the portable requirement.
const SHADOW_DRAW_STRIDE: u64 = 256;

/// How much of the blurred highlight pass is added back. Enough to bloom a bright aperture,
/// not enough to wash out a hazy stage.
const BLOOM_MIX: f32 = 0.16;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
struct Globals {
    view_projection: [[f32; 4]; 4],
    view: [[f32; 4]; 4],
    inverse_projection: [[f32; 4]; 4],
    camera_position: [f32; 4],
    screen: [f32; 4],
    params: [f32; 4],
    params2: [f32; 4],
    params3: [f32; 4],
}

/// Why a frame could not be presented.
#[derive(Clone, Debug, PartialEq)]
pub enum RenderError {
    /// The surface is temporarily unavailable; try again next frame.
    SkipFrame,
    /// Recoverable surface or device failure, named for the diagnostics surface.
    Surface(String),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SkipFrame => formatter.write_str("surface unavailable this frame"),
            Self::Surface(detail) => write!(formatter, "surface: {detail}"),
        }
    }
}

impl std::error::Error for RenderError {}

/// One read-back frame.
pub struct CapturedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// What one presented frame cost and how far behind the newest input it is.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameStats {
    pub cpu_micros: u64,
    /// How long this frame waited for the display to release a drawable. The wait happens on the
    /// caller's thread, so an application that also handles input needs to know about it.
    pub acquire_micros: u64,
    pub lights: u32,
    pub beams: u32,
    pub instances: u32,
    pub draw_calls: u32,
    /// Set when the renderer had to reduce quality to stay inside the budget.
    pub degraded: bool,
    /// What the GPU itself spent on a recent frame, where the adapter can time one. This is the
    /// number that says whether there is headroom under a display-limited frame rate.
    pub gpu_micros: Option<u64>,
}

pub struct Renderer {
    gpu: Gpu,
    targets: Targets,
    /// Fraction of the surface the shaded passes are drawn at, from the active quality tier. The
    /// composite samples the result up to the surface, so the overlay and the plan views stay at
    /// the display's own resolution whatever this is.
    target_scale: f32,
    meshes: HashMap<MeshKind, GpuMesh>,
    cone: GpuMesh,
    mesh_instances: HashMap<MeshKind, DynamicBuffer>,
    beam_instances: DynamicBuffer,
    laser_instances: DynamicBuffer,
    line_vertices: DynamicBuffer,
    lights: DynamicBuffer,
    tile_counts: DynamicBuffer,
    tile_lights: DynamicBuffer,
    globals_buffer: wgpu::Buffer,
    post_settings: wgpu::Buffer,
    scene_layout: wgpu::BindGroupLayout,
    cull_layout: wgpu::BindGroupLayout,
    depth_layout: wgpu::BindGroupLayout,
    post_layout: wgpu::BindGroupLayout,
    composite_layout: wgpu::BindGroupLayout,
    scene_bind_group: wgpu::BindGroup,
    cull_bind_group: wgpu::BindGroup,
    depth_bind_group: wgpu::BindGroup,
    shadow_atlas: wgpu::TextureView,
    shadow_pipeline: wgpu::RenderPipeline,
    shadow_layout: wgpu::BindGroupLayout,
    shadow_draw_layout: wgpu::BindGroupLayout,
    shadow_sampler: wgpu::Sampler,
    /// Every piece of gobo artwork in the rig, as one array the lighting passes sample.
    gobo_atlas: crate::gobos::GoboAtlas,
    gobo_sampler: wgpu::Sampler,
    shadow_bind_group: wgpu::BindGroup,
    shadow_draw_group: wgpu::BindGroup,
    shadow_matrices: DynamicBuffer,
    shadow_draws: wgpu::Buffer,
    /// How many lights actually have a map this frame.
    shadow_count: u32,
    haze_view: wgpu::TextureView,
    haze_sampler: wgpu::Sampler,
    bloom_extract_group: wgpu::BindGroup,
    bloom_blur_a_group: wgpu::BindGroup,
    bloom_blur_b_group: wgpu::BindGroup,
    composite_source_group: wgpu::BindGroup,
    composite_bloom_group: wgpu::BindGroup,
    sampler: wgpu::Sampler,
    surface_pipeline: wgpu::RenderPipeline,
    line_pipeline: wgpu::RenderPipeline,
    beam_pipeline: wgpu::RenderPipeline,
    laser_pipeline: wgpu::RenderPipeline,
    cull_pipeline: wgpu::ComputePipeline,
    extract_pipeline: wgpu::RenderPipeline,
    blur_pipeline: wgpu::RenderPipeline,
    composite_pipeline: wgpu::RenderPipeline,
    overlay_pipeline: wgpu::RenderPipeline,
    overlay_bind_group: wgpu::BindGroup,
    overlay_globals: wgpu::Buffer,
    overlay_quads: DynamicBuffer,
    frame: FrameInstances,
    stats: FrameStats,
    /// Set for one frame to redirect the composite into an offscreen target.
    capture_request: Option<wgpu::TextureView>,
    /// Whether the beam pass had to skip volumes to hold the budget.
    beam_overflow: bool,
    /// Times one frame at a time on the GPU, where the adapter supports it.
    timer: Option<crate::timing::GpuTimer>,
    /// Smoothed automatic exposure. A show with a hundred simultaneous beams is physically far
    /// brighter than one with four, so a fixed exposure either crushes the small rig or blows out
    /// the large one. The operator's exposure remains a multiplier on top of this.
    auto_exposure: f32,
    last_frame_at: Option<std::time::Instant>,
}

impl Renderer {
    pub fn new(target: &dyn PresentationSurface) -> Result<Self, String> {
        Self::with_icon(target, None)
    }

    /// Build the renderer with the application icon the overlay draws in its corner. The icon is
    /// `RGBA8`, [`crate::overlay::ICON_SIZE`] square; anything else leaves the corner empty.
    pub fn with_icon(
        target: &dyn PresentationSurface,
        icon: Option<&[u8]>,
    ) -> Result<Self, String> {
        let gpu = Gpu::new(target)?;
        let device = gpu.device.clone();
        // Timed only where the adapter can time a pass; elsewhere there is simply no reading.
        let timer = crate::timing::GpuTimer::new(&gpu.device, &gpu.queue, gpu.timestamps);
        let samples = gpu.samples;
        let targets = Targets::new(&device, gpu.config.width, gpu.config.height, samples);
        let multisample = wgpu::MultisampleState {
            count: samples,
            ..Default::default()
        };

        let mut meshes = HashMap::new();
        for kind in MeshKind::PROCEDURAL {
            let Some((name, data)) = mesh::procedural(kind) else {
                continue;
            };
            meshes.insert(kind, GpuMesh::new(&device, name, &data));
        }
        let cone = GpuMesh::new(&device, "cone", &mesh::unit_cone(28));

        let buffers = SceneBuffers::new(&device, &gpu.queue, &targets);

        // What the shaders may see, what they are compiled from, and the pipelines that result.
        let layouts = Layouts::new(&device, samples);
        let pipeline_layouts = PipelineLayouts::new(&device, &layouts);
        let modules = Modules::new(&device, samples);
        let pipelines = Pipelines::new(
            &device,
            &layouts,
            &pipeline_layouts,
            &modules,
            multisample,
            gpu.format,
        );

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("viz linear"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let shadow = ShadowResources::new(&device);
        let groups = Groups::new(
            &device, &gpu.queue, &layouts, &buffers, &shadow, &targets, &sampler,
        );
        let overlay = OverlayAtlas::new(&device, &gpu.queue, &layouts, icon);

        Ok(Self {
            gpu,
            targets,
            target_scale: 1.0,
            meshes,
            cone,
            mesh_instances: buffers.mesh_instances,
            beam_instances: buffers.beam_instances,
            laser_instances: buffers.laser_instances,
            line_vertices: buffers.line_vertices,
            lights: buffers.lights,
            tile_counts: buffers.tile_counts,
            tile_lights: buffers.tile_lights,
            globals_buffer: buffers.globals,
            post_settings: buffers.post_settings,
            scene_layout: layouts.scene,
            cull_layout: layouts.cull,
            depth_layout: layouts.depth,
            post_layout: layouts.post,
            composite_layout: layouts.composite,
            scene_bind_group: groups.scene,
            cull_bind_group: groups.cull,
            depth_bind_group: groups.depth,
            shadow_atlas: shadow.atlas,
            shadow_pipeline: pipelines.shadow,
            shadow_layout: layouts.shadow,
            shadow_draw_layout: layouts.shadow_draw,
            shadow_sampler: shadow.sampler,
            gobo_atlas: groups.gobo_atlas,
            gobo_sampler: groups.gobo_sampler,
            shadow_bind_group: groups.shadow,
            shadow_draw_group: groups.shadow_draw,
            shadow_matrices: shadow.matrices,
            shadow_draws: shadow.draws,
            shadow_count: 0,
            haze_view: groups.haze_view,
            haze_sampler: groups.haze_sampler,
            bloom_extract_group: groups.bloom_extract,
            bloom_blur_a_group: groups.bloom_blur_a,
            bloom_blur_b_group: groups.bloom_blur_b,
            composite_source_group: groups.composite_source,
            composite_bloom_group: groups.composite_bloom,
            sampler,
            surface_pipeline: pipelines.surface,
            line_pipeline: pipelines.line,
            beam_pipeline: pipelines.beam,
            laser_pipeline: pipelines.laser,
            cull_pipeline: pipelines.cull,
            extract_pipeline: pipelines.extract,
            blur_pipeline: pipelines.blur,
            composite_pipeline: pipelines.composite,
            overlay_pipeline: pipelines.overlay,
            overlay_bind_group: overlay.bind_group,
            overlay_globals: overlay.globals,
            overlay_quads: buffers.overlay_quads,
            frame: FrameInstances::default(),
            stats: FrameStats::default(),
            capture_request: None,
            beam_overflow: false,
            timer,
            auto_exposure: 1.0,
            last_frame_at: None,
        })
    }

    pub fn adapter_name(&self) -> &str {
        &self.gpu.adapter_name
    }

    /// Samples per pixel in the shaded passes. `1` where the adapter offers no multisampling.
    pub fn samples(&self) -> u32 {
        self.targets.samples
    }

    pub fn backend(&self) -> &str {
        &self.gpu.backend
    }

    pub fn stats(&self) -> FrameStats {
        self.stats
    }

    /// Re-attach the swapchain without changing size, for a window the system stopped
    /// compositing while it was hidden.
    pub fn reconfigure(&mut self) {
        self.gpu.reconfigure();
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.gpu.resize(width, height);
        self.rebuild_targets();
    }

    /// Adopt the quality tier's render scale. Rebuilding every target is far too expensive to do
    /// per frame, so it happens only when the scale actually changes — a quality change or a
    /// resize, both of which an operator does deliberately.
    fn set_target_scale(&mut self, scale: f32) {
        let scale = scale.clamp(0.25, 2.0);
        if (scale - self.target_scale).abs() < 0.001 {
            return;
        }
        self.target_scale = scale;
        self.rebuild_targets();
    }

    /// The size the shaded passes draw at: the surface scaled by the active tier.
    fn scaled_size(&self) -> (u32, u32) {
        let scale = self.target_scale;
        let width = ((self.gpu.config.width as f32 * scale).round() as u32).max(1);
        let height = ((self.gpu.config.height as f32 * scale).round() as u32).max(1);
        (width, height)
    }

    fn rebuild_targets(&mut self) {
        let device = self.gpu.device.clone();
        let (width, height) = self.scaled_size();
        self.targets = Targets::new(&device, width, height, self.gpu.samples);
        let tiles = self.targets.tile_count();
        self.tile_counts
            .ensure(&device, (tiles * 4).max(256) as u64);
        self.tile_lights
            .ensure(&device, (tiles * MAX_LIGHTS_PER_TILE * 4).max(256) as u64);
        self.rebuild_scene_groups();
        self.depth_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("viz depth"),
            layout: &self.depth_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.targets.depth),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.haze_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.haze_sampler),
                },
            ],
        });
        self.bloom_extract_group = post_group(
            &device,
            &self.post_layout,
            &self.targets.shaded,
            &self.sampler,
            &self.post_settings,
        );
        self.bloom_blur_a_group = post_group(
            &device,
            &self.post_layout,
            &self.targets.bloom_a,
            &self.sampler,
            &self.post_settings,
        );
        self.bloom_blur_b_group = post_group(
            &device,
            &self.post_layout,
            &self.targets.bloom_b,
            &self.sampler,
            &self.post_settings,
        );
        self.composite_source_group = post_group(
            &device,
            &self.post_layout,
            &self.targets.shaded,
            &self.sampler,
            &self.post_settings,
        );
        self.composite_bloom_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("viz composite bloom"),
            layout: &self.composite_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.targets.bloom_a),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
    }

    /// Rebuild the two groups that point at the shadow matrix buffer, which moves whenever the
    /// number of shadow-casting lights grows past its allocation.
    /// Every mesh the current frame asked to draw.
    fn drawn_meshes(&self) -> Vec<MeshKind> {
        self.frame
            .meshes
            .iter()
            .filter(|(_, instances)| !instances.is_empty())
            .map(|(kind, _)| *kind)
            .collect()
    }

    /// Upload any fixture-model geometry this scene needs that is not on the GPU yet.
    fn ensure_model_meshes(&mut self, scene: &Scene) {
        let device = self.gpu.device.clone();
        for (model_index, model) in scene.models.iter().enumerate() {
            for (part_index, part) in model.parts.iter().enumerate() {
                let kind = MeshKind::ModelPart(model_index as u32, part_index as u32);
                if self.meshes.contains_key(&kind) {
                    continue;
                }
                let vertices: Vec<Vertex> = part
                    .positions
                    .iter()
                    .zip(part.normals.iter())
                    .map(|(position, normal)| Vertex {
                        position: *position,
                        normal: *normal,
                        uv: [0.0, 0.0],
                    })
                    .collect();
                if vertices.is_empty() || part.indices.is_empty() {
                    continue;
                }
                self.meshes.insert(
                    kind,
                    GpuMesh::new(
                        &device,
                        "fixture model",
                        &mesh::MeshData {
                            vertices,
                            indices: part.indices.clone(),
                        },
                    ),
                );
                self.mesh_instances.insert(
                    kind,
                    DynamicBuffer::new(
                        &device,
                        "viz model instances",
                        BufferUsages::VERTEX,
                        (size_of::<MeshInstance>() * 256) as u64,
                    ),
                );
            }
        }
    }

    /// Upload the scene's gobo artwork when it is not the artwork already on the GPU.
    ///
    /// A rig's glass changes when the show does — a repatch, another profile, a new revision —
    /// and not otherwise, so this is keyed on the scene revision rather than run per frame.
    fn ensure_gobo_atlas(&mut self, scene: &Scene) {
        if self.gobo_atlas.matches(&scene.gobo_artwork, scene.revision) {
            return;
        }
        self.gobo_atlas = crate::gobos::GoboAtlas::new(
            &self.gpu.device,
            &self.gpu.queue,
            &scene.gobo_artwork,
            scene.revision,
        );
        self.rebuild_shadow_groups();
    }

    fn rebuild_shadow_groups(&mut self) {
        let device = self.gpu.device.clone();
        self.shadow_bind_group = build_shadow_group(
            &device,
            &self.shadow_layout,
            &self.shadow_atlas,
            &self.shadow_sampler,
            &self.shadow_matrices,
            &self.gobo_atlas.view,
            &self.gobo_sampler,
        );
        self.shadow_draw_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("viz shadow draw"),
            layout: &self.shadow_draw_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.shadow_matrices.buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &self.shadow_draws,
                        offset: 0,
                        size: std::num::NonZeroU64::new(16),
                    }),
                },
            ],
        });
    }

    fn rebuild_scene_groups(&mut self) {
        let device = self.gpu.device.clone();
        self.scene_bind_group = build_scene_group(
            &device,
            &self.scene_layout,
            &self.globals_buffer,
            &self.lights,
            &self.tile_counts,
            &self.tile_lights,
        );
        self.cull_bind_group = build_scene_group(
            &device,
            &self.cull_layout,
            &self.globals_buffer,
            &self.lights,
            &self.tile_counts,
            &self.tile_lights,
        );
    }

    /// Adapt exposure to how much light the scene is actually producing.
    ///
    /// The estimate comes from the light list rather than a GPU read-back, so it costs nothing and
    /// adds no latency to the presented image. It behaves like eye adaptation: it moves towards
    /// the target over `ADAPTATION_SECONDS` instead of stepping.
    fn adapt_exposure(&mut self, delta_seconds: f32) {
        const ADAPTATION_SECONDS: f32 = 0.6;
        const REFERENCE: f32 = 2.6;
        let total: f32 = self
            .frame
            .lights
            .iter()
            .map(|light| light.colour_intensity[3].clamp(0.0, 1.0))
            .sum();
        let target = (REFERENCE / (1.0 + total.max(0.0).sqrt())).clamp(0.05, 1.6);
        let alpha = if ADAPTATION_SECONDS <= 0.0 {
            1.0
        } else {
            1.0 - (-delta_seconds / ADAPTATION_SECONDS).exp()
        };
        self.auto_exposure += (target - self.auto_exposure) * alpha.clamp(0.0, 1.0);
    }

    /// Effective exposure applied by the last frame, for the diagnostics surface.
    pub fn exposure(&self) -> f32 {
        self.auto_exposure
    }

    /// Render one frame into an offscreen image and read it back as RGBA8.
    ///
    /// This is how the fixed-camera golden images for every named view and quality tier are
    /// produced, and how a headless build machine proves the renderer works without a display
    /// server showing anything.
    pub fn capture(
        &mut self,
        scene: &Scene,
        values: &SceneValues,
        view: &ViewConfiguration,
        overlay: &crate::overlay::Overlay,
        time_seconds: f32,
    ) -> Result<CapturedImage, RenderError> {
        let device = self.gpu.device.clone();
        let queue = self.gpu.queue.clone();
        let width = self.gpu.config.width;
        let height = self.gpu.config.height;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("viz capture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.gpu.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        self.capture_request = Some(texture.create_view(&wgpu::TextureViewDescriptor::default()));
        self.render(scene, values, view, overlay, time_seconds)?;

        let row_bytes = width * 4;
        let padded = row_bytes.div_ceil(256) * 256;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("viz capture readback"),
            size: (padded * height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("viz capture copy"),
        });
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        queue.submit(Some(encoder.finish()));
        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });
        let mapped = slice
            .get_mapped_range()
            .map_err(|error| RenderError::Surface(format!("capture readback: {error}")))?;
        let mut pixels = Vec::with_capacity((row_bytes * height) as usize);
        for row in 0..height {
            let start = (row * padded) as usize;
            pixels.extend_from_slice(&mapped[start..start + row_bytes as usize]);
        }
        drop(mapped);
        buffer.unmap();
        let bgra = matches!(
            self.gpu.format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        );
        if bgra {
            for pixel in pixels.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
        }
        Ok(CapturedImage {
            width,
            height,
            rgba: pixels,
        })
    }

    /// Acquire the next presentable texture, recovering once from a lost or outdated surface.
    fn acquire(&mut self) -> Result<wgpu::SurfaceTexture, RenderError> {
        match self.gpu.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture)
            | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => Ok(texture),
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                Err(RenderError::SkipFrame)
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.gpu.reconfigure();
                match self.gpu.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(texture)
                    | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => Ok(texture),
                    other => Err(RenderError::Surface(format!("{other:?}"))),
                }
            }
            other => Err(RenderError::Surface(format!("{other:?}"))),
        }
    }
}

mod buffers;
mod frame;
mod groups;
mod layouts;
mod overlay_atlas;
mod pipelines;

use buffers::SceneBuffers;
use groups::Groups;
use layouts::{
    Layouts, PipelineLayouts, ShadowResources, build_scene_group, build_shadow_group, post_group,
};
use overlay_atlas::OverlayAtlas;
use pipelines::{Modules, Pipelines};

/// Edge of the haze volume, in voxels. A cube this size wraps every few metres of stage without
/// the eye finding the repeat, and costs a quarter of a megabyte.
const HAZE_VOLUME_EDGE: u32 = 32;

/// Build the tiling noise volume the beam pass reads its local haze density from.
///
/// Sampling one small 3D texture costs a fraction of evaluating noise arithmetically at every
/// march step, which is what makes non-uniform haze affordable at the frame rates a desk needs.
/// The values are per-voxel random and the hardware's trilinear filter does the smoothing, so the
/// volume tiles seamlessly with repeat addressing.
pub(super) fn build_haze_volume(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (wgpu::TextureView, wgpu::Sampler) {
    let edge = HAZE_VOLUME_EDGE as usize;
    let mut voxels = vec![0_u8; edge * edge * edge];
    // A fixed generator: the same room looks the same on every machine and in every capture.
    let mut state = 0x2545_f491_4f6c_dd1d_u64;
    for voxel in &mut voxels {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        *voxel = (state >> 33) as u8;
    }
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("viz haze"),
        size: wgpu::Extent3d {
            width: HAZE_VOLUME_EDGE,
            height: HAZE_VOLUME_EDGE,
            depth_or_array_layers: HAZE_VOLUME_EDGE,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D3,
        format: wgpu::TextureFormat::R8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        texture.as_image_copy(),
        &voxels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(HAZE_VOLUME_EDGE),
            rows_per_image: Some(HAZE_VOLUME_EDGE),
        },
        wgpu::Extent3d {
            width: HAZE_VOLUME_EDGE,
            height: HAZE_VOLUME_EDGE,
            depth_or_array_layers: HAZE_VOLUME_EDGE,
        },
    );
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("viz haze"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        address_mode_w: wgpu::AddressMode::Repeat,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    (
        texture.create_view(&wgpu::TextureViewDescriptor::default()),
        sampler,
    )
}

const _: () = assert!(TILE_SIZE == 16);

#[cfg(test)]
mod tests {
    use super::*;

    /// The multisampled beam pass reads a different WGSL texture type, and it is reached by
    /// substituting one declaration. A rename in the shader would leave the substitution a silent
    /// no-op and the pipeline would fail validation on a multisampling adapter only.
    #[test]
    fn the_beam_shader_carries_the_scene_depth_declaration_that_is_substituted() {
        assert!(BEAM_WGSL.contains(SCENE_DEPTH_BINDING));
        assert_eq!(scene_depth_binding(1), SCENE_DEPTH_BINDING);
        assert!(scene_depth_binding(4).contains("texture_depth_multisampled_2d"));
        let multisampled = BEAM_WGSL.replace(SCENE_DEPTH_BINDING, scene_depth_binding(4));
        assert!(multisampled.contains("texture_depth_multisampled_2d"));
        assert!(!multisampled.contains(SCENE_DEPTH_BINDING));
    }
}
