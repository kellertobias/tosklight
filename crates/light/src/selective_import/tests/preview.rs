use super::support::*;
use crate::selective_import::*;
use light_core::FixtureId;
use serde_json::json;
use uuid::Uuid;

#[test]
fn preview_expands_dependencies_skips_identical_and_reports_conflicts() {
    let rig = TestRig::new();
    rig.source_object(
        "macro",
        "root",
        json!({"id":"root","macro_id":"child","future":{"retained":true}}),
    );
    rig.source_object("macro", "child", json!({"id":"child","name":"Source"}));
    rig.target_object("macro", "child", json!({"id":"child","name":"Target"}));

    let blocked = rig.preview(rig.request("macro", "root"));
    assert!(!blocked.can_apply());
    assert_eq!(blocked.dependencies.len(), 1);
    assert_eq!(
        blocked.dependencies[0].disposition,
        ImportDependencyDisposition::Included
    );
    assert_eq!(blocked.conflicts.len(), 1);
    assert!(blocked.blockers.contains(&ImportBlocker::ObjectConflict {
        key: key("macro", "child")
    }));

    let request = rig.request("macro", "root").resolve(
        key("macro", "child"),
        ImportConflictResolution::KeepDestination,
    );
    let resolved = rig.preview(request);
    assert!(resolved.can_apply());
    assert!(resolved.objects.iter().any(|object| {
        object.source == key("macro", "child")
            && object.action == ImportObjectAction::KeepDestination
    }));
}

#[test]
fn semantic_identity_is_a_noop_unless_duplicate_is_explicit() {
    let rig = TestRig::new();
    let body = json!({"id":"same","nested":{"b":2,"a":1}});
    rig.source_object("macro", "same", body.clone());
    rig.target_object("macro", "same", body);

    let skipped = rig.preview(rig.request("macro", "same"));
    assert!(skipped.can_apply());
    assert_eq!(skipped.objects[0].action, ImportObjectAction::SkipIdentical);

    let duplicated = rig.preview(
        rig.request("macro", "same")
            .resolve(key("macro", "same"), ImportConflictResolution::Duplicate),
    );
    assert!(duplicated.can_apply());
    assert!(matches!(
        duplicated.objects[0].action,
        ImportObjectAction::Duplicate { .. }
    ));
}

#[test]
fn duplicate_keys_are_stable_and_references_are_planned_to_the_copy() {
    let rig = TestRig::new();
    rig.source_object("macro", "root", json!({"id":"root","macro_id":"child"}));
    rig.source_object(
        "macro",
        "child",
        json!({"id":"child","future_extension":{"opaque":[1,2,3]}}),
    );
    rig.target_object("macro", "child", json!({"id":"child","different":true}));
    let request = rig
        .request("macro", "root")
        .resolve(key("macro", "child"), ImportConflictResolution::Duplicate);

    let first = rig.preview(request.clone());
    let second = rig.preview(request);
    let destination = first
        .objects
        .iter()
        .find(|object| object.source == key("macro", "child"))
        .unwrap()
        .destination
        .clone();
    assert_ne!(destination, key("macro", "child"));
    assert_eq!(
        second
            .objects
            .iter()
            .find(|object| object.source == key("macro", "child"))
            .unwrap()
            .destination,
        destination
    );
}

#[test]
fn duplicate_key_skips_semantic_identities_from_unselected_custom_objects() {
    let rig = TestRig::new();
    rig.source_object(
        "custom_object",
        "1",
        json!({"identity":"1","payload":"source"}),
    );
    rig.target_object(
        "custom_object",
        "1",
        json!({"identity":"1","payload":"target"}),
    );
    rig.target_object("custom_object", "reserved", json!({"identity":"2"}));

    let preview = rig.preview(rig.request("custom_object", "1").resolve(
        key("custom_object", "1"),
        ImportConflictResolution::Duplicate,
    ));

    assert!(preview.can_apply(), "{:?}", preview.blockers);
    assert_eq!(preview.objects[0].destination.id(), "3");
}

#[test]
fn missing_target_identity_slot_is_a_reference_rewrite_blocker() {
    let rig = TestRig::new();
    rig.source_object(
        "custom_object",
        "root",
        json!({
            "identity":"root-identity",
            "reference": {
                "kind":"custom_object",
                "id":"child",
                "slot":"missing-slot",
                "identity":"child-identity"
            }
        }),
    );
    rig.source_object(
        "custom_object",
        "child",
        json!({"identity":"child-identity"}),
    );

    let preview = rig.preview(rig.request("custom_object", "root"));

    assert!(!preview.can_apply());
    assert!(preview.blockers.contains(&ImportBlocker::ReferenceRewrite {
        owner: key("custom_object", "root"),
        message: "no destination identity for custom_object/child slot missing-slot".into(),
    }));
}

