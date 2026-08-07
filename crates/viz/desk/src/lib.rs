#![forbid(unsafe_code)]
//! The lighting-desk scene provider.
//!
//! Scene and configuration come from the desk API; every live output value comes from real
//! Art-Net or sACN packets on the lighting network. Those planes never cross.

mod client;
mod preload_overlay;
mod provider;
mod routes;
mod scene_build;
mod transform;
/// The read projection of the scene-source API.
///
/// Public so another scene source can prove, by decoding into these exact types, that what it
/// serves is what the visualizer reads.
pub mod wire;

#[cfg(test)]
mod tests;

pub use provider::{DeskConnection, DeskProvider};
pub use routes::{default_mappings, mappings};
pub use scene_build::{DeskReadModels, build};
pub use transform::{Placement, PlacementSource, resolve, rotation_to_world, to_world};
