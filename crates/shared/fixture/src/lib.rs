#![forbid(unsafe_code)]
//! Fixture definitions, portable fixture library, color calibration, patching, and DMX encoding.

mod definition;
mod definition_model;
mod encoding;
mod error;
mod library;
mod package;
mod patch;
mod patch_model;
mod patch_validation;
mod portable_patch;
mod profile;

pub use definition::*;
pub use definition_model::*;
pub use encoding::*;
pub use error::*;
pub use library::*;
pub use package::*;
pub use patch::*;
pub use patch_model::*;
pub use patch_validation::*;
pub use portable_patch::*;
pub use profile::*;

#[cfg(test)]
mod tests;
