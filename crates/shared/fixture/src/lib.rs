#![forbid(unsafe_code)]
//! Fixture definitions, portable fixture library, color calibration, patching, and DMX encoding.

pub mod body_catalogue;
mod color_calibration;
mod definition;
mod definition_model;
mod encoding;
mod error;
pub mod gdtf;
mod highlight_look;
mod library;
pub mod media_color;
mod package;
mod patch;
mod patch_model;
mod patch_validation;
mod portable_patch;
mod position_calibration;
mod position_freeze;
mod profile;
mod scenery_options;

pub use color_calibration::*;
pub use definition::*;
pub use definition_model::*;
pub use encoding::*;
pub use error::*;
pub use highlight_look::*;
pub use library::*;
pub use package::*;
pub use patch::*;
pub use patch_model::*;
pub use patch_validation::*;
pub use portable_patch::*;
pub use position_calibration::*;
pub use position_freeze::*;
pub use profile::*;
pub use scenery_options::*;

#[cfg(test)]
mod tests;
