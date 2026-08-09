#![forbid(unsafe_code)]
//! Renderer-owned semantic scene, live values, view configuration, and the source-provider
//! boundary shared by every Viz scene source.
//!
//! Nothing in this crate knows about HTTP routes, Art-Net, sACN, or planner IPC. A provider
//! projects its own transport into these types; the render core consumes only these types.
//!
//! # Coordinate convention
//!
//! Metres, right-handed, matching the existing ToskLight stage convention:
//!
//! - `+X` runs stage left to stage right,
//! - `+Y` is up, and
//! - `+Z` points towards the audience (downstage/front).

mod atmosphere;
mod diagnostics;
mod model;
mod persistence;
mod provider;
mod scene;
mod values;
mod view;

pub use atmosphere::{Atmosphere, AtmospherePreference, DEFAULT_DENSITY};
pub use diagnostics::{
    ConnectionState, FallbackReason, InputHealth, InputMappingStatus, ProviderDiagnostics,
    SourceProtocol, UniverseGrade, UniverseHealth,
};
pub use glam;
pub use model::{FixtureModel, ModelError, ModelPart, ModelPartKind, read_glb};
pub use persistence::{
    DEFAULT_DECAY_SECONDS, DEFAULT_FALLOFF, DEFAULT_THRESHOLD, PersistencePreference,
};
pub use provider::{
    ProviderCapabilities, ProviderError, ProviderEvent, ProviderKind, SceneProvider,
};
pub use scene::{
    Aabb, BodyKind, EmitterInstance, EmitterKind, EmitterLayoutCells, EmitterOptics, FixtureBody,
    FixtureInstance, GoboArtwork, GoboSlot, LaserOptics, LightSource, MotionAxis, Scene,
    SceneryKind, SceneryObject, SourceForm, euler_degrees,
};
pub use uuid;
pub use values::{
    CellValue, EmitterValues, LaserScan, PhysicalMotionState, PhysicalMotionTarget, ScanPoint,
    SceneValues, WheelMotionState,
};
pub use view::{Camera, RenderQuality, Theme, ViewConfiguration, ViewMode};

/// Wire/protocol version of the semantic scene contract understood by this render core.
pub const SCENE_PROTOCOL_VERSION: u32 = 1;
