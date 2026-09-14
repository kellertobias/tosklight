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

/// Whether `bytes` are a complete, self-contained GLB 2.0 model, as a package model must be.
///
/// A model brought in outside a package — a venue model imported straight into a show — is held to
/// the same rule, so a show never carries a model a package could not.
pub fn validate_glb_model(bytes: &[u8]) -> Result<(), FixturePackageError> {
    validate_glb(bytes)
}

#[cfg(test)]
mod tests;
