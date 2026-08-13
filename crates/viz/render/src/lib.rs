#![forbid(unsafe_code)]
//! The Viz render core.
//!
//! The core owns GPU resources and nothing else. It takes a presentation-surface adapter, a
//! semantic scene, its live values, and a view configuration, and presents one image. It never
//! opens a socket, parses a packet, or talks to a scene source.

mod buffers;
mod camera;
mod export;
pub mod font;
mod gobos;
mod gpu;
mod instances;
pub mod media;
mod mesh;
pub mod overlay;
mod pick;
mod renderer;
mod targets;
mod timing;

pub use camera::{CameraControl, Ray, ResolvedCamera};
pub use export::{GeometryInstance, GeometryMesh, SceneGeometry, scene_geometry};
pub use gpu::PresentationSurface;
pub use instances::{
    EmitterPose, FrameInstances, FrameStyle, GOBO_SLOTS, SemanticLight, build as build_instances,
    emitter_pose, semantic_lights,
};
pub use overlay::{Overlay, OverlayQuad};
pub use pick::{Pick, PickedElement, pick};
pub use renderer::{CapturedImage, FrameStats, RenderError, Renderer};
pub use targets::{MAX_LIGHTS_PER_TILE, TILE_SIZE};
pub use timing::GpuPassTimings;
pub use wgpu;
