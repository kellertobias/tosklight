//! PosiStageNet on the desk: what the operator configured, what is arriving, and what it holds.
//!
//! The wire format lives in `light-psn-wire` and knows nothing about shows. This is the other
//! half: the configuration the show stores, the arithmetic that turns a marker's position into a
//! 3D Point's held value, and the zones that decide when a macro runs. Each piece is separated
//! from the socket so that the operator semantics — where a point ends up, when a zone counts as
//! occupied — are testable without a network.

pub(in crate::runtime) mod bindings;
pub(in crate::runtime) mod config;
pub(in crate::runtime) mod listener;
pub(in crate::runtime) mod service;
pub(in crate::runtime) mod zone_macros;
pub(in crate::runtime) mod zones;
