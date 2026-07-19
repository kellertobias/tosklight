use super::super::{parse_fixture_selection, resolve_fixture_reference};
use super::*;
use light_core::FixtureId;
use light_fixture::PatchedFixture;
use light_show::ShowStore;

fn assert_numbering(fixtures: &[PatchedFixture]) {
    let mut fresnels = fixtures
        .iter()
        .filter(|fixture| fixture.name.starts_with("Front Fresnel"))
        .collect::<Vec<_>>();
    fresnels.sort_by(|left, right| left.name.cmp(&right.name));
    assert_eq!(
        fresnels
            .iter()
            .map(|fixture| fixture.fixture_number.unwrap())
            .collect::<Vec<_>>(),
        vec![1, 2, 3, 4, 5, 6]
    );
    let numbers = fixtures
        .iter()
        .map(|fixture| fixture.fixture_number.unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(numbers.len(), fixtures.len());
    for (prefix, expected) in [
        ("Back Profile", 101..=108),
        ("Back LED Wash", 201..=205),
        ("Back Trackspot", 301..=304),
        ("Floor RGBW PAR", 401..=412),
        ("Back RGB Sunstrip", 501..=506),
    ] {
        let actual = fixtures
            .iter()
            .filter(|fixture| fixture.name.starts_with(prefix))
            .map(|fixture| fixture.fixture_number.unwrap())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(actual, expected.collect());
    }
    for (name, number) in [
        ("Stage Hazer", 99),
        ("Middle ACL Set", 28),
        ("Outside ACL Set", 29),
        ("Overhead RGB Multi-patch", 999),
    ] {
        assert_eq!(
            fixtures
                .iter()
                .find(|fixture| fixture.name == name)
                .unwrap()
                .fixture_number,
            Some(number)
        );
    }
}

fn assert_hazer(fixtures: &[PatchedFixture]) {
    let hazer = fixtures
        .iter()
        .find(|fixture| fixture.name == "Stage Hazer")
        .unwrap();
    assert_eq!((hazer.universe, hazer.address), (Some(1), Some(13)));
    assert_eq!(hazer.definition.footprint, 2);
    assert_eq!(
        hazer.definition.heads[0]
            .parameters
            .iter()
            .map(|parameter| parameter.attribute.0.as_str())
            .collect::<Vec<_>>(),
        vec!["fog", "fan"]
    );
}

fn sunstrip(fixtures: &[PatchedFixture], number: u32) -> &PatchedFixture {
    fixtures
        .iter()
        .find(|fixture| fixture.fixture_number == Some(number))
        .unwrap()
}

fn assert_sunstrip_definition(fixtures: &[PatchedFixture]) {
    let sunstrip = sunstrip(fixtures, 501);
    assert_eq!(sunstrip.logical_heads.len(), 10);
    assert!(sunstrip.definition.heads.iter().all(|head| {
        head.parameters.iter().any(|parameter| {
            parameter.attribute.is_intensity()
                && parameter.virtual_dimmer
                && parameter.components.is_empty()
        }) && head
            .parameters
            .iter()
            .filter(|parameter| parameter.attribute.0.starts_with("color."))
            .all(|parameter| parameter.virtual_dimmer)
    }));
    assert_eq!(
        resolve_fixture_reference(fixtures, "501.2").unwrap(),
        sunstrip
            .logical_heads
            .iter()
            .find(|head| head.head_index == 1)
            .unwrap()
            .fixture_id
    );
    assert_eq!(
        parse_fixture_selection(fixtures, &["501".into(), ".".into(), "2".into()]).unwrap(),
        vec![
            sunstrip
                .logical_heads
                .iter()
                .find(|head| head.head_index == 1)
                .unwrap()
                .fixture_id
        ]
    );
}

fn logical_children(fixture: &PatchedFixture) -> Vec<FixtureId> {
    fixture
        .logical_heads
        .iter()
        .map(|head| head.fixture_id)
        .collect()
}

fn assert_sunstrip_selection(fixtures: &[PatchedFixture]) {
    let first = sunstrip(fixtures, 501);
    let second = sunstrip(fixtures, 502);
    let first_children = logical_children(first);
    let second_children = logical_children(second);
    assert_eq!(
        parse_fixture_selection(fixtures, &["501".into()]).unwrap(),
        first_children
    );
    assert_eq!(
        parse_fixture_selection(fixtures, &["501".into(), "THRU".into(), "502".into()]).unwrap(),
        first_children
            .iter()
            .chain(&second_children)
            .copied()
            .collect::<Vec<_>>()
    );
    assert_eq!(
        parse_fixture_selection(
            fixtures,
            &[
                "501".into(),
                ".".into(),
                "0".into(),
                "THRU".into(),
                "502".into(),
                ".".into(),
                "0".into(),
            ],
        )
        .unwrap(),
        vec![first.fixture_id, second.fixture_id]
    );
    assert_eq!(
        parse_fixture_selection(
            fixtures,
            &[
                "501".into(),
                ".".into(),
                "2".into(),
                "THRU".into(),
                "501".into(),
                ".".into(),
                "4".into(),
            ],
        )
        .unwrap(),
        first_children[1..4].to_vec()
    );
    assert_eq!(
        parse_fixture_selection(
            fixtures,
            &[
                "501".into(),
                "+".into(),
                "501".into(),
                ".".into(),
                "1".into()
            ],
        )
        .unwrap(),
        first_children
    );
    assert!(
        parse_fixture_selection(
            fixtures,
            &[
                "501".into(),
                ".".into(),
                "1".into(),
                "THRU".into(),
                "502".into(),
                ".".into(),
                "1".into(),
            ],
        )
        .is_err()
    );
    assert!(parse_fixture_selection(fixtures, &["501".into(), "+".into()]).is_err());
    assert!(resolve_fixture_reference(fixtures, "501.11").is_err());
}

fn assert_patch(fixtures: &[PatchedFixture]) {
    for (name, expected_universe, expected_address) in [
        ("Front Fresnel 1", 1, 1),
        ("Front Fresnel 6", 1, 6),
        ("Middle ACL Set", 1, 11),
        ("Outside ACL Set", 1, 12),
        ("Back Profile 1", 2, 1),
        ("Back LED Wash 1", 2, 49),
        ("Back Trackspot 1", 2, 79),
        ("Floor RGBW PAR 1", 3, 1),
        ("Back RGB Sunstrip 1", 3, 61),
        ("Front RGB Strobe 1", 3, 241),
        ("Overhead RGB Multi-patch", 4, 1),
    ] {
        let fixture = fixtures
            .iter()
            .find(|fixture| fixture.name == name)
            .unwrap();
        assert_eq!(
            (fixture.universe, fixture.address),
            (Some(expected_universe), Some(expected_address)),
            "unexpected patch for {name}"
        );
    }
    assert_eq!(
        fixtures
            .iter()
            .filter_map(|fixture| fixture.universe)
            .collect::<std::collections::BTreeSet<_>>(),
        std::collections::BTreeSet::from([1, 2, 3, 4])
    );
    let mut occupied = std::collections::BTreeSet::new();
    for fixture in fixtures {
        for channel in
            fixture.address.unwrap()..fixture.address.unwrap() + fixture.definition.footprint
        {
            assert!(
                occupied.insert((fixture.universe.unwrap(), channel)),
                "overlap at {}.{channel}",
                fixture.universe.unwrap()
            );
        }
    }
}

fn assert_layout(store: &ShowStore, fixtures: &[PatchedFixture]) {
    let layout = store.objects("stage_layout").unwrap().pop().unwrap().body;
    assert_eq!(layout["positions3d"].as_object().unwrap().len(), 70);
    assert!(layout.get("assets").is_none());
    let multipatched = fixtures
        .iter()
        .filter(|fixture| !fixture.multipatch.is_empty())
        .collect::<Vec<_>>();
    assert_eq!(multipatched.len(), 3);
    assert!(
        multipatched
            .iter()
            .all(|fixture| fixture.multipatch.len() == 7)
    );
    assert_eq!(
        fixtures
            .iter()
            .filter(|fixture| fixture.definition.device_type == "scanner")
            .count(),
        4
    );
}

#[test]
fn seeds_the_complete_non_overlapping_default_rig() {
    let path = std::env::temp_dir().join(format!(
        "tosklight-default-show-{}.show",
        uuid::Uuid::new_v4()
    ));
    initialise(&path).unwrap();
    let store = ShowStore::open(&path).unwrap();
    let fixtures = store
        .objects("patched_fixture")
        .unwrap()
        .into_iter()
        .map(|object| serde_json::from_value::<PatchedFixture>(object.body).unwrap())
        .collect::<Vec<_>>();
    light_fixture::validate_patch(&fixtures).unwrap();
    assert_eq!(fixtures.len(), 49);
    assert_numbering(&fixtures);
    assert_hazer(&fixtures);
    assert_sunstrip_definition(&fixtures);
    assert_sunstrip_selection(&fixtures);
    assert_patch(&fixtures);
    assert_layout(&store, &fixtures);
    drop(store);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn upgrades_the_legacy_single_universe_default_patch() {
    let path = std::env::temp_dir().join(format!(
        "tosklight-default-show-upgrade-{}.show",
        uuid::Uuid::new_v4()
    ));
    initialise(&path).unwrap();
    let store = ShowStore::open(&path).unwrap();
    for object in store.objects("patched_fixture").unwrap() {
        let mut fixture: PatchedFixture = serde_json::from_value(object.body).unwrap();
        fixture.universe = Some(1);
        fixture.split_patches = vec![light_fixture::SplitPatch {
            split: 1,
            universe: Some(1),
            address: fixture.address,
        }];
        store
            .put_object(
                "patched_fixture",
                &object.id,
                &serde_json::to_value(fixture).unwrap(),
                object.revision,
            )
            .unwrap();
    }
    let document = store.portable_document().unwrap();
    let mut transaction = document.transaction();
    stage_upgrade(&document, &mut transaction).unwrap();
    store.apply_portable_transaction(transaction).unwrap();
    drop(store);
    let fixtures = ShowStore::open(&path)
        .unwrap()
        .objects("patched_fixture")
        .unwrap()
        .into_iter()
        .map(|object| serde_json::from_value::<PatchedFixture>(object.body).unwrap())
        .collect::<Vec<_>>();
    for (name, expected) in [
        ("Middle ACL Set", (Some(1), Some(11))),
        ("Back Profile 1", (Some(2), Some(1))),
        ("Floor RGBW PAR 1", (Some(3), Some(1))),
        ("Overhead RGB Multi-patch", (Some(4), Some(1))),
    ] {
        let fixture = fixtures
            .iter()
            .find(|fixture| fixture.name == name)
            .unwrap();
        assert_eq!((fixture.universe, fixture.address), expected);
        assert_eq!(
            (
                fixture.split_patches[0].universe,
                fixture.split_patches[0].address
            ),
            expected
        );
    }
    std::fs::remove_file(path).unwrap();
}
