#![forbid(unsafe_code)]

//! Media Server domain.
//!
//! This crate owns the authoritative Media state model and the pure behavior that describes it:
//! output identity, the layer and master state, and the canonical DMX personality that Art-Net,
//! sACN, the HTTP API, UI metadata, tests, and GDTF all read from.
//!
//! It deliberately holds no protocol, HTTP, filesystem, decoder, GPU, or operating-system types.
//! Anything that talks to the outside world lives in an adapter and translates into the typed
//! commands this crate defines. It reads no clock either: adapters stamp commands, and the
//! reducer only compares timestamps.

pub mod address;
pub mod audio;
pub mod authored_tempo;
pub mod catalog;
pub mod clock;
pub mod color;
pub mod command;
pub mod dmx;
pub mod geometry;
pub mod layer;
pub mod master;
pub mod output;
pub mod personality;
pub mod playback;
pub mod speed;
pub mod state;
pub mod tempo;
pub mod text;
pub mod text_catalog;
pub mod timeline;
pub mod visualizer;

pub use address::{AddressClass, AssetId, MediaAddress};
pub use audio::{Analysis, BeatDetector, Tuning};
pub use catalog::{
    CatalogError, CatalogFolder, CatalogItem, CatalogLocation, CatalogSnapshot, ItemKind,
};
pub use clock::{MeasuredCadence, RenderClock};
pub use color::{FlipMirror, Tint};
pub use command::{
    Command, CommandKind, CommandSource, ControlOwnership, LayerControls, MasterControls, Timestamp,
};
pub use geometry::{LayerTransform, Point, Size};
pub use layer::{
    ANALOG_TV_EFFECT, AnalogTvParameters, BEAT_MOVE_EFFECT, BEAT_SCALE_TURN_EFFECT,
    BEAT_SCAN_EFFECT, BLUR_EFFECT, BeatMoveDirection, BeatMoveParameters, BeatScaleTurnParameters,
    BeatScanEdge, BeatScanParameters, BlurParameters, DIGITAL_TV_EFFECT, DigitalTvParameters,
    EffectSlot, FEEDBACK_EFFECT, FeedbackMotion, FeedbackParameters, KALEIDOSCOPE_EFFECT,
    KaleidoscopeParameters, LayerState, MaskSource, MaskState, OPACITY_CYCLE_EFFECT,
    OpacityCycleInterval, RASTERIZE_EFFECT, RasterizeMode, RasterizeParameters, ScalingMode,
    SourceFailure, SourceStatus,
};
pub use master::MasterState;
pub use output::{OutputId, OutputName, PresentationMode};
pub use personality::{LayerPersonality, SlotFootprint, StartAddressError};
pub use playback::{OnceEndState, PlayMode};
pub use speed::SpeedMultiplier;
pub use state::{Applied, MediaState, OutputState, apply};
pub use tempo::{
    ResolvedTempo, SpeedGroupId, SpeedGroupSnapshot, TempoSource, effective_rate, resolve_tempo,
};
pub use text::{Countdown, TextEntry, TextKind, Visibility};
pub use text_catalog::{Alignment, TextCatalog, TextCatalogError, TextSlot, TextStyle};
pub use timeline::{MediaTiming, Presentation};
pub use visualizer::{
    ALL_KINDS, GeneratedCatalog, GeneratedCatalogError, GeneratedEntry, Parameter,
    VisualizerConfiguration, VisualizerKind, VisualizerParameters,
};
