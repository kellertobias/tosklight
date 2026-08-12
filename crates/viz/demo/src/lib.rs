#![forbid(unsafe_code)]
//! Generating the canonical ToskLight demo show.
//!
//! The demo show is a product artefact, not a file somebody once saved: it is built here from the
//! rig declared in [`rig`] and the shipped fixture packages, through the same patch boundary the
//! Viz editor and the desk use. A release therefore ships a demo whose embedded profile revisions
//! are the ones that were current when it was built, and a fixture package that gains a model or a
//! mode reaches the demo by regenerating it.
//!
//! Nothing here reads the operator's data or writes anywhere but the destination it is given.

pub mod rig;

#[cfg(test)]
mod tests;

use light_application::{PatchFixtureCandidate, PatchFixturesCommand};
use light_core::{DmxAddress, FixtureId, Universe};
use light_fixture::{
    FixtureLibrary, FixtureLocation, FixtureProfile, FixtureVector, InstalledFixtureAppearance,
    PatchedFixturePatch, PatchedFixtureProfileReference, SplitPatch,
};
use rig::{DEMO_RIG, RigBlock};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use viz_document::PlanningDocument;

/// The name the generated show carries, and the name every copy of it is derived from.
pub const DEMO_SHOW_NAME: &str = "Demo Show";

/// The file name the packaged template is shipped under.
pub const DEMO_SHOW_FILE_NAME: &str = "demo-show.show";

/// The last slot of a DMX universe. A rig that will not fit is a mistake in the rig, not something
/// to silently wrap into the next universe.
const LAST_SLOT: u32 = 512;

#[derive(Debug)]
pub enum DemoError {
    /// The shipped library has no profile the rig names. Regenerating against a library that is
    /// missing a package must fail rather than quietly ship a smaller rig.
    MissingProfile {
        manufacturer: String,
        profile: String,
    },
    MissingMode {
        profile: String,
        mode: String,
    },
    UniverseFull {
        universe: u16,
        fixture: String,
    },
    Library(String),
    Document(String),
}

impl std::fmt::Display for DemoError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingProfile {
                manufacturer,
                profile,
            } => write!(
                formatter,
                "the fixture library has no {manufacturer} {profile}; the demo rig cannot be built \
                 from it"
            ),
            Self::MissingMode { profile, mode } => {
                write!(formatter, "{profile} has no mode called {mode}")
            }
            Self::UniverseFull { universe, fixture } => write!(
                formatter,
                "universe {universe} has no room left for {fixture}"
            ),
            Self::Library(detail) => write!(formatter, "fixture library: {detail}"),
            Self::Document(detail) => write!(formatter, "show file: {detail}"),
        }
    }
}

impl std::error::Error for DemoError {}

/// What was written, for the caller to report.
#[derive(Debug)]
pub struct GeneratedShow {
    pub path: PathBuf,
    pub name: String,
    pub fixtures: usize,
    /// The profile revisions the rig actually embedded, so a build log records what shipped.
    pub profile_revisions: BTreeMap<String, u32>,
}

