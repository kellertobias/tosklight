use super::*;
use std::convert::Infallible;

/// Retains a source GDTF for exactly the profiles it was given.
struct Retained(HashMap<Uuid, Vec<u8>>);

impl GdtfSource for Retained {
    type Error = Infallible;

    fn source_gdtf(&self, profile: FixtureId, _: u32) -> Result<Option<Vec<u8>>, Infallible> {
        Ok(self.0.get(&profile.0).cloned())
    }
}

fn patched(profile: &FixtureProfile, number: u32) -> (String, PatchedFixture) {
    let definition = profile
        .resolved_definition(profile.modes[0].id)
        .expect("a blank profile resolves");
    let fixture: PatchedFixture = serde_json::from_value(serde_json::json!({
        "fixture_id": Uuid::new_v4(),
        "fixture_number": number,
        "name": format!("Wash {number}"),
        "definition": definition,
        "universe": 1,
        "address": number,
    }))
    .expect("a patched fixture");
    (Uuid::new_v4().to_string(), fixture)
}

fn profile(name: &str, revision: u32) -> FixtureProfile {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Acme".into();
    profile.name = name.into();
    profile.revision = revision;
    profile.modes[0].name = "Mode — A".into();
    profile
}

#[test]
fn a_profile_without_a_retained_source_is_embedded_as_a_generated_gdtf() {
    let profile = profile("Wash", 1);
    let fixtures = [patched(&profile, 1), patched(&profile, 2)];
    let (document, summary) = build_mvr_document(
        &fixtures,
        &HashMap::new(),
        Vec::new(),
        &Retained(HashMap::new()),
        |_| None,
    )
    .unwrap();

    assert_eq!(summary.generated_profiles, 2);
    assert!(summary.missing_profiles.is_empty());
    let spec = &document.fixtures[0].gdtf_spec;
    assert_eq!(
        document.fixtures[1].gdtf_spec, *spec,
        "one file per profile revision"
    );
    let modes = light_mvr::read_gdtf(&document.files[spec]).expect("a readable GDTF");
    assert_eq!(modes[0].name, "Mode - A");
    assert_eq!(
        document.fixtures[0].gdtf_mode, "Mode - A",
        "the fixture names the mode as the generated GDTF spells it"
    );
    assert_eq!(
        summary.warnings.len(),
        1,
        "the operator is told it was generated"
    );
}

fn physical_profile() -> FixtureProfile {
    let mut profile = profile("Spot", 2);
    let mode = &mut profile.modes[0];
    mode.splits[0].footprint = 1;
    let mut function = light_fixture::ChannelFunction::continuous(
        "Zoom",
        light_core::AttributeKey("zoom".into()),
        255,
    );
    function.behavior = light_fixture::ChannelFunctionBehavior::Continuous {
        physical_min: 5.0,
        physical_max: 50.0,
        unit: Some("deg".into()),
    };
    mode.channels.push(
        serde_json::from_value(serde_json::json!({
            "id": Uuid::new_v4(),
            "head_id": mode.heads[0].id,
            "split": 1,
            "fixture_attribute": "zoom",
            "attribute": "zoom",
            "resolution": "u8",
            "default_raw": 0,
            "highlight_raw": 255,
            "functions": [function],
        }))
        .unwrap(),
    );
    profile
}

#[test]
fn current_profile_data_wins_over_an_unverified_retained_source() {
    let mut current = physical_profile();
    let original = gdtf::profile::package_profile(&current).unwrap();
    current.modes[0].channels[0].functions[0].behavior =
        light_fixture::ChannelFunctionBehavior::Continuous {
            physical_min: 60.0,
            physical_max: 3.0,
            unit: Some("deg".into()),
        };
    let expected = gdtf::profile::package_profile(&current).unwrap();
    let fixtures = [patched(&current, 1)];
    let source = Retained(HashMap::from([(current.id.0, original.clone())]));
    let (document, summary) =
        build_mvr_document(&fixtures, &HashMap::new(), Vec::new(), &source, |_| None).unwrap();

    assert_eq!(summary.embedded_profiles, 0);
    assert_eq!(summary.generated_profiles, 1);
    let spec = &document.fixtures[0].gdtf_spec;
    assert_eq!(spec, "Acme@Spot.gdtf");
    assert_ne!(document.files[spec], original);
    assert_eq!(
        document.files[spec], expected,
        "the exported archive carries the current directed physical range"
    );
    assert_eq!(document.fixtures[0].gdtf_mode, "Mode - A");
    assert!(summary.warnings.iter().any(|warning| {
        warning.contains("retained source GDTF") && warning.contains("not verified")
    }));
    assert_eq!(
        source.0[&current.id.0], original,
        "source ownership stays with the library"
    );
}

