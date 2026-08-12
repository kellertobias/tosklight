//! Generating the demo show against the packages this repository actually ships.
//!
//! These tests build the real thing rather than a stand-in: a rig that names a profile the library
//! no longer carries, a mode that was renamed, or an addressing change that overlaps two fixtures
//! has to fail here rather than in a release nobody opened.

use super::*;
use light_core::{AttributeKey, AttributeValue};
use light_programmer::Preset;
use std::collections::HashSet;
use viz_scene::EmitterValues;

/// The shipped packages, which are what a release generates the demo from.
fn shipped_packages() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../assets/fixture-library")
}

/// A directory of this test's own, under the canonical temporary root rather than the system one.
fn workspace(name: &str) -> PathBuf {
    let root = std::env::var_os("LIGHT_TMP_DIR").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../.artifacts/tmp"),
        PathBuf::from,
    );
    let directory = root.join("viz-demo-tests").join(name);
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).expect("test workspace");
    directory
}

fn library_in(directory: &Path) -> FixtureLibrary {
    let library = FixtureLibrary::open(directory.join("fixtures.sqlite")).expect("library");
    library
        .load_fixture_package_directory(shipped_packages())
        .expect("the shipped fixture packages load");
    library
}

fn generated_in(name: &str) -> (PathBuf, GeneratedShow) {
    let directory = workspace(name);
    let library = library_in(&directory);
    let destination = directory.join(DEMO_SHOW_FILE_NAME);
    let generated = generate(library, &destination).expect("the demo show generates");
    (directory, generated)
}

#[test]
fn every_profile_and_mode_the_rig_names_is_in_the_shipped_library() {
    let (_directory, generated) = generated_in("profiles");
    let expected: usize = DEMO_RIG.iter().map(|block| block.count as usize).sum();
    assert_eq!(generated.fixtures, expected);
    assert_eq!(generated.name, DEMO_SHOW_NAME);
}

#[test]
fn the_generated_show_reopens_as_an_ordinary_document() {
    let (_directory, generated) = generated_in("reopen");
    let document = PlanningDocument::open(&generated.path).expect("the show reopens");
    assert_eq!(document.name().expect("name"), DEMO_SHOW_NAME);
    let snapshot = document.patch_snapshot().expect("patch");
    assert_eq!(snapshot.fixtures.len(), generated.fixtures);
}

/// A show that ships has to open on a machine that has never seen this fixture library, so the
/// profile revisions the rig uses have to have travelled into the file.
#[test]
fn the_generated_show_embeds_the_profile_revisions_its_rig_uses() {
    let (_directory, generated) = generated_in("embedded");
    let document = PlanningDocument::open(&generated.path).expect("the show reopens");
    let snapshot = document.patch_snapshot().expect("patch");
    assert!(
        !snapshot.profile_revisions.is_empty(),
        "the demo show embedded no profile revisions"
    );
    for fixture in &snapshot.fixtures {
        assert!(
            snapshot
                .profile_revisions
                .iter()
                .any(|revision| revision.profile_id == fixture.profile.profile_id
                    && revision.profile_revision == fixture.profile.profile_revision),
            "{} references a profile revision the show does not carry",
            fixture.patch.name
        );
    }
}

/// Two fixtures sharing a slot is the one addressing mistake a generated rig can make silently.
#[test]
fn no_two_fixtures_in_the_demo_rig_share_a_dmx_slot() {
    let (_directory, generated) = generated_in("addresses");
    let document = PlanningDocument::open(&generated.path).expect("the show reopens");
    let snapshot = document.patch_snapshot().expect("patch");
    let mut occupied: HashSet<(u16, u16)> = HashSet::new();
    for fixture in &snapshot.fixtures {
        let profile = snapshot
            .profile_revisions
            .iter()
            .find(|profile| {
                profile.profile_id == fixture.profile.profile_id
                    && profile.profile_revision == fixture.profile.profile_revision
            })
            .expect("fixture profile revision");
        let mode = profile
            .referenced_modes
            .iter()
            .find(|mode| mode.mode_id == fixture.profile.mode_id)
            .expect("fixture mode");
        for split in &fixture.patch.split_patches {
            let (Some(universe), Some(address)) = (split.universe, split.address) else {
                panic!("{} is not patched", fixture.patch.name);
            };
            let footprint = mode
                .splits
                .iter()
                .find(|mode_split| mode_split.number == split.split)
                .expect("patch split exists in selected mode")
                .footprint;
            for slot in address..address + footprint {
                assert!(
                    occupied.insert((universe, slot)),
                    "{} uses {universe}.{slot}, which is already taken",
                    fixture.patch.name
                );
            }
        }
    }
}

