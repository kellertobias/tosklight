//! Generating the demo show against the packages this repository actually ships.
//!
//! These tests build the real thing rather than a stand-in: a rig that names a profile the library
//! no longer carries, a mode that was renamed, or an addressing change that overlaps two fixtures
//! has to fail here rather than in a release nobody opened.

use super::*;
use std::collections::HashSet;

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
        for split in &fixture.patch.split_patches {
            let (Some(universe), Some(address)) = (split.universe, split.address) else {
                panic!("{} is not patched", fixture.patch.name);
            };
            assert!(
                occupied.insert((universe, address)),
                "{} starts on {universe}.{address}, which is already taken",
                fixture.patch.name
            );
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
    type PatchRow = (Option<u32>, String, Option<u16>, Option<u16>);

    let names = |generated: &GeneratedShow| {
        let document = PlanningDocument::open(&generated.path).expect("reopen");
        let snapshot = document.patch_snapshot().expect("patch");
        let mut rows: Vec<PatchRow> = snapshot
            .fixtures
            .iter()
            .map(|fixture| {
                (
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