#[test]
fn refused_physical_curves_report_the_reason_without_using_stale_source_bytes() {
    let mut profile = physical_profile();
    let original = gdtf::profile::package_profile(&profile).unwrap();
    let calibration = light_fixture::PhysicalMappingCalibration {
        samples: vec![
            light_fixture::PhysicalMappingPoint {
                raw: 0,
                physical: 5.0,
            },
            light_fixture::PhysicalMappingPoint {
                raw: 128,
                physical: 18.0,
            },
            light_fixture::PhysicalMappingPoint {
                raw: 255,
                physical: 50.0,
            },
        ],
        ..Default::default()
    };
    profile.modes[0].channels[0].functions[0].physical_mapping = Some(calibration.clone());
    let source = Retained(HashMap::from([(profile.id.0, original)]));
    let fixtures = [patched(&profile, 1), patched(&profile, 2)];
    let (document, summary) =
        build_mvr_document(&fixtures, &HashMap::new(), Vec::new(), &source, |_| None).unwrap();

    assert_eq!(summary.generated_profiles, 0);
    assert_eq!(summary.embedded_profiles, 0);
    assert_eq!(summary.missing_profiles.len(), 2);
    assert_eq!(
        document.fixtures.len(),
        2,
        "unsupported GDTF does not delete show fixtures"
    );
    for fixture in &document.fixtures {
        assert!(
            !document.files.contains_key(&fixture.gdtf_spec),
            "never fall back to stale source data"
        );
    }
    let errors = summary
        .warnings
        .iter()
        .filter(|warning| warning.contains("piecewise physical curve"))
        .collect::<Vec<_>>();
    assert_eq!(errors.len(), 1, "one diagnostic per profile revision");
    assert!(errors[0].contains("Acme · Spot (revision 2)"));
    assert!(errors[0].contains(".toskfixture"));
    assert!(
        summary
            .warnings
            .iter()
            .any(|warning| warning.contains("not verified"))
    );
    let native = tosklight_mvr_fixture_metadata(&document);
    assert_eq!(native.len(), 2);
    for fixture in native.values() {
        let snapshot = fixture
            .fixture
            .definition
            .profile_snapshot
            .as_ref()
            .unwrap();
        assert_eq!(
            snapshot.modes[0].channels[0].functions[0]
                .physical_mapping
                .as_ref(),
            Some(&calibration)
        );
    }
}

#[test]
fn two_revisions_of_one_fixture_get_distinct_archive_names() {
    let first = profile("Wash", 1);
    let mut second = first.clone();
    second.revision = 2;
    let fixtures = [patched(&first, 1), patched(&second, 2)];
    let (document, _) = build_mvr_document(
        &fixtures,
        &HashMap::new(),
        Vec::new(),
        &Retained(HashMap::new()),
        |_| None,
    )
    .unwrap();
    assert_eq!(document.fixtures[0].gdtf_spec, "Acme@Wash.gdtf");
    assert_eq!(document.fixtures[1].gdtf_spec, "Acme@Wash r2.gdtf");
}

#[test]
fn archive_names_stay_at_the_root_and_never_differ_only_by_case() {
    let mut used = HashSet::new();
    assert_eq!(
        archive_name("gdtf/Acme@Spot.GDTF", 1, &mut used),
        "Acme@Spot.gdtf"
    );
    assert_eq!(
        archive_name("acme@spot.gdtf", 4, &mut used),
        "acme@spot r4.gdtf"
    );
    assert_eq!(
        archive_name("acme@spot", 4, &mut used),
        "acme@spot r4-2.gdtf"
    );
    assert_eq!(archive_name("A:B?.gdtf", 1, &mut used), "A_B_.gdtf");
    assert_eq!(archive_name(" .gdtf", 1, &mut used), "Fixture.gdtf");
}