/// Generating twice must produce the same rig: the demo is a build product, and a build product
/// that differs run to run cannot be the thing a release promises it packaged.
#[test]
fn generating_twice_produces_the_same_rig() {
    let (_first_directory, first) = generated_in("repeat-one");
    let (_second_directory, second) = generated_in("repeat-two");
    assert_eq!(first.fixtures, second.fixtures);
    assert_eq!(first.profile_revisions, second.profile_revisions);

    /// One fixture as the operator sees it addressed: number, name, universe and address.
    type PatchRow = (String, Option<u32>, String, Option<u16>, Option<u16>);

    let names = |generated: &GeneratedShow| {
        let document = PlanningDocument::open(&generated.path).expect("reopen");
        let snapshot = document.patch_snapshot().expect("patch");
        let mut rows: Vec<PatchRow> = snapshot
            .fixtures
            .iter()
            .map(|fixture| {
                (
                    fixture.patch.fixture_id.0.to_string(),
                    fixture.patch.fixture_number,
                    fixture.patch.name.clone(),
                    fixture.patch.universe,
                    fixture.patch.address,
                )
            })
            .collect();
        rows.sort();
        rows
    };
    assert_eq!(names(&first), names(&second));
}

#[test]
fn the_demo_profiles_project_canonical_gobo_and_prism_controls() {
    let directory = workspace("gobo-prism-capabilities");
    let library = library_in(&directory);
    let profiles = library.profiles().expect("profiles");
    let profile = profiles
        .iter()
        .find(|profile| profile.manufacturer == "ROBE" && profile.name == "Robin DLS Profile")
        .expect("shipped ROBE profile");
    let mode = profile
        .modes
        .iter()
        .find(|mode| mode.name == "Mode 3")
        .expect("documented Mode 3");
    let attributes = mode
        .channels
        .iter()
        .map(|channel| channel.attribute.0.as_str())
        .collect::<HashSet<_>>();
    for required in ["gobo.1", "gobo.1.rotation", "prism.1", "prism.1.rotation"] {
        assert!(
            attributes.contains(required),
            "Mode 3 does not project canonical {required}"
        );
    }
    let serialized = serde_json::to_value(profile).expect("serialized profile");
    assert!(
        serialized["gobos"]
            .as_array()
            .is_some_and(|gobos| gobos.iter().any(|gobo| !gobo["artwork_asset"].is_null())),
        "the demo gobo fixture carries no artwork"
    );
}

#[test]
fn the_mixed_demo_preset_produces_observable_gobo_and_prism_state() {
    let (_directory, generated) = generated_in("gobo-prism-look");
    let document = PlanningDocument::open(&generated.path).expect("the show reopens");
    let stored = document
        .objects("preset")
        .expect("presets")
        .into_iter()
        .find(|preset| preset.id == GOBO_PRISM_DEMO_PRESET)
        .expect("gobo/prism demo preset");
    let preset: Preset = serde_json::from_value(stored.body).expect("typed mixed preset");
    assert_eq!(preset.name, "Gobo + Prism Demo");

    let gobo = preset
        .values
        .get(&demo_fixture_id(GOBO_DEMO_FIXTURE_NUMBER))
        .expect("gobo fixture values");
    let prism = preset
        .values
        .get(&demo_fixture_id(PRISM_DEMO_FIXTURE_NUMBER))
        .expect("prism fixture values");
    let normalized = |values: &std::collections::HashMap<AttributeKey, AttributeValue>,
                      key: &str| match values.get(&AttributeKey(key.into())) {
        Some(AttributeValue::Normalized(value)) => *value,
        other => panic!("{key} is not a normalized demo value: {other:?}"),
    };

    let gobo_state = EmitterValues {
        intensity: normalized(gobo, "intensity"),
        shutter: normalized(gobo, "shutter"),
        gobo: normalized(gobo, "gobo.1"),
        ..EmitterValues::default()
    };
    assert!(gobo_state.visible_intensity() > 0.99);
    assert!(gobo_state.gobo_slot(8) > 0);

    let prism_state = EmitterValues {
        intensity: normalized(prism, "intensity"),
        shutter: normalized(prism, "shutter"),
        gobo: normalized(prism, "gobo.1"),
        prism: normalized(prism, "prism.1"),
        ..EmitterValues::default()
    };
    assert!(prism_state.visible_intensity() > 0.99);
    assert!(prism_state.gobo_slot(8) > 0);
    assert!(prism_state.prism_facets() >= 3);
}

/// Regenerating over an existing file replaces it rather than merging into it.
#[test]
fn regenerating_replaces_an_existing_file() {
    let directory = workspace("replace");
    let destination = directory.join(DEMO_SHOW_FILE_NAME);
    std::fs::write(&destination, b"not a show file").expect("stale file");
    let library = library_in(&directory);
    let generated = generate(library, &destination).expect("regenerates over the stale file");
    assert_eq!(
        generated.fixtures,
        DEMO_RIG.iter().map(|b| b.count as usize).sum::<usize>()
    );
}
