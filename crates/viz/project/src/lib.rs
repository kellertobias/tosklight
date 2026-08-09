#![forbid(unsafe_code)]
//! DMX-to-fixture projection.
//!
//! One half compiles a patched show into the renderer's semantic scene and the channel bindings
//! that decode it; the other half applies received universe frames to those bindings. Fixture
//! semantics — splits, fine bytes, ranges, heads, colour systems, pixels, hazers — come from the
//! shared ToskLight fixture library rather than a second schema.

mod appearance;
mod binding;
mod colour;
mod decode;
mod default_model;
mod fallback;
mod plan;

pub use appearance::{
    MAX_COLOUR_TEMPERATURE_KELVIN, MIN_COLOUR_TEMPERATURE_KELVIN, apply_installed_appearance,
    colour_temperature_linear_rgb, installed_appearance_linear_rgb, parse_srgb_hex_linear,
};
pub use binding::ChannelRef;
pub use colour::{ResolvedColour, named_colour};
pub use decode::Decoder;
pub use default_model::{DefaultModel, FixtureTraits, choose as choose_default_model};
pub use fallback::{OpticalClass, classify};
pub use plan::{
    ColourBinding, EmitterBinding, ExternalCameraBinding, GOBO_ARTWORK_EDGE, PatchedFixture,
    PhysicalInstance, ScenePlan, compile, decode_gobo_artwork,
};
