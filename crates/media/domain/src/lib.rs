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
pub mod timeline;

pub use address::{AddressClass, MediaAddress};
pub use clock::{MeasuredCadence, RenderClock};
pub use color::{FlipMirror, Tint};
pub use command::{Command, CommandKind, CommandSource, ControlOwnership, Timestamp};
pub use geometry::{LayerTransform, Point, Size};
pub use layer::{LayerState, MaskState, ScalingMode, SourceFailure, SourceStatus};
pub use master::MasterState;
pub use output::{OutputId, OutputName, PresentationMode};
pub use personality::{LayerPersonality, PersonalityVersion, SlotFootprint, StartAddressError};
pub use playback::{OnceEndState, PlayMode};
pub use speed::SpeedMultiplier;
pub use state::{Applied, MediaState, OutputState, apply};
pub use tempo::{SpeedGroupId, TempoSource};
pub use timeline::{MediaTiming, Presentation};
