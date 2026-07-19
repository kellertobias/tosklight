use super::support::{PROFILE_A, create, profile_value};
use crate::StoreError;
use serde_json::json;
use std::fs;

#[test]
fn inline_snapshot_discovery_is_lossless_and_canonicalizes_duplicates() {
    let (path, show) = create("legacy-discovery");
    let profile = profile_value(PROFILE_A, 4, "Legacy");
    show.put_object(
        "fixture",
        "one",
        &json!({
            "definition":{"profile_snapshot":profile},
            "future_fixture":{"profile_snapshot":{"opaque":true}},
            "profile_snapshot":{"opaque":true},
            "fixtures":[{"definition":{"profile_snapshot":{"opaque":true}}}]
        }),
        0,
    )
    .unwrap();
    show.put_object(
        "fixture_bundle",
        "two",
        &json!({"fixtures":[{"definition":{"profile_snapshot":profile}}]}),
        0,
    )
    .unwrap();

    let document = show.portable_document().unwrap();
    let discovered = document.discover_legacy_inline_profile_snapshots().unwrap();
    assert_eq!(discovered.len(), 2);
    assert_eq!(discovered[0].json_pointer(), "/definition/profile_snapshot");
    assert_eq!(
        discovered[1].json_pointer(),
        "/fixtures/0/definition/profile_snapshot"
    );
    assert_eq!(
        discovered[0].profile().profile()["future_asset_manifest"],
        profile["future_asset_manifest"]
    );
    let canonical = document
        .canonical_legacy_fixture_profile_revisions()
        .unwrap();
    assert_eq!(canonical.len(), 1);
    assert_eq!(canonical[0].profile(), &profile);
    assert!(
        document.object("fixture", "one").unwrap().body()["definition"]["profile_snapshot"]
            .is_object()
    );
    drop(show);
    let _ = fs::remove_file(path);
}

#[test]
fn unrelated_snapshot_names_are_not_fixture_profiles() {
    let (path, show) = create("legacy-unrelated-snapshot");
    show.put_object(
        "future_object",
        "one",
        &json!({"profile_snapshot":{"opaque":true}}),
        0,
    )
    .unwrap();

    let document = show.portable_document().unwrap();
    assert!(
        document
            .discover_legacy_inline_profile_snapshots()
            .unwrap()
            .is_empty()
    );
    drop(show);
    let _ = fs::remove_file(path);
}

#[test]
fn current_patched_fixture_objects_are_discovered() {
    let (path, show) = create("legacy-patched-fixture-snapshot");
    let profile = profile_value(PROFILE_A, 1, "Current fixture profile");
    show.put_object(
        "patched_fixture",
        "one",
        &json!({
            "definition":{"profile_snapshot":profile},
            "fixtures":[{"definition":{"profile_snapshot":{"opaque":true}}}]
        }),
        0,
    )
    .unwrap();

    let discovered = show
        .portable_document()
        .unwrap()
        .discover_legacy_inline_profile_snapshots()
        .unwrap();
    assert_eq!(discovered.len(), 1);
    assert_eq!(discovered[0].owner().kind(), "patched_fixture");
    assert_eq!(discovered[0].json_pointer(), "/definition/profile_snapshot");
    drop(show);
    let _ = fs::remove_file(path);
}

#[test]
fn canonicalized_inline_snapshot_can_be_inserted_without_removing_legacy_data() {
    let (path, show) = create("legacy-insert");
    let profile = profile_value(PROFILE_A, 2, "Legacy insert");
    show.put_object(
        "fixture",
        "one",
        &json!({"definition":{"profile_snapshot":profile}}),
        0,
    )
    .unwrap();
    let document = show.portable_document().unwrap();
    let canonical = document
        .canonical_legacy_fixture_profile_revisions()
        .unwrap();
    let mut transaction = document.transaction();
    transaction
        .put_fixture_profile_revision(canonical[0].clone())
        .unwrap();

    let commit = show.apply_portable_transaction(transaction).unwrap();
    assert_eq!(commit.fixture_profile_revisions(), canonical);
    let persisted = show.objects("fixture").unwrap().remove(0);
    assert_eq!(persisted.body["definition"]["profile_snapshot"], profile);
    drop(show);
    let _ = fs::remove_file(path);
}

#[test]
fn canonicalization_rejects_conflicting_inline_identity() {
    let (path, show) = create("legacy-conflict");
    let first = profile_value(PROFILE_A, 1, "First");
    let second = profile_value(PROFILE_A, 1, "Second");
    show.put_object(
        "fixture",
        "one",
        &json!({"definition":{"profile_snapshot":first}}),
        0,
    )
    .unwrap();
    show.put_object(
        "fixture",
        "two",
        &json!({"definition":{"profile_snapshot":second}}),
        0,
    )
    .unwrap();

    let document = show.portable_document().unwrap();
    assert!(matches!(
        document.canonical_legacy_fixture_profile_revisions(),
        Err(StoreError::FixtureProfileRevisionConflict { revision: 1, .. })
    ));
    drop(show);
    let _ = fs::remove_file(path);
}