#[test]
fn patch_layers_become_named_mvr_layers_in_patch_order() {
    let layers = mvr_layers([
        (
            "floor".to_owned(),
            serde_json::json!({"id": "floor", "name": "Floor", "order": 2}),
        ),
        (
            "truss".to_owned(),
            serde_json::json!({"name": " Truss 1 ", "order": 1}),
        ),
        (
            "spare".to_owned(),
            serde_json::json!({"id": "spare", "order": 3}),
        ),
    ]);
    let layer = |id: &str, name: &str| light_mvr::MvrLayer {
        id: id.into(),
        name: name.into(),
    };
    assert_eq!(
        layers,
        vec![
            layer("truss", "Truss 1"),
            layer("floor", "Floor"),
            layer("spare", "")
        ]
    );

    let profile = profile("Wash", 1);
    let mut on_floor = patched(&profile, 1);
    on_floor.1.layer_id = "floor".into();
    let (document, _) = build_mvr_document(
        &[on_floor],
        &HashMap::new(),
        layers.clone(),
        &Retained(HashMap::new()),
        |_| None,
    )
    .unwrap();
    assert_eq!(document.layers, layers);
    assert_eq!(document.fixtures[0].layer.as_deref(), Some("floor"));
}

struct LibrarySource(light_fixture::FixtureLibrary);

impl GdtfSource for LibrarySource {
    type Error = light_fixture::FixtureError;

    fn source_gdtf(
        &self,
        profile: FixtureId,
        revision: u32,
    ) -> Result<Option<Vec<u8>>, Self::Error> {
        self.0.source_gdtf(profile, revision)
    }

    fn source_gdtf_with_evidence(
        &self,
        profile: FixtureId,
        revision: u32,
    ) -> Result<Option<FixtureGdtfSource>, Self::Error> {
        self.0.source_gdtf_with_evidence(profile, revision)
    }
}

fn rich_source(profile: &FixtureProfile) -> Vec<u8> {
    // Source-only descriptive metadata is deliberately absent from the normalized profile.
    // Byte-for-byte retention must keep it instead of replacing the archive with a new subset.
    let mut source = gdtf::profile::fixture_type(profile).unwrap();
    source
        .description
        .push_str(" Original manufacturer optical notes");
    for (mode, original) in source.modes.iter_mut().zip(&profile.modes) {
        mode.name = original.name.clone();
    }
    gdtf::package(&source).unwrap()
}

#[test]
fn verified_source_survives_revision_only_saves_with_its_original_archive_and_mode_names() {
    let source = LibrarySource(light_fixture::FixtureLibrary::open(":memory:").unwrap());
    let mut draft = physical_profile();
    draft.modes[0].name = "Source Mode".into();
    let first = source.0.save_profile(draft, 0).unwrap();
    let original = rich_source(&first);
    source
        .0
        .set_profile_source_gdtf(first.id, first.revision, &original)
        .unwrap();
    let second = source.0.save_profile(first, 1).unwrap();
    let fixtures = [patched(&second, 1)];
    let (document, summary) =
        build_mvr_document(&fixtures, &HashMap::new(), Vec::new(), &source, |_| None).unwrap();
    assert_eq!(summary.embedded_profiles, 1);
    assert_eq!(summary.generated_profiles, 0);
    assert!(summary.warnings.is_empty());
    assert!(summary.missing_profiles.is_empty());
    assert_eq!(document.files[&document.fixtures[0].gdtf_spec], original);
    assert_eq!(document.fixtures[0].gdtf_mode, "Source Mode");
}

#[test]
fn export_verification_and_type_cache_use_each_actual_snapshot_not_library_identifiers() {
    let source = LibrarySource(light_fixture::FixtureLibrary::open(":memory:").unwrap());
    let original_profile = source.0.save_profile(physical_profile(), 0).unwrap();
    let original = rich_source(&original_profile);
    source
        .0
        .set_profile_source_gdtf(original_profile.id, original_profile.revision, &original)
        .unwrap();
    let mut edited_snapshot = original_profile.clone();
    edited_snapshot.modes[0].channels[0].functions[0].behavior =
        light_fixture::ChannelFunctionBehavior::Continuous {
            physical_min: 60.0,
            physical_max: 3.0,
            unit: Some("deg".into()),
        };
    let expected = gdtf::profile::package_profile(&edited_snapshot).unwrap();
    // Deliberately identical identifiers: only the embedded content differs.
    let fixtures = [patched(&original_profile, 1), patched(&edited_snapshot, 2)];
    let (document, summary) =
        build_mvr_document(&fixtures, &HashMap::new(), Vec::new(), &source, |_| None).unwrap();
    assert_eq!(summary.embedded_profiles, 1);
    assert_eq!(summary.generated_profiles, 1);
    let first = &document.fixtures[0].gdtf_spec;
    let second = &document.fixtures[1].gdtf_spec;
    assert_ne!(first, second);
    assert_eq!(document.files[first], original);
    assert_eq!(document.files[second], expected);
    assert!(summary.warnings.iter().any(|warning| {
        warning.contains("associated profile does not match the actual export snapshot")
    }));
    assert!(
        summary
            .warnings
            .iter()
            .any(|warning| warning.contains("without optical wheels"))
    );
}