#[test]
fn preview_includes_profile_snapshot_without_inferring_unknown_asset_fields() {
    let rig = TestRig::new();
    let profile_id = FixtureId::new();
    let snapshot = profile(
        profile_id,
        7,
        json!({
            "managed_asset_id":"audio-a",
            "future_asset_manifest":{"checksum":"kept"},
            "photograph_asset":"data:image/png;base64,AAE="
        }),
    );
    rig.source_profile(&snapshot);
    rig.source_object(
        "managed_asset",
        "audio-a",
        json!({"id":"audio-a","bytes":"AAEC"}),
    );
    rig.source_object(
        "effect",
        "one",
        json!({"id":"one","profile_id":profile_id.0,"profile_revision":7}),
    );

    let preview = rig.preview(rig.request("effect", "one"));
    assert!(preview.can_apply());
    assert_eq!(preview.profiles.len(), 1);
    assert_eq!(preview.profiles[0].action, ImportProfileAction::Copy);
    assert!(preview.managed_assets.is_empty());
    assert!(
        !preview
            .objects
            .iter()
            .any(|object| { object.source == key("managed_asset", "audio-a") })
    );
}

#[test]
fn preview_blocks_missing_dependencies_profiles_and_profile_conflicts() {
    let missing = TestRig::new();
    missing.source_object("macro", "root", json!({"id":"root","macro_id":"gone"}));
    let preview = missing.preview(missing.request("macro", "root"));
    assert!(preview.blockers.contains(&ImportBlocker::MissingObject {
        key: key("macro", "gone"),
        required_by: Some(key("macro", "root")),
    }));

    let profiles = TestRig::new();
    let id = FixtureId::new();
    profiles.source_object(
        "effect",
        "one",
        json!({"id":"one","profile_id":id.0,"profile_revision":2}),
    );
    let missing_profile = profiles.preview(profiles.request("effect", "one"));
    assert!(matches!(
        missing_profile.blockers.as_slice(),
        [ImportBlocker::MissingProfile { .. }]
    ));

    let conflict = TestRig::new();
    conflict.source_profile(&profile(id, 2, json!({"future":"source"})));
    conflict.target_profile(&profile(id, 2, json!({"future":"target"})));
    conflict.source_object(
        "effect",
        "one",
        json!({"id":"one","profile_id":id.0,"profile_revision":2}),
    );
    let blocked = conflict.preview(conflict.request("effect", "one"));
    assert!(blocked.blockers.contains(&ImportBlocker::ProfileConflict {
        key: ImportProfileKey {
            profile_id: id,
            revision: 2,
        },
    }));
}

#[test]
fn binding_to_an_existing_destination_is_visible_without_pulling_an_object() {
    let rig = TestRig::new();
    rig.source_object("macro", "root", json!({"id":"root","macro_id":"shared"}));
    rig.target_object("macro", "shared", json!({"id":"shared","destination":true}));

    let preview = rig.preview(rig.request("macro", "root"));
    assert!(preview.can_apply());
    assert_eq!(
        preview.dependencies[0].disposition,
        ImportDependencyDisposition::BoundToDestination
    );
    assert!(
        !preview
            .objects
            .iter()
            .any(|object| object.source == key("macro", "shared"))
    );
}

#[test]
fn unknown_profile_fields_are_not_inferred_as_managed_asset_references() {
    let rig = TestRig::new();
    let id = FixtureId::new();
    rig.source_profile(&profile(id, 1, json!({"managed_asset_id":"audio"})));
    rig.source_object(
        "managed_asset",
        "audio",
        json!({"id":"audio","body":"source"}),
    );
    rig.target_object(
        "managed_asset",
        "audio",
        json!({"id":"audio","body":"target"}),
    );
    rig.source_object(
        "effect",
        "one",
        json!({"id":"one","profile_id":id.0,"profile_revision":1}),
    );
    let preview = rig.preview(rig.request("effect", "one"));
    assert!(preview.can_apply());
    assert!(preview.managed_assets.is_empty());
    assert!(
        !preview
            .objects
            .iter()
            .any(|object| { object.source == key("managed_asset", "audio") })
    );
}

#[test]
fn identical_objects_still_expand_their_source_dependencies() {
    let rig = TestRig::new();
    let root = json!({"id":"root","macro_id":"child"});
    rig.source_object("macro", "root", root.clone());
    rig.target_object("macro", "root", root);
    rig.source_object("macro", "child", json!({"id":"child","source":true}));

    let preview = rig.preview(rig.request("macro", "root"));

    assert!(preview.can_apply());
    assert!(preview.objects.iter().any(|object| {
        object.source == key("macro", "root") && object.action == ImportObjectAction::SkipIdentical
    }));
    assert!(preview.objects.iter().any(|object| {
        object.source == key("macro", "child")
            && object.action == ImportObjectAction::ImportPreservingId
    }));
}

