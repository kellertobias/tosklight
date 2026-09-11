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
    let (document, summary) =
        build_mvr_document(&fixtures, &HashMap::new(), &Retained(HashMap::new())).unwrap();

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

#[test]
fn a_retained_source_is_embedded_unchanged_under_the_name_the_fixture_references() {
    let profile = profile("Spot", 1);
    let fixtures = [patched(&profile, 1)];
    let source = Retained(HashMap::from([(profile.id.0, b"source".to_vec())]));
    let (document, summary) = build_mvr_document(&fixtures, &HashMap::new(), &source).unwrap();

    assert_eq!(summary.embedded_profiles, 1);
    assert_eq!(summary.generated_profiles, 0);
    let spec = &document.fixtures[0].gdtf_spec;
    assert_eq!(
        spec, "Acme@Spot.gdtf",
        "case is preserved, unlike the old lowercased name"
    );
    assert_eq!(document.files[spec], b"source");
    assert_eq!(document.fixtures[0].gdtf_mode, "Mode — A");
}

#[test]
fn two_revisions_of_one_fixture_get_distinct_archive_names() {
    let first = profile("Wash", 1);
    let mut second = first.clone();
    second.revision = 2;
    let fixtures = [patched(&first, 1), patched(&second, 2)];
    let (document, _) =
        build_mvr_document(&fixtures, &HashMap::new(), &Retained(HashMap::new())).unwrap();
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