#[test]
fn a_selected_mode_subset_cannot_reuse_full_profile_source_evidence() {
    let source = LibrarySource(light_fixture::FixtureLibrary::open(":memory:").unwrap());
    let mut draft = physical_profile();
    let mut second = FixtureProfile::blank().modes.remove(0);
    second.name = "Other Mode".into();
    draft.modes.push(second);
    let full = source.0.save_profile(draft, 0).unwrap();
    let original = rich_source(&full);
    source
        .0
        .set_profile_source_gdtf(full.id, full.revision, &original)
        .unwrap();
    let mut subset = full.clone();
    subset.modes.truncate(1);
    let fixtures = [patched(&subset, 1)];
    let (document, summary) =
        build_mvr_document(&fixtures, &HashMap::new(), Vec::new(), &source, |_| None).unwrap();
    assert_eq!(summary.embedded_profiles, 0);
    assert_eq!(summary.generated_profiles, 1);
    let data = &document.files[&document.fixtures[0].gdtf_spec];
    assert_ne!(data, &original);
    assert_eq!(light_mvr::read_gdtf(data).unwrap().len(), 1);
}

#[test]
fn verified_source_is_retained_even_when_native_calibration_cannot_be_generated() {
    let source = LibrarySource(light_fixture::FixtureLibrary::open(":memory:").unwrap());
    let mut draft = physical_profile();
    let original = rich_source(&draft);
    draft.modes[0].channels[0].functions[0].physical_mapping =
        Some(light_fixture::PhysicalMappingCalibration {
            samples: vec![
                light_fixture::PhysicalMappingPoint {
                    raw: 0,
                    physical: 5.0,
                },
                light_fixture::PhysicalMappingPoint {
                    raw: 128,
                    physical: 18.0,
                },
                light_fixture::PhysicalMappingPoint {
                    raw: 255,
                    physical: 50.0,
                },
            ],
            source: Some("Import calibration".into()),
            revision: 3,
            ..Default::default()
        });
    let profile = source.0.save_profile(draft, 0).unwrap();
    assert!(gdtf::profile::package_profile(&profile).is_err());
    // Attachment explicitly associates the normalized import and its complete original archive.
    // The fingerprint checks unchanged content, not independent measurement correctness.
    source
        .0
        .set_profile_source_gdtf(profile.id, profile.revision, &original)
        .unwrap();
    let fixtures = [patched(&profile, 1)];
    let (document, summary) =
        build_mvr_document(&fixtures, &HashMap::new(), Vec::new(), &source, |_| None).unwrap();
    assert_eq!(summary.embedded_profiles, 1);
    assert!(summary.missing_profiles.is_empty());
    assert!(summary.warnings.is_empty());
    assert_eq!(document.files[&document.fixtures[0].gdtf_spec], original);

    // A calibration-only provenance revision is meaningful; excluding the library revision must
    // not accidentally exclude nested revision fields and recertify this edited snapshot.
    let mut changed = profile;
    changed.modes[0].channels[0].functions[0]
        .physical_mapping
        .as_mut()
        .unwrap()
        .revision += 1;
    let fixtures = [patched(&changed, 1)];
    let (_, summary) =
        build_mvr_document(&fixtures, &HashMap::new(), Vec::new(), &source, |_| None).unwrap();
    assert_eq!(summary.embedded_profiles, 0);
    assert_eq!(summary.missing_profiles.len(), 1);
    assert!(
        summary
            .warnings
            .iter()
            .any(|warning| warning.contains("piecewise physical curve"))
    );
}