#[test]
fn logical_heads_and_multipatches_are_owned_identities_not_fixture_dependencies() {
    let rig = TestRig::new();
    let fixture = portable_fixture_record(10_000, 1);
    rig.source_profile(&fixture.profile);
    rig.source_object(
        "patched_fixture",
        &fixture.fixture_id.0.to_string(),
        fixture.body,
    );

    let preview = rig.preview(rig.request("patched_fixture", &fixture.fixture_id.0.to_string()));

    assert!(preview.can_apply(), "{:?}", preview.blockers);
    assert!(preview.dependencies.is_empty());
    assert_eq!(preview.profiles.len(), 1);
}

#[test]
fn fixture_number_conflicts_are_blocked_during_preview_candidate_validation() {
    let rig = TestRig::new();
    let source = portable_fixture_record(20_000, 7);
    let target = portable_fixture_record(30_000, 7);
    rig.source_profile(&source.profile);
    rig.target_profile(&target.profile);
    rig.source_object(
        "patched_fixture",
        &source.fixture_id.0.to_string(),
        source.body,
    );
    rig.target_object(
        "patched_fixture",
        &target.fixture_id.0.to_string(),
        target.body,
    );

    let preview = rig.preview(rig.request("patched_fixture", &source.fixture_id.0.to_string()));

    assert!(!preview.can_apply());
    assert!(
        preview
            .blockers
            .iter()
            .any(|blocker| { matches!(blocker, ImportBlocker::CandidateInvalid { .. }) })
    );
}

#[test]
fn managed_asset_preview_is_revision_exact() {
    let rig = TestRig::new();
    let asset_id = crate::AssetId(Uuid::from_u128(80_000));
    let revision_one = crate::AssetReference {
        id: asset_id,
        revision: crate::AssetRevision(1),
    };
    let revision_two = crate::AssetReference {
        id: asset_id,
        revision: crate::AssetRevision(2),
    };
    rig.source_object(
        "audio_cue",
        "one",
        json!({"id":"one","asset_id":asset_id.0,"asset_revision":1}),
    );
    rig.source_object(
        "audio_cue",
        "two",
        json!({"id":"two","asset_id":asset_id.0,"asset_revision":2}),
    );
    rig.asset_action(revision_one, ImportManagedAssetAction::SkipIdentical);
    rig.asset_action(revision_two, ImportManagedAssetAction::Missing);
    let request = SelectiveShowImportRequest::new(
        rig.source_id,
        rig.target_id,
        [key("audio_cue", "one"), key("audio_cue", "two")],
    );

    let preview = rig.preview(request);

    assert_eq!(preview.managed_assets.len(), 2);
    assert!(
        preview
            .blockers
            .contains(&ImportBlocker::MissingManagedAsset {
                asset: revision_two,
            })
    );
    assert!(
        !preview
            .blockers
            .contains(&ImportBlocker::MissingManagedAsset {
                asset: revision_one,
            })
    );
}

#[test]
fn conflicting_revisions_of_one_profile_family_share_one_duplicate_identity() {
    let rig = TestRig::new();
    let profile_id = FixtureId::new();
    for revision in [1, 2] {
        rig.source_profile(&profile(
            profile_id,
            revision,
            json!({"source_revision":revision}),
        ));
        rig.target_profile(&profile(
            profile_id,
            revision,
            json!({"target_revision":revision}),
        ));
        rig.source_object(
            "effect",
            &format!("effect-{revision}"),
            json!({
                "id":format!("effect-{revision}"),
                "profile_id":profile_id.0,
                "profile_revision":revision
            }),
        );
    }
    let first = ImportProfileKey {
        profile_id,
        revision: 1,
    };
    let second = ImportProfileKey {
        profile_id,
        revision: 2,
    };
    let request = SelectiveShowImportRequest::new(
        rig.source_id,
        rig.target_id,
        [key("effect", "effect-1"), key("effect", "effect-2")],
    )
    .resolve_profile(first, ImportProfileConflictResolution::Duplicate)
    .resolve_profile(second, ImportProfileConflictResolution::Duplicate);

    let preview = rig.preview(request);

    assert!(preview.can_apply(), "{:?}", preview.blockers);
    assert_eq!(preview.profiles.len(), 2);
    assert_eq!(
        preview.profiles[0].destination.profile_id,
        preview.profiles[1].destination.profile_id
    );
    assert_ne!(preview.profiles[0].destination.profile_id, profile_id);
    assert_eq!(preview.profiles[0].destination.revision, 1);
    assert_eq!(preview.profiles[1].destination.revision, 2);
}
