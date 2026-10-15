use super::*;
use crate::EngineSnapshot;
use light_core::SessionId;
use light_fixture::{FixtureProfile, PatchedFixture, PatchedHead};
use light_programmer::ProgrammerRegistry;

struct ReportRig {
    engine: Engine,
    programmers: ProgrammerRegistry,
    session: SessionId,
    roots: [FixtureId; 2],
    owners: [Vec<FixtureId>; 2],
}

fn report_rig() -> ReportRig {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../assets/fixture-library/showtec--sunstrip-led-rgb-42206.toskfixture");
    let profile: FixtureProfile =
        light_fixture::read_fixture_package(&std::fs::read(path).unwrap()).unwrap();
    let mode = &profile.modes[0];
    let roots = [FixtureId::new(), FixtureId::new()];
    let mut owners = [Vec::new(), Vec::new()];
    let fixtures = roots
        .iter()
        .enumerate()
        .map(|(index, root)| {
            let logical_heads = mode
                .heads
                .iter()
                .enumerate()
                .filter(|(_, head)| !head.master_shared)
                .map(|(head_index, head)| PatchedHead {
                    profile_head_id: Some(head.id),
                    head_index: head_index as u16,
                    fixture_id: FixtureId::new(),
                })
                .collect::<Vec<_>>();
            owners[index] = mode
                .heads
                .iter()
                .enumerate()
                .map(|(head_index, _)| {
                    logical_heads
                        .iter()
                        .find(|head| head.head_index == head_index as u16)
                        .map_or(*root, |head| head.fixture_id)
                })
                .collect();
            serde_json::from_value::<PatchedFixture>(serde_json::json!({
                "fixture_id":root, "definition":profile.resolved_definition(mode.id).unwrap(),
                "universe":1, "address":1 + index * 100, "logical_heads":logical_heads
            }))
            .unwrap()
        })
        .collect::<Vec<_>>();
    let programmers = ProgrammerRegistry::default();
    let session = SessionId::new();
    programmers.start(session);
    let engine = Engine::new(programmers.clone());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: fixtures.into(),
            ..Default::default()
        })
        .unwrap();
    ReportRig {
        engine,
        programmers,
        session,
        roots,
        owners,
    }
}

fn assert_owners(
    rig: &ReportRig,
    wanted: Option<&HashSet<FixtureId>>,
    expected: &[(FixtureId, FixtureId)],
) {
    let (_, accepted) = rig.engine.color_report_heads(wanted);
    assert_eq!(
        accepted
            .iter()
            .map(|head| (head.fixture_id, head.owner))
            .collect::<Vec<_>>(),
        expected
    );
    let legacy = rig.engine.color_intent_report(wanted).unwrap();
    assert_eq!(
        legacy
            .iter()
            .map(|head| (head.fixture_id, head.owner))
            .collect::<Vec<_>>(),
        expected
    );
}

#[test]
fn mixed_root_and_logical_selection_includes_exact_heads_without_siblings_or_duplicates() {
    let rig = report_rig();
    let wanted = HashSet::from([rig.roots[0], rig.owners[0][2], rig.owners[1][3]]);
    let mut expected = rig.owners[0]
        .iter()
        .map(|owner| (rig.roots[0], *owner))
        .collect::<Vec<_>>();
    expected.push((rig.roots[1], rig.owners[1][3]));
    assert_owners(&rig, Some(&wanted), &expected);
}

#[test]
fn logical_selection_alone_preserves_profile_order_and_current_owner_target() {
    let rig = report_rig();
    let target = Xyz {
        x: 0.2,
        y: 0.1,
        z: 0.3,
    };
    rig.programmers.set(
        rig.session,
        rig.owners[0][2],
        AttributeKey::color(),
        AttributeValue::ColorXyz(target),
    );
    let wanted = HashSet::from([rig.owners[0][4], rig.owners[0][2]]);
    assert_owners(
        &rig,
        Some(&wanted),
        &[
            (rig.roots[0], rig.owners[0][2]),
            (rig.roots[0], rig.owners[0][4]),
        ],
    );
    let legacy = rig.engine.color_intent_report(Some(&wanted)).unwrap();
    assert_eq!(legacy[0].target, Some(target));
    assert_eq!(legacy[1].target, None);
}

#[test]
fn root_and_unfiltered_reports_keep_all_heads_while_empty_or_unknown_selection_stays_empty() {
    let rig = report_rig();
    let first = rig.owners[0]
        .iter()
        .map(|owner| (rig.roots[0], *owner))
        .collect::<Vec<_>>();
    assert_owners(&rig, Some(&HashSet::from([rig.roots[0]])), &first);
    let all = first
        .into_iter()
        .chain(rig.owners[1].iter().map(|owner| (rig.roots[1], *owner)))
        .collect::<Vec<_>>();
    assert_owners(&rig, None, &all);
    assert_owners(&rig, Some(&HashSet::new()), &[]);
    assert_owners(&rig, Some(&HashSet::from([FixtureId::new()])), &[]);
}