#[test]
fn portable_gdtf_source_exports_without_a_library_and_rejects_edited_metadata() {
    let mut profile = profile("Portable source", 1);
    let mut second = profile.modes[0].clone();
    second.id = Uuid::new_v4();
    second.name = "Other mode".into();
    profile.modes.push(second);
    let source = light_fixture::gdtf::profile::package_profile(&profile).unwrap();
    profile.source_gdtf =
        Some(light_fixture::ProfileGdtfSource::associate(&profile, &source).unwrap());
    let fixtures = [patched(&profile, 1)];
    let (document, summary) = build_mvr_document(
        &fixtures,
        &HashMap::new(),
        Vec::new(),
        &Retained(HashMap::new()),
        |_| None,
    )
    .unwrap();
    assert_eq!(summary.embedded_profiles, 1);
    assert_eq!(document.files[&document.fixtures[0].gdtf_spec], source);
    profile.optics.beam_angle_degrees = Some(37.0);
    let (document, summary) = build_mvr_document(
        &[patched(&profile, 1)],
        &HashMap::new(),
        Vec::new(),
        &Retained(HashMap::new()),
        |_| None,
    )
    .unwrap();
    assert_eq!(summary.embedded_profiles, 0);
    assert_eq!(summary.generated_profiles, 1);
    assert!(light_mvr::read_gdtf(&document.files[&document.fixtures[0].gdtf_spec]).is_ok());
    assert!(
        summary
            .warnings
            .iter()
            .any(|warning| warning.contains("does not match"))
    );
}

#[test]
fn three_hundred_fixtures_share_one_archive_and_restore_the_original_association() {
    let mut profile = profile("Shared source", 1);
    let mut source_type = gdtf::profile::fixture_type(&profile).unwrap();
    let mut state = 7u32;
    source_type.description = (0..131072)
        .map(|_| {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
                [(state >> 24) as usize % 62] as char
        })
        .collect();
    let source = gdtf::package(&source_type).unwrap();
    profile.source_gdtf =
        Some(light_fixture::ProfileGdtfSource::associate(&profile, &source).unwrap());
    let template = patched(&profile, 1).1;
    let fixtures: Vec<_> = (1..=300)
        .map(|number| {
            let mut fixture = template.clone();
            fixture.fixture_id = light_core::FixtureId(Uuid::new_v4());
            fixture.fixture_number = Some(number);
            (fixture.fixture_id.0.to_string(), fixture)
        })
        .collect();
    let (document, summary) = build_mvr_document(
        &fixtures,
        &HashMap::new(),
        Vec::new(),
        &Retained(HashMap::new()),
        |_| None,
    )
    .unwrap();
    assert_eq!(summary.embedded_profiles, 300);
    assert_eq!(
        document
            .files
            .values()
            .filter(|bytes| **bytes == source)
            .count(),
        1
    );
    let json = &document.files[TOSKLIGHT_MVR_FIXTURE_METADATA_PATH];
    assert!(!String::from_utf8_lossy(json).contains("data:application/vnd.gdtf;base64,"));
    assert!(
        json.len() < source.len() * 30,
        "metadata {} must not repeat source {} per fixture",
        json.len(),
        source.len()
    );
    // Exercise the actual ZIP codec: it canonicalizes mixed-case archive keys on read.
    let document = light_mvr::read(&light_mvr::write(&document).unwrap()).unwrap();
    let restored = tosklight_mvr_fixture_metadata(&document);
    assert_eq!(restored.len(), 300);
    let attachments: Vec<_> = restored
        .values()
        .map(|entry| {
            entry
                .fixture
                .definition
                .profile_snapshot
                .as_ref()
                .unwrap()
                .source_gdtf
                .as_ref()
                .unwrap()
        })
        .collect();
    assert_eq!(attachments[0].decoded_archive().unwrap(), source);
    assert_eq!(
        attachments[0].profile_fingerprint,
        profile.source_gdtf.as_ref().unwrap().profile_fingerprint
    );
    assert!(attachments.iter().all(|attachment| std::sync::Arc::ptr_eq(
        &attachments[0].archive_asset,
        &attachment.archive_asset
    )));
}

