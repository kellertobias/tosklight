#![forbid(unsafe_code)]
//! Compatibility staging for the one canonical ToskLight demo show.
//!
//! The complete demo is authored through the Desk API generator and committed as
//! `assets/demo.show`. Desk, PreViz, captures, and release packaging must copy those exact bytes;
//! this crate remains only for callers of the former `viz-demo-show` command.

use light_fixture::FixtureLibrary;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use viz_document::PlanningDocument;

pub const DEMO_SHOW_NAME: &str = "Demo Show";
pub const DEMO_SHOW_FILE_NAME: &str = "demo-show.show";

#[derive(Debug)]
pub enum DemoError {
    Document(String),
}

impl std::fmt::Display for DemoError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Document(detail) => write!(formatter, "show file: {detail}"),
        }
    }
}

impl std::error::Error for DemoError {}

#[derive(Debug)]
pub struct GeneratedShow {
    pub path: PathBuf,
    pub name: String,
    pub fixtures: usize,
    pub profile_revisions: BTreeMap<String, u32>,
}

fn canonical_demo_show() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../assets/demo.show")
}

/// Copy the canonical demo asset to `destination` and prove that the copied show reopens.
///
/// `library` is accepted for source compatibility with the retired rig generator. The canonical
/// show already embeds the exact portable profile revisions it uses.
pub fn generate(_library: FixtureLibrary, destination: &Path) -> Result<GeneratedShow, DemoError> {
    let source = canonical_demo_show();
    let bytes = std::fs::read(&source).map_err(|error| {
        DemoError::Document(format!("reading canonical {}: {error}", source.display()))
    })?;
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|error| DemoError::Document(error.to_string()))?;
    }
    std::fs::write(destination, bytes).map_err(|error| DemoError::Document(error.to_string()))?;
    let document = PlanningDocument::open(destination)
        .map_err(|error| DemoError::Document(error.to_string()))?;
    let snapshot = document
        .patch_snapshot()
        .map_err(|error| DemoError::Document(error.to_string()))?;
    Ok(GeneratedShow {
        path: destination.to_owned(),
        name: DEMO_SHOW_NAME.to_owned(),
        fixtures: snapshot.fixtures.len(),
        profile_revisions: BTreeMap::new(),
    })
}

#[cfg(test)]
mod tests;
