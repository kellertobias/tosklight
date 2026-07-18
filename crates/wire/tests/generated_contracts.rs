use std::{fs, path::Path};

#[test]
fn checked_in_contract_artifacts_match_the_rust_dtos() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for artifact in light_wire::generation::generated_artifacts() {
        let path = workspace_root.join(&artifact.path);
        let actual = fs::read_to_string(&path).unwrap_or_else(|error| {
            panic!(
                "generated wire artifact {} is missing: {error}; run cargo run -p light-wire --example generate-contracts",
                artifact.path
            )
        });
        assert_eq!(
            actual, artifact.contents,
            "generated wire artifact {} is stale; run cargo run -p light-wire --example generate-contracts",
            artifact.path
        );
    }
}
