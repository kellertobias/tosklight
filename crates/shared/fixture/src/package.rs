//! Portable, self-contained ToskLight fixture packages.

mod archive;
mod assets;
mod codec;
mod glb;
mod manifest;

pub use codec::{read_fixture_package, read_package, write_fixture_package, write_package};
pub use manifest::*;

use archive::validate_zip_entry;
use glb::{invalid, validate_glb, validate_profile};

#[cfg(test)]
mod tests;
