use super::*;
use light_application::MvrImportResolution;
use light_fixture::{ChannelFunction, ChannelFunctionBehavior, FixtureChannel, MultiPatchInstance};
use light_mvr::{MvrDocument, MvrFixture};
use std::collections::HashMap;

fn pan_profile() -> FixtureProfile {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "MVR Precision".into();
    profile.name = "Source fixture".into();
    profile.short_name = "Source".into();
    let mode = &mut profile.modes[0];
    mode.name = "Precise".into();
    let mut channel: FixtureChannel = serde_json::from_value(serde_json::json!({
        "id": Uuid::new_v4(), "head_id": mode.heads[0].id, "split":1,
        "attribute":"pan", "fixture_attribute":"pan", "resolution":"u16",
        "secondary_slots":[3], "default_raw":32769, "highlight_raw":65534,
    }))
    .unwrap();
    channel.functions = vec![ChannelFunction::continuous(
        "Pan",
        channel.attribute.clone(),
        65535,
    )];
    channel.functions[0].behavior = ChannelFunctionBehavior::Continuous {
        physical_min: 540.,
        physical_max: -540.,
        unit: Some("degrees".into()),
    };
    mode.channels = vec![channel];
    mode.splits[0].footprint = 3;
    profile
}

fn source(uuid: u128, spec: &str, address: u16) -> MvrFixture {
    MvrFixture {
        uuid: Uuid::from_u128(uuid),
        name: format!("Source {uuid}"),
        fixture_id: None,
        gdtf_spec: spec.into(),
        gdtf_mode: "Precise".into(),
        universe: Some(1),
        address: Some(address),
        matrix: [1., 0., 0., 0., 1., 0., 0., 0., 1., 0., 0., 0.],
        layer: None,
        class: None,
    }
}

fn external(profile: &FixtureProfile) -> MvrDocument {
    MvrDocument {
        fixtures: vec![source(1, "odd-name.GDTF", 1)],
        files: HashMap::from([(
            "GDTF/Odd-Name.gdtf".into(),
            light_fixture::gdtf::profile::package_profile(profile).unwrap(),
        )]),
        ..Default::default()
    }
}

#[test]
fn mvr_embedded_profiles_keep_precision_source_and_distinct_revision_contents() {
    let rig = rig("mvr-exact-sources");
    let first = pan_profile();
    let mut second = first.clone();
    second.modes[0].channels[0].functions[0].behavior = ChannelFunctionBehavior::Continuous {
        physical_min: 270.,
        physical_max: -270.,
        unit: Some("degrees".into()),
    };
    let mut archive = external(&first);
    archive.fixtures.push(source(2, "second.gdtf", 20));
    archive.files.insert(
        "second.gdtf".into(),
        light_fixture::gdtf::profile::package_profile(&second).unwrap(),
    );
    let expected_sources = archive.files.values().cloned().collect::<Vec<_>>();
    let prepared = rig.document.prepare_mvr_import(archive).unwrap();
    assert!(prepared.preview.fixtures.iter().all(|f| f.matched));
    assert!(rig.document.patch_snapshot().unwrap().fixtures.is_empty());
    let report = rig
        .document
        .import_prepared_mvr(prepared, HashMap::new())
        .unwrap();
    assert_eq!(
        (report.imported_fixtures, report.unresolved_fixtures),
        (2, 0)
    );
    let store = ShowStore::open(&rig.path).unwrap();
    let portable = store.portable_document().unwrap();
    let mut ranges = Vec::new();
    for revision in portable
        .fixture_profile_revisions()
        .iter()
        .filter(|r| r.id().profile_id() == first.id)
    {
        let profile: FixtureProfile = serde_json::from_value(revision.profile().clone()).unwrap();
        assert!(
            expected_sources.contains(
                &profile
                    .source_gdtf
                    .as_ref()
                    .unwrap()
                    .decoded_archive()
                    .unwrap()
            )
        );
        let channel = &profile.modes[0].channels[0];
        assert_eq!(channel.resolution, light_fixture::ChannelResolution::U16);
        assert_eq!(channel.secondary_slots, vec![3]);
        assert_eq!(channel.default_raw, 32769);
        match channel.functions[0].behavior {
            ChannelFunctionBehavior::Continuous {
                physical_min,
                physical_max,
                ..
            } => ranges.push((physical_min as i32, physical_max as i32)),
            _ => panic!("continuous Pan survives"),
        }
    }
    ranges.sort();
    assert_eq!(ranges, vec![(270, -270), (540, -540)]);
    drop(store);
    let reopened = PlanningDocument::open(&rig.path).unwrap();
    assert_eq!(reopened.patch_snapshot().unwrap().fixtures.len(), 2);
}

