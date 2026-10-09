use super::*;
use std::cell::Cell;

fn catalog(name: &str) -> FixtureProfile {
    let mut profile = FixtureProfile::blank();
    profile.revision = 1;
    profile.manufacturer = "Maker".into();
    profile.name = name.into();
    profile.short_name = format!("{name} Short");
    profile.modes[0].name = "Standard".into();
    profile
}
fn source(spec: &str) -> light_mvr::MvrFixture {
    let mut fixture = mvr_fixture(Uuid::new_v4(), "Source", 1, 1);
    fixture.gdtf_spec = spec.into();
    fixture.gdtf_mode = "standard".into();
    fixture
}

#[test]
fn mvr_binding_skips_unrelated_invalid_library_projection() {
    let profile = catalog("Rig");
    let mut unrelated = catalog("Other");
    unrelated.modes[0].name.clear();
    assert!(unrelated.validate().is_err());
    let document = light_mvr::MvrDocument {
        fixtures: vec![source("Rig.gdtf")],
        ..Default::default()
    };
    let bound = bind_mvr_sources(
        &document,
        &[unrelated.clone(), profile.clone()],
        &[],
        |_, _| Ok(Some(profile.clone())),
        |_| vec![],
    )
    .unwrap();
    assert_eq!(bound.definitions.len(), 1);
    assert!(
        bind_mvr_sources(
            &light_mvr::MvrDocument::default(),
            &[unrelated],
            &[],
            |_, _| panic!("empty MVR must not resolve library revisions"),
            |_| vec![]
        )
        .unwrap()
        .definitions
        .is_empty()
    );
}

#[test]
fn mvr_binding_matches_existing_aliases_and_caches_authoritative_revision() {
    let profile = catalog("Rig");
    let mut immutable = profile.clone();
    immutable.notes = "Full immutable snapshot".into();
    let calls = Cell::new(0);
    let sources = vec![
        source("folder\\RIG.GDTF"),
        source("Rig Short.gdtf"),
        source("Maker@Rig Short.gdtf"),
    ];
    let document = light_mvr::MvrDocument {
        fixtures: sources,
        ..Default::default()
    };
    let bound = bind_mvr_sources(
        &document,
        &[profile.clone()],
        &[],
        |id, revision| {
            assert_eq!(id, profile.id);
            assert_eq!(revision, profile.revision);
            calls.set(calls.get() + 1);
            Ok(Some(immutable.clone()))
        },
        |_| vec![],
    )
    .unwrap();
    assert_eq!(bound.definitions.len(), 3);
    assert_eq!(
        calls.get(),
        1,
        "project/resolve only the matched immutable mode once"
    );
    for definition in bound.definitions.values() {
        assert_eq!(definition.profile_id, Some(profile.id));
        assert_eq!(definition.mode_id, Some(profile.modes[0].id));
        assert_eq!(
            definition.profile_snapshot.as_ref().unwrap().notes,
            immutable.notes
        );
    }
}

#[test]
fn mvr_binding_preserves_ambiguity_mode_and_legacy_dedup_rules() {
    let profile = catalog("Rig");
    let duplicate = catalog("Rig");
    let mut wrong = source("Rig.gdtf");
    wrong.gdtf_mode = "Missing".into();
    let document = light_mvr::MvrDocument {
        fixtures: vec![source("Rig.gdtf"), wrong],
        ..Default::default()
    };
    let bound = bind_mvr_sources(
        &document,
        &[profile.clone(), duplicate],
        &[],
        |_, _| panic!("ambiguous/missing modes must not resolve"),
        |_| vec![],
    )
    .unwrap();
    assert!(bound.definitions.is_empty());
    assert_eq!(bound.warnings.len(), 2);
    let mut shadow = fixture_definition(1);
    shadow.profile_id = None;
    shadow.mode_id = None;
    shadow.id = profile.id;
    shadow.model = "Legacy".into();
    let document = light_mvr::MvrDocument {
        fixtures: vec![source("Legacy.gdtf")],
        ..Default::default()
    };
    let bound = bind_mvr_sources(
        &document,
        &[profile],
        &[shadow.clone()],
        |_, _| panic!("shadowed legacy must not resolve"),
        |_| vec![],
    )
    .unwrap();
    assert!(bound.definitions.is_empty());
    shadow.id = FixtureId::new();
    let bound = bind_mvr_sources(
        &document,
        &[],
        &[shadow.clone()],
        |_, _| panic!("legacy definition is already authoritative"),
        |_| vec![],
    )
    .unwrap();
    assert_eq!(bound.definitions.values().next().unwrap().id, shadow.id);
    let mut later = shadow.clone();
    later.model = "Hidden".into();
    let hidden = light_mvr::MvrDocument {
        fixtures: vec![source("Hidden.gdtf")],
        ..Default::default()
    };
    let bound = bind_mvr_sources(
        &hidden,
        &[],
        &[shadow, later],
        |_, _| panic!("legacy definitions are authoritative"),
        |_| vec![],
    )
    .unwrap();
    assert!(
        bound.definitions.is_empty(),
        "first legacy definition for an ID retains precedence"
    );
}

#[test]
fn mvr_binding_still_rejects_invalid_matched_authoritative_revision() {
    let profile = catalog("Rig");
    let mut invalid = profile.clone();
    invalid.modes[0].name.clear();
    assert!(invalid.validate().is_err());
    let document = light_mvr::MvrDocument {
        fixtures: vec![source("Rig.gdtf")],
        ..Default::default()
    };
    assert!(
        bind_mvr_sources(
            &document,
            &[profile],
            &[],
            |_, _| Ok(Some(invalid.clone())),
            |_| vec![]
        )
        .is_err()
    );
}

#[test]
fn mvr_binding_caches_each_mode_and_keeps_legacy_revision_validation() {
    let mut profile = catalog("Rig");
    let mut second = profile.modes[0].clone();
    second.id = Uuid::new_v4();
    second.name = "Extended".into();
    profile.modes.push(second);
    let mut extended = source("Rig.gdtf");
    extended.gdtf_mode = "EXTENDED".into();
    let document = light_mvr::MvrDocument {
        fixtures: vec![source("Rig.gdtf"), extended],
        ..Default::default()
    };
    let calls = Cell::new(0);
    let bound = bind_mvr_sources(
        &document,
        &[profile.clone()],
        &[],
        |_, _| {
            calls.set(calls.get() + 1);
            Ok(Some(profile.clone()))
        },
        |_| vec![],
    )
    .unwrap();
    assert_eq!(calls.get(), 2);
    assert_eq!(
        bound
            .definitions
            .values()
            .map(|definition| definition.mode_id.unwrap())
            .collect::<std::collections::HashSet<_>>()
            .len(),
        2
    );
    let legacy = profile.resolved_definition(profile.modes[0].id).unwrap();
    let repeated = light_mvr::MvrDocument {
        fixtures: vec![source("Rig.gdtf"), source("Rig.gdtf")],
        ..Default::default()
    };
    calls.set(0);
    assert_eq!(
        bind_mvr_sources(
            &repeated,
            &[],
            &[legacy.clone()],
            |_, _| {
                calls.set(calls.get() + 1);
                Ok(Some(profile.clone()))
            },
            |_| vec![]
        )
        .unwrap()
        .definitions
        .len(),
        2
    );
    assert_eq!(calls.get(), 1);
    let mut wrong_revision = profile.clone();
    wrong_revision.revision += 1;
    assert!(
        bind_mvr_sources(
            &repeated,
            &[],
            &[legacy],
            |_, _| Ok(Some(wrong_revision.clone())),
            |_| vec![]
        )
        .is_err()
    );
}
