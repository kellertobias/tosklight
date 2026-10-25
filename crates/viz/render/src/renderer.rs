//! The render core: one frame of the semantic scene into one presented image.

use crate::buffers::{DynamicBuffer, GpuMesh};
use crate::instances::{FrameInstances, MeshInstance, MeshKind};
use crate::mesh;
use crate::targets::{MAX_LIGHTS_PER_TILE, TILE_SIZE, Targets};
use bytemuck::{Pod, Zeroable};
use std::collections::HashMap;
use viz_scene::Scene;
#[cfg(test)]
use viz_scene::{SceneValues, ViewConfiguration};
use wgpu::BufferUsages;

const COMMON_WGSL: &str = include_str!("shaders/common.wgsl");
const SURFACE_WGSL: &str = include_str!("shaders/surface.wgsl");
const MEDIA_WGSL: &str = include_str!("shaders/media.wgsl");
const BEAM_WGSL: &str = include_str!("shaders/beam.wgsl");
const LASER_WGSL: &str = include_str!("shaders/laser.wgsl");
const LINES_WGSL: &str = include_str!("shaders/lines.wgsl");
const CULL_WGSL: &str = include_str!("shaders/cull.wgsl");
const POST_WGSL: &str = include_str!("shaders/post.wgsl");
const OVERLAY_WGSL: &str = include_str!("shaders/overlay.wgsl");
const SHADOW_WGSL: &str = include_str!("shaders/shadow.wgsl");

#[cfg(test)]
mod curtain_mask_tests;
#[cfg(test)]
mod curtain_optics_tests;
#[cfg(test)]
mod optics_tests;
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

/// Ultra targets one 60 Hz display interval instead of consuming every available GPU cycle.
pub const ULTRA_GPU_BUDGET_MICROS: u64 = 16_000;

const ULTRA_LADDER: [(u32, u32, f32); 6] = [
    (64, 10, 1.0),
    (48, 8, 1.0),
    (40, 6, 0.9),
    (32, 4, 0.8),
    (24, 2, 0.7),
    (16, 0, 0.6),
];

#[derive(Debug, Default)]
struct UltraBudget {
    level: usize,
    over_budget: u8,
    under_budget: u16,
}

impl UltraBudget {
    fn observe(&mut self, gpu_micros: u64) {
        if gpu_micros > ULTRA_GPU_BUDGET_MICROS {
            self.under_budget = 0;
            self.over_budget = self.over_budget.saturating_add(1);
            if self.over_budget >= 2 && self.level + 1 < ULTRA_LADDER.len() {
                self.level += 1;
                self.over_budget = 0;
            }
        } else if gpu_micros < 12_000 {
            self.over_budget = 0;
            self.under_budget = self.under_budget.saturating_add(1);
            if self.under_budget >= 120 && self.level > 0 {
                self.level -= 1;
                self.under_budget = 0;
            }
        } else {
            self.over_budget = 0;
            self.under_budget = 0;
        }
    }

    fn settings(&self) -> (u32, u32, f32) {
        ULTRA_LADDER[self.level]
    }

    fn degraded(&self) -> bool {
        self.level > 0
    }
}

/// The exposure the picture is drawn at, before the operator's own trim multiplies it.
///
/// Fixed, and deliberately so. This used to adapt to how much light the rig was producing, the way
/// an eye does, and the result was a Stage that fought the dimmer: taking a rig down opened the
/// exposure by almost as much as the fixtures had closed, so the first tenth of a fade was the
/// whole visible change and the rest of it did nothing. A lighting desk's Stage has to answer
/// "how bright is this" with the same picture every time, which means one exposure, not a
/// wandering one.
const BASE_EXPOSURE: f32 = 0.85;

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
    params4: [f32; 4],
    params5: [f32; 4],
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
    /// Elapsed render wall time, including acquire; not executing-thread CPU time.
    pub cpu_micros: u64,
    /// How long this frame waited for the display to release a drawable. The wait happens on the
    /// caller's thread, so an application that also handles input needs to know about it.
    pub acquire_micros: u64,
    pub lights: u32,
    pub beams: u32,
    pub instances: u32,
    pub particles_requested: u32,
    pub particles_drawn: u32,
    pub draw_calls: u32,
    /// Set when the renderer had to reduce quality to stay inside the budget.
    pub degraded: bool,
    /// The concrete limits responsible for [`Self::degraded`]. Kept as counts so the operator
    /// surface can explain what must change instead of reporting an unexplained warning.
    pub quality_reduction: QualityReduction,
    /// What the GPU itself spent on a recent frame, where the adapter can time one. This is the
    /// number that says whether there is headroom under a display-limited frame rate.
    pub gpu_micros: Option<u64>,
    /// Identity of the most recent completed asynchronous timestamp sample.
    pub gpu_sample_id: Option<u64>,
    /// Whether that sample belongs to this scene/view/quality/size generation.
    pub gpu_sample_matches_frame: bool,
    /// Named pass costs from the same asynchronously sampled frame as `gpu_micros`.
    pub gpu_passes: crate::timing::GpuPassTimings,
    /// Effective Ultra controls after the frame-budget controller is applied.
    pub volumetric_steps: u32,
    pub shadow_budget: u32,
    pub render_scale: f32,
    /// Audience capacity and stable subset actually drawn this frame.
    pub crowd_authored: u32,
    pub crowd_drawn: u32,
}

/// Every bounded renderer resource that can make a frame cheaper than the requested quality.
#[derive(Clone, Copy, Debug, Default)]
pub struct QualityReduction {
    pub lights: Option<(u32, u32)>,
    pub beams: Option<(u32, u32)>,
    pub crowd: Option<(u32, u32)>,
    pub particles: Option<(u32, u32)>,
    pub ultra_gpu_budget: bool,
}

impl QualityReduction {
    pub const fn is_active(self) -> bool {
        self.lights.is_some()
            || self.beams.is_some()
            || self.crowd.is_some()
            || self.particles.is_some()
            || self.ultra_gpu_budget
    }
}

mod context;
mod draft;
mod facade;
mod lines;
mod overlay;
mod runtime_hash;
mod shaded;
mod shaded_assets;
mod tiled;
use context::FrameContext;
pub use facade::Renderer;
use overlay::OverlayRenderer;
use shaded::ShadedRenderer;

mod buffers;
mod capture;
pub use capture::PendingCapture;
pub(crate) mod curtain_occlusion;
mod frame;
mod groups;
mod layouts;
mod overlay_atlas;
mod pipelines;

use buffers::SceneBuffers;
use groups::Groups;
use layouts::{
    Layouts, PipelineLayouts, ShadowResources, build_media_group, build_scene_group,
    build_shadow_group, post_group,
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
mod tests;