/// Build the demo show at `destination` from the packages in `library`.
///
/// The destination is created; an existing file there is replaced, because this is a generated
/// artefact and regenerating it is the only way it is ever updated.
pub fn generate(library: FixtureLibrary, destination: &Path) -> Result<GeneratedShow, DemoError> {
    let profiles = library
        .profiles()
        .map_err(|error| DemoError::Library(error.to_string()))?;
    if destination.exists() {
        std::fs::remove_file(destination)
            .map_err(|error| DemoError::Document(error.to_string()))?;
    }
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|error| DemoError::Document(error.to_string()))?;
    }

    let document = PlanningDocument::create(destination, DEMO_SHOW_NAME)
        .map_err(|error| DemoError::Document(error.to_string()))?
        .with_library(library);

    let mut candidates = Vec::new();
    let mut revisions = BTreeMap::new();
    // Addresses are handed out per universe in patch order, exactly as an operator patching the
    // rig from the top of the sheet downwards would get them.
    let mut next_slot: BTreeMap<u16, u32> = BTreeMap::new();

    for block in DEMO_RIG {
        let profile = find_profile(&profiles, block)?;
        let mode = profile
            .modes
            .iter()
            .find(|mode| mode.name == block.mode)
            .ok_or_else(|| DemoError::MissingMode {
                profile: profile.name.clone(),
                mode: block.mode.to_owned(),
            })?;
        revisions.insert(
            format!("{} {}", profile.manufacturer, profile.name),
            profile.revision,
        );
        for index in 0..block.count {
            let name = block.fixture_name(index);
            let cursor = next_slot.entry(block.universe).or_insert(1);
            let mut split_patches = Vec::with_capacity(mode.splits.len());
            for split in &mode.splits {
                let end = *cursor + u32::from(split.footprint) - 1;
                if end > LAST_SLOT {
                    return Err(DemoError::UniverseFull {
                        universe: block.universe,
                        fixture: name.clone(),
                    });
                }
                split_patches.push(SplitPatch {
                    split: split.number,
                    universe: Some(block.universe as Universe),
                    address: Some(*cursor as DmxAddress),
                });
                *cursor = end + 1;
            }
            candidates.push(candidate(
                block,
                profile,
                mode.id,
                index,
                name,
                split_patches,
            ));
        }
    }

    let fixtures = candidates.len();
    document
        .patch_fixtures(PatchFixturesCommand {
            show_id: document.show_id(),
            fixtures: candidates,
            remove_fixture_ids: Vec::new(),
            placements: Vec::new(),
            vector_spreads: Vec::new(),
            fixture_updates: Vec::new(),
        })
        .map_err(|error| DemoError::Document(error.to_string()))?;

    Ok(GeneratedShow {
        path: destination.to_path_buf(),
        name: DEMO_SHOW_NAME.to_owned(),
        fixtures,
        profile_revisions: revisions,
    })
}

fn find_profile<'a>(
    profiles: &'a [FixtureProfile],
    block: &RigBlock,
) -> Result<&'a FixtureProfile, DemoError> {
    profiles
        .iter()
        .filter(|profile| {
            profile.manufacturer == block.manufacturer && profile.name == block.profile
        })
        // A library may hold more than one revision of a package; the demo ships the newest, which
        // is the one the desk would patch from too.
        .max_by_key(|profile| profile.revision)
        .ok_or_else(|| DemoError::MissingProfile {
            manufacturer: block.manufacturer.to_owned(),
            profile: block.profile.to_owned(),
        })
}

fn candidate(
    block: &RigBlock,
    profile: &FixtureProfile,
    mode_id: uuid::Uuid,
    index: u32,
    name: String,
    split_patches: Vec<SplitPatch>,
) -> PatchFixtureCandidate {
    let (x, y, z) = block.position(index);
    PatchFixtureCandidate {
        profile: PatchedFixtureProfileReference {
            profile_id: profile.id,
            profile_revision: u64::from(profile.revision),
            mode_id,
        },
        patch: PatchedFixturePatch {
            fixture_id: FixtureId::new(),
            fixture_number: Some(block.first_number + index),
            virtual_fixture_number: None,
            name,
            universe: split_patches.first().and_then(|split| split.universe),
            address: split_patches.first().and_then(|split| split.address),
            split_patches,
            layer_id: "default".to_owned(),
            direct_control: None,
            internal_bindings: Default::default(),
            location: FixtureLocation { x, y, z },
            rotation: FixtureVector {
                x: block.rotation.0,
                y: block.rotation.1,
                z: block.rotation.2,
            },
            logical_heads: Vec::new(),
            multipatch: Vec::new(),
            group_masters_enabled: true,
            grand_master_enabled: true,
            invert_pan: false,
            invert_tilt: false,
            bracket_angle: 0.0,
            shaper_angle: None,
            installed_appearance: InstalledFixtureAppearance::default(),
            move_in_black_enabled: true,
            move_in_black_delay_millis: 0,
            highlight_overrides: Default::default(),
        },
    }
}