#[test]
fn mvr_invalid_present_archive_cannot_fall_back_to_a_library_namesake() {
    let path = temp_path("mvr-invalid-present");
    let library_path = path.with_extension("library.sqlite");
    let library = light_fixture::FixtureLibrary::open(&library_path).unwrap();
    let imported = light_fixture::gdtf::read::import_profile(
        &light_fixture::gdtf::profile::package_profile(&pan_profile()).unwrap(),
    )
    .unwrap();
    let profile = library.save_profile(imported.profile, 0).unwrap();
    let document = PlanningDocument::create(&path, "Namesake")
        .unwrap()
        .with_library(library);
    let spec = format!("{}.gdtf", profile.name);
    let mut archive = MvrDocument {
        fixtures: vec![source(1, &spec, 1)],
        ..Default::default()
    };
    // Only an absent archive permits installed fallback, with the full retained source snapshot.
    let fallback = document.prepare_mvr_import(archive.clone()).unwrap();
    assert!(fallback.preview.fixtures[0].matched);
    archive.files.insert(spec, vec![0, 1, 2]);
    let invalid = document.prepare_mvr_import(archive).unwrap();
    assert!(!invalid.preview.fixtures[0].matched);
    assert!(!invalid.preview.warnings.is_empty());
    let result = document
        .import_prepared_mvr(invalid, HashMap::new())
        .unwrap();
    assert_eq!(result.unresolved_fixtures, 1);
    assert!(document.patch_snapshot().unwrap().fixtures.is_empty());
    // Refresh because the unresolved-object write changed the destination.
    let mut archive = external(&profile);
    archive.files.clear();
    archive.fixtures[0].gdtf_spec = format!("{}.gdtf", profile.name);
    document.import_mvr(archive, HashMap::new()).unwrap();
    let portable = ShowStore::open(&path).unwrap().portable_document().unwrap();
    let stored = portable
        .fixture_profile_revision(profile.id, u64::from(profile.revision))
        .unwrap();
    let stored: FixtureProfile = serde_json::from_value(stored.profile().clone()).unwrap();
    assert_eq!(stored.source_gdtf, profile.source_gdtf);
    drop(document);
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(library_path);
}

#[test]
fn mvr_preview_reimport_excludes_own_patch_and_detects_other_lean_copies() {
    let rig = rig("mvr-own-occupancy");
    let mut command = patch_one(rig.document.show_id(), rig.profile);
    command.fixtures[0]
        .patch
        .multipatch
        .push(MultiPatchInstance {
            id: Uuid::new_v4(),
            universe: Some(1),
            address: Some(30),
            split_patches: vec![SplitPatch {
                split: 1,
                universe: Some(1),
                address: Some(30),
            }],
            ..Default::default()
        });
    rig.document.patch_fixtures(command).unwrap();
    let archive = PlanningDocument::read_mvr(&rig.document.export_mvr().unwrap().data).unwrap();
    let prepared = rig.document.prepare_mvr_import(archive).unwrap();
    assert!(
        prepared
            .preview
            .fixtures
            .iter()
            .all(|f| f.matched && !f.conflicted)
    );
    rig.document
        .import_prepared_mvr(prepared, HashMap::new())
        .unwrap();
    let snapshot = rig.document.patch_snapshot().unwrap();
    assert_eq!(snapshot.fixtures.len(), 1);
    assert_eq!(snapshot.fixtures[0].patch.address, Some(1));
    assert_eq!(snapshot.fixtures[0].patch.multipatch[0].address, Some(30));
    let mut other = external(&pan_profile());
    other.fixtures[0].address = Some(30);
    let preview = rig.document.preview_mvr(&other).unwrap();
    assert!(
        preview.fixtures[0].conflicted,
        "a lean physical copy occupies its address"
    );
}

#[test]
fn mvr_prepared_import_rejects_changed_destination_and_invalid_address_without_writes() {
    let rig = rig("mvr-retry");
    let prepared = rig
        .document
        .prepare_mvr_import(external(&pan_profile()))
        .unwrap();
    let revision = rig.document.patch_revision().unwrap();
    assert!(
        rig.document
            .import_prepared_mvr(
                prepared.clone(),
                HashMap::from([(
                    Uuid::from_u128(1),
                    MvrImportResolution::Address {
                        universe: 1,
                        address: 512
                    }
                ),])
            )
            .is_err()
    );
    assert_eq!(rig.document.patch_revision().unwrap(), revision);
    assert!(rig.document.patch_snapshot().unwrap().fixtures.is_empty());
    assert_eq!(
        rig.document
            .import_prepared_mvr(prepared.clone(), HashMap::new())
            .unwrap()
            .imported_fixtures,
        1
    );
    let error = rig
        .document
        .import_prepared_mvr(prepared, HashMap::new())
        .err()
        .unwrap();
    assert!(error.to_string().contains("destination changed"), "{error}");
    assert_eq!(rig.document.patch_snapshot().unwrap().fixtures.len(), 1);
}

#[test]
fn mvr_native_identity_collision_is_rejected_atomically() {
    let source = rig("mvr-native-collision-source");
    source
        .document
        .patch_fixtures(patch_one(source.document.show_id(), source.profile))
        .unwrap();
    let archive = PlanningDocument::read_mvr(&source.document.export_mvr().unwrap().data).unwrap();
    let target = rig("mvr-native-collision-target");
    let source_store = ShowStore::open(&source.path).unwrap();
    let mut body = source_store
        .portable_document()
        .unwrap()
        .fixture_profile_revision(source.profile.profile_id, source.profile.profile_revision)
        .unwrap()
        .profile()
        .clone();
    body["notes"] = serde_json::json!("Same immutable key, different meaning");
    target.document.retain_fixture_profile(body).unwrap();
    let revision = target.document.patch_revision().unwrap();
    let error = target
        .document
        .import_mvr(archive, HashMap::new())
        .err()
        .unwrap();
    assert!(error.to_string().contains("immutable profile"), "{error}");
    assert_eq!(target.document.patch_revision().unwrap(), revision);
    assert!(
        target
            .document
            .patch_snapshot()
            .unwrap()
            .fixtures
            .is_empty()
    );
}
