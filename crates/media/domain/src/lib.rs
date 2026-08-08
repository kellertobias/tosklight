#![forbid(unsafe_code)]

//! Media Server domain.
//!
//! This crate owns the authoritative Media state model and the pure behavior that describes it:
//! output identity, the layer and master state, and the canonical DMX personality that Art-Net,
//! sACN, the HTTP API, UI metadata, tests, and GDTF all read from.
//!
//! It deliberately holds no protocol, HTTP, filesystem, decoder, GPU, or operating-system types.
//! Anything that talks to the outside world lives in an adapter and translates into the typed
//! commands this crate defines.

pub mod output;
pub mod personality;
pub mod tempo;

pub use output::{OutputId, OutputName, PresentationMode};
pub use personality::{LayerPersonality, PersonalityVersion, SlotFootprint, StartAddressError};
pub use tempo::{SpeedGroupId, TempoSource};