#[test]
fn shared_mvr_source_keeps_distinct_stale_and_unverified_associations() {
    let mut first = physical_profile();
    let source = rich_source(&first);
    first.source_gdtf = Some(light_fixture::ProfileGdtfSource::associate(&first, &source).unwrap());
    // Both current profiles have changed, so standard export generates their current output.
    first.name = "Edited first".into();
    let mut second = first.clone();
    second.name = "Edited second".into();
    second.source_gdtf.as_mut().unwrap().profile_fingerprint = None;
    let fixtures = [patched(&first, 1), patched(&second, 2)];
    let (mut document, summary) = build_mvr_document(
        &fixtures,
        &HashMap::new(),
        Vec::new(),
        &Retained(HashMap::new()),
        |_| None,
    )
    .unwrap();
    assert_eq!(summary.generated_profiles, 2);
    assert_eq!(summary.embedded_profiles, 0);
    let metadata: ToskLightMvrFixtureMetadata =
        serde_json::from_slice(&document.files[TOSKLIGHT_MVR_FIXTURE_METADATA_PATH]).unwrap();
    assert_eq!(metadata.gdtf_sources.len(), 2);
    let archive_paths: HashSet<_> = metadata
        .gdtf_sources
        .values()
        .map(|source| source.archive_asset.clone())
        .collect();
    assert_eq!(archive_paths.len(), 1);
    let path = archive_paths.iter().next().unwrap().to_string();
    assert!(path.starts_with("tosklight/source-"));
    assert_eq!(document.files[&path], source);
    let restored = tosklight_mvr_fixture_metadata(&document);
    for (index, profile) in [first, second].iter().enumerate() {
        let actual = restored[&fixtures[index].1.fixture_id.0]
            .fixture
            .definition
            .profile_snapshot
            .as_ref()
            .unwrap();
        assert_eq!(actual.source_gdtf, profile.source_gdtf);
        assert!(
            !actual
                .source_gdtf
                .as_ref()
                .unwrap()
                .matches_profile(actual)
                .unwrap()
        );
    }
    document.files.insert(path, b"corrupt".to_vec());
    assert!(
        tosklight_mvr_fixture_metadata(&document).is_empty(),
        "invalid ancillary evidence must not silently restore a different source"
    );
}

#[test]
fn mvr_type_cache_distinguishes_source_only_optics_and_association_state() {
    let mut first = physical_profile();
    let source_a = rich_source(&first);
    first.source_gdtf =
        Some(light_fixture::ProfileGdtfSource::associate(&first, &source_a).unwrap());
    let mut unverified = first.clone();
    unverified.source_gdtf.as_mut().unwrap().profile_fingerprint = None;
    let mut source_type = gdtf::profile::fixture_type(&first).unwrap();
    source_type
        .description
        .push_str(" Different source-only optical measurements");
    let source_b = gdtf::package(&source_type).unwrap();
    let mut other_optics = first.clone();
    other_optics.source_gdtf =
        Some(light_fixture::ProfileGdtfSource::associate(&other_optics, &source_b).unwrap());
    assert_eq!(
        fixture_profile_source_fingerprint(&first).unwrap(),
        fixture_profile_source_fingerprint(&other_optics).unwrap()
    );
    let fixtures = [
        patched(&first, 1),
        patched(&unverified, 2),
        patched(&other_optics, 3),
    ];
    let (document, summary) = build_mvr_document(
        &fixtures,
        &HashMap::new(),
        Vec::new(),
        &Retained(HashMap::new()),
        |_| None,
    )
    .unwrap();
    assert_eq!(summary.embedded_profiles, 2);
    assert_eq!(summary.generated_profiles, 1);
    let specs: Vec<_> = document
        .fixtures
        .iter()
        .map(|fixture| fixture.gdtf_spec.clone())
        .collect();
    assert_eq!(specs.iter().collect::<HashSet<_>>().len(), 3);
    assert_eq!(document.files[&specs[0]], source_a);
    assert_ne!(document.files[&specs[1]], source_a);
    assert_eq!(document.files[&specs[2]], source_b);
    assert_eq!(
        document
            .files
            .values()
            .filter(|bytes| **bytes == source_a)
            .count(),
        1
    );
    let restored = tosklight_mvr_fixture_metadata(&document);
    for (fixture, expected) in fixtures.iter().zip([&first, &unverified, &other_optics]) {
        assert_eq!(
            restored[&fixture.1.fixture_id.0]
                .fixture
                .definition
                .profile_snapshot
                .as_ref()
                .unwrap()
                .source_gdtf,
            expected.source_gdtf
        );
    }
}
