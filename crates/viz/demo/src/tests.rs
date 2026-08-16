use super::*;

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
    FixtureLibrary::open(directory.join("fixtures.sqlite")).expect("library")
}

#[test]
fn compatibility_generator_copies_the_canonical_asset_byte_for_byte() {
    let directory = workspace("canonical-copy");
    let destination = directory.join(DEMO_SHOW_FILE_NAME);
    let generated = generate(library_in(&directory), &destination).expect("canonical demo copies");
    assert_eq!(generated.fixtures, 293);
    assert_eq!(generated.name, DEMO_SHOW_NAME);
    assert_eq!(
        std::fs::read(destination).expect("copied demo"),
        std::fs::read(canonical_demo_show()).expect("canonical demo")
    );
}

#[test]
fn compatibility_generator_replaces_a_stale_destination() {
    let directory = workspace("replace");
    let destination = directory.join(DEMO_SHOW_FILE_NAME);
    std::fs::write(&destination, b"not a show").expect("stale destination");
    generate(library_in(&directory), &destination).expect("canonical demo replaces stale file");
    PlanningDocument::open(&destination).expect("copied show reopens");
}
