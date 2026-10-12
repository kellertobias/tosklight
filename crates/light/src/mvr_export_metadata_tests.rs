//! Native preview memory must scale with unique source profiles, without collapsing conflicts.
use super::*;
use std::sync::Arc;

fn profile() -> FixtureProfile {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Acme".into();
    profile.name = "Native sharing".into();
    profile
}

fn fixture(profile: &FixtureProfile, mode: Uuid, number: u32) -> PatchedFixture {
    serde_json::from_value(serde_json::json!({
        "fixture_id": Uuid::new_v4(), "fixture_number": number,
        "name": format!("Native {number}"),
        "definition": profile.resolved_definition(mode).unwrap(),
        "universe": 1, "address": number,
    }))
    .unwrap()
}

fn document(fixtures: Vec<PatchedFixture>) -> light_mvr::MvrDocument {
    let metadata = serde_json::json!({"version":1,"fixtures":fixtures.into_iter().map(|fixture|
        serde_json::json!({"mvr_uuid":fixture.fixture_id,"fixture":fixture})
    ).collect::<Vec<_>>()});
    light_mvr::MvrDocument {
        files: HashMap::from([(
            TOSKLIGHT_MVR_FIXTURE_METADATA_PATH.into(),
            serde_json::to_vec(&metadata).unwrap(),
        )]),
        ..Default::default()
    }
}

#[test]
fn identical_native_profiles_share_one_allocation_across_modes() {
    let mut profile = profile();
    let mut second = profile.modes[0].clone();
    second.id = Uuid::new_v4();
    second.name = "Second mode".into();
    profile.modes.push(second);
    let fixtures = (1..=32)
        .map(|number| fixture(&profile, profile.modes[number as usize % 2].id, number))
        .collect::<Vec<_>>();
    let original = fixtures.clone();
    let decoded = tosklight_mvr_fixture_metadata(&document(fixtures));
    assert_eq!(decoded.len(), 32);
    let first = decoded
        .values()
        .next()
        .unwrap()
        .fixture
        .definition
        .profile_snapshot
        .as_ref()
        .unwrap();
    for source in original {
        let entry = &decoded[&source.fixture_id.0];
        assert_eq!(
            serde_json::to_value(&entry.fixture).unwrap(),
            serde_json::to_value(source).unwrap()
        );
        assert!(
            Arc::ptr_eq(
                first,
                entry.fixture.definition.profile_snapshot.as_ref().unwrap()
            ),
            "repeated source content must not allocate one retained full profile per fixture"
        );
    }
}

#[test]
fn native_content_conflicts_keep_same_revision_distinct() {
    let profile = profile();
    let mut changed = profile.clone();
    changed.notes = "Different source calibration evidence".into();
    let a = fixture(&profile, profile.modes[0].id, 1);
    let b = fixture(&changed, changed.modes[0].id, 2);
    let decoded = tosklight_mvr_fixture_metadata(&document(vec![a.clone(), b.clone()]));
    let pa = decoded[&a.fixture_id.0]
        .fixture
        .definition
        .profile_snapshot
        .as_ref()
        .unwrap();
    let pb = decoded[&b.fixture_id.0]
        .fixture
        .definition
        .profile_snapshot
        .as_ref()
        .unwrap();
    assert_eq!((pa.id, pa.revision), (pb.id, pb.revision));
    assert!(!Arc::ptr_eq(pa, pb));
    assert_eq!(pb.notes, changed.notes);
    let staged = crate::mvr_import::MvrDefinitions {
        definitions: decoded
            .into_iter()
            .map(|(id, entry)| (id, entry.fixture.definition))
            .collect(),
        ..Default::default()
    };
    let conflicts =
        crate::mvr_import::mvr_profile_conflicts(&staged, &Default::default(), &HashMap::new())
            .unwrap();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].fixtures.len(), 2);
}

#[test]
fn same_revision_keeps_different_source_bytes_and_verification_evidence_separate() {
    let mut original = profile();
    let archive = light_fixture::gdtf::profile::package_profile(&original).unwrap();
    original.source_gdtf =
        Some(light_fixture::ProfileGdtfSource::associate(&original, &archive).unwrap());
    let mut unverified = original.clone();
    unverified.source_gdtf.as_mut().unwrap().profile_fingerprint = None;
    let mut alternate_bytes = archive.clone();
    alternate_bytes.push(0); // Valid ZIP trailing data, retained verbatim rather than regenerated.
    let mut alternate = original.clone();
    alternate.source_gdtf =
        Some(light_fixture::ProfileGdtfSource::associate(&alternate, &alternate_bytes).unwrap());
    assert_eq!(
        alternate
            .source_gdtf
            .as_ref()
            .unwrap()
            .decoded_archive()
            .unwrap(),
        alternate_bytes
    );
    assert!(
        !unverified
            .source_gdtf
            .as_ref()
            .unwrap()
            .matches_profile(&unverified)
            .unwrap()
    );
    let fixtures = [&original, &unverified, &alternate]
        .into_iter()
        .enumerate()
        .map(|(index, profile)| fixture(profile, profile.modes[0].id, index as u32 + 1))
        .collect::<Vec<_>>();
    let decoded = tosklight_mvr_fixture_metadata(&document(fixtures.clone()));
    let mut snapshots = Vec::new();
    for source in fixtures {
        let entry = &decoded[&source.fixture_id.0];
        assert_eq!(
            serde_json::to_value(&entry.fixture).unwrap(),
            serde_json::to_value(source).unwrap()
        );
        snapshots.push(entry.fixture.definition.profile_snapshot.as_ref().unwrap());
    }
    for (index, profile) in snapshots.iter().enumerate() {
        assert!(
            snapshots[index + 1..]
                .iter()
                .all(|other| !Arc::ptr_eq(profile, other)),
            "source bytes or fingerprint must not collapse under identical profile ID/revision"
        );
    }
    assert_eq!(
        snapshots[1]
            .source_gdtf
            .as_ref()
            .unwrap()
            .profile_fingerprint,
        None
    );
    let staged = crate::mvr_import::MvrDefinitions {
        definitions: decoded
            .into_iter()
            .map(|(id, entry)| (id, entry.fixture.definition))
            .collect(),
        ..Default::default()
    };
    let conflicts =
        crate::mvr_import::mvr_profile_conflicts(&staged, &Default::default(), &HashMap::new())
            .unwrap();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].fixtures.len(), 3);
}
