//! Aim reads a Point and its target from one captured engine source frame.

use super::*;
use light_core::{AttributeKey, AttributeValue, FixtureId};

fn package(name: &str) -> light_fixture::FixtureProfile {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../assets/fixture-library")
        .join(name);
    light_fixture::read_fixture_package(&std::fs::read(path).unwrap()).unwrap()
}

#[test]
fn aim_follows_a_nonshared_point_and_old_source_frames_remain_coherent() {
    // An older contract-0 engine (TL-552: no longer production) keeps the scalar Aim coordinate
    // boundary; this pins that compatibility path.
    let (state, data_dir) =
        test_state_with_programming_contract(ProgrammerRegistry::default(), None, 0);
    let point_id = FixtureId::new();
    let mover_id = FixtureId::new();
    let target_id = FixtureId::new();

    let point_profile = package("tosklight--3d-point.toskfixture");
    let mut point = operational_fixture(point_id);
    point.fixture_number = Some(6);
    let point_mode = &point_profile.modes[0];
    point.definition = point_profile.resolved_definition(point_mode.id).unwrap();
    point.logical_heads = point_mode
        .heads
        .iter()
        .enumerate()
        .filter(|(_, head)| !head.master_shared)
        .map(|(index, head)| light_fixture::PatchedHead {
            profile_head_id: Some(head.id),
            head_index: index as u16,
            fixture_id: FixtureId::new(),
        })
        .collect();
    point.universe = None;
    point.address = None;

    let mover_profile = package("jb-lighting--jbled-a7.toskfixture");
    let mut mover = operational_fixture(mover_id);
    mover.definition = mover_profile
        .resolved_definition(mover_profile.modes[0].id)
        .unwrap();
    mover.fixture_number = Some(1);
    mover.address = Some(1);

    let mut target = operational_fixture(target_id);
    target.fixture_number = Some(5);
    target.address = Some(100);
    target.position_master = Some(point_id.0);
    state
        .output
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![point, mover, target].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();

    let held = |attribute: &str, value| {
        light_engine::TrackedOverride::new(
            point_id,
            AttributeKey(attribute.into()),
            AttributeValue::Normalized(value),
        )
    };
    state
        .output
        .engine()
        // Keep both targets inside the shipped profile's positive Pan range; negative Pan
        // clips to the same stop and cannot establish that the target transform changed.
        .set_tracked_overrides([held("point.position.x", 0.45)]);
    let captured = state.output.engine().observe_source_frame(&[]);
    assert_eq!(captured.points().len(), 1);
    assert_eq!(captured.points()[0].fixture_id, point_id);
    assert!((captured.points()[0].offset_metres[0] + 10.0).abs() < 0.0001);
    let captured_target = captured
        .mounts()
        .mount(target_id.0)
        .unwrap()
        .world_from_fixture
        .unwrap();
    assert!((captured_target.point([0.0; 3])[0] + 10.0).abs() < 0.0001);
    assert!(!captured.values().materialised_by_name());
    let old_aim =
        super::super::programmer_aim_command::aim_selection(&state, &[mover_id], 5).unwrap();
    assert_eq!(old_aim.len(), 2);
    let direct_point_aim =
        super::super::programmer_aim_command::aim_selection(&state, &[mover_id], 6).unwrap();
    assert_eq!(direct_point_aim, old_aim);

    state.output.engine().set_tracked_overrides([
        held("point.position.x", 0.45),
        held("point.position.y", 0.55),
    ]);
    let mut changed = (*state.output.snapshot()).clone();
    Arc::make_mut(&mut changed.fixtures)[0].location.x = 9_000;
    state.output.replace_snapshot(changed).unwrap();
    let new_aim =
        super::super::programmer_aim_command::aim_selection(&state, &[mover_id], 5).unwrap();
    assert_eq!(new_aim.len(), 2);
    assert_ne!(old_aim, new_aim);
    let new_direct_point_aim =
        super::super::programmer_aim_command::aim_selection(&state, &[mover_id], 6).unwrap();
    assert_eq!(new_direct_point_aim.len(), 2);
    assert_ne!(new_direct_point_aim, new_aim);

    // The older values and Point projection still belong to the older fixture generation.
    assert_eq!(captured.snapshot().fixtures[0].location.x, 0);
    assert!((captured.points()[0].offset_metres[0] + 10.0).abs() < 0.0001);
    assert_eq!(captured.points()[0].offset_metres[1], 0.0);
    assert_eq!(
        captured
            .mounts()
            .mount(target_id.0)
            .unwrap()
            .world_from_fixture,
        Some(captured_target)
    );
    assert!(!captured.values().materialised_by_name());
    let _ = std::fs::remove_dir_all(data_dir);
}

/// NOTICE-002: with nothing selected, Aim at a real target is a quiet no-op, but a missing Aim
/// target, whether typed or stored in a stale Aim preset, stays an actionable error. Neither
/// path may touch the selection, the Programmer or the output.
#[test]
fn empty_source_aim_validates_the_explicit_target_before_the_quiet_no_op() {
    // Preserve the still-supported scalar production contract. Contract1 intentionally retains
    // an empty live Group's semantic Target; dedicated semantic Aim tests cover that behavior.
    let (state, data_dir) =
        test_state_with_programming_contract(ProgrammerRegistry::default(), None, 0);
    let session = Session {
        capability: light_core::SurfaceCapability::Programming,
        id: SessionId::new(),
        token: "aim-empty-source".into(),
        connected: true,
        desk: test_control_desk(),
    };
    state.programming.start(session.id);
    state.sessions.insert_session(session.clone());
    let mut first = operational_fixture(FixtureId::new());
    first.fixture_number = Some(1);
    let mut target = operational_fixture(FixtureId::new());
    target.fixture_number = Some(2);
    target.address = Some(2);
    state
        .output
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![first, target].into(),
            groups: vec![light_programmer::GroupDefinition {
                id: "1".into(),
                ..Default::default()
            }]
            .into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    let show_path = data_dir.join("shows/aim-empty-source.show");
    let show_id = default_show::initialise_legacy_test_show(&show_path).unwrap();
    let entry = ShowEntry {
        is_base_show: false,
        id: show_id,
        name: "Aim empty source".into(),
        path: show_path.display().to_string(),
        revision: 0,
        updated_at: String::new(),
        created_at: None,
        last_loaded_at: None,
        revision_copy: None,
    };
    state.active_show.replace_current(Some(entry.clone()));
    let repository = ActiveShowRepository::open(&entry.path).unwrap();
    for (number, aim_at) in [(5, 998), (6, 2)] {
        let preset = light_programmer::Preset {
            family: light_programmer::PresetFamily::Position,
            number,
            aim_at_fixture_number: Some(aim_at),
            ..Default::default()
        };
        repository
            .put_object(
                "preset",
                &format!("3.{number}"),
                &serde_json::to_value(preset).unwrap(),
                0,
            )
            .unwrap();
    }

    let dmx =
        |state: &AppState| (*state.output.render(Default::default()).unwrap().universes).clone();
    let context = operator_action_context(&session, light_application::ActionSource::Http);
    for blind in [false, true] {
        state.programming.select(session.id, []);
        state
            .programming
            .set_modes(session.id, Some(blind), None, None, None);
        let before = state.programming.get(session.id).unwrap();
        let output_before = dmx(&state);
        for command in [
            "FIXTURE 999 AT FIXTURE 2",
            "AT 3.6",
            "FIXTURE 999 AT 3.6",
            "GROUP 1 AT 3.6",
        ] {
            assert_eq!(
                execute_programmer_command_from(&state, &session, command, &context),
                Ok(0),
                "{command}"
            );
        }
        for command in [
            "FIXTURE 999 AT FIXTURE 998",
            "AT 3.5",
            "FIXTURE 999 AT 3.5",
            "GROUP 1 AT 3.5",
        ] {
            let error = execute_programmer_command_from(&state, &session, command, &context)
                .expect_err(command);
            assert!(
                error.contains("no fixture numbered 998"),
                "{command}: {error}"
            );
        }
        let after = state.programming.get(session.id).unwrap();
        assert!(after.selected.is_empty());
        assert_eq!(
            serde_json::to_value(&after).unwrap(),
            serde_json::to_value(&before).unwrap()
        );
        assert_eq!(after.undo.len(), before.undo.len());
        assert_eq!(dmx(&state), output_before);
    }
    let _ = std::fs::remove_dir_all(data_dir);
}

/// Root Point plus a target attached away from its origin, and an unpatched moving light.
pub(super) fn install_semantic_aim_rig(state: &AppState) -> (FixtureId, FixtureId, FixtureId) {
    let point_id = FixtureId::new();
    let mover_id = FixtureId::new();
    let target_id = FixtureId::new();
    let profile = package("tosklight--3d-point.toskfixture");
    let mut point = operational_fixture(point_id);
    point.fixture_number = Some(901);
    point.location = light_fixture::FixtureLocation {
        x: 1000,
        y: 3000,
        z: 2000,
    };
    point.definition = profile.resolved_definition(profile.modes[0].id).unwrap();
    point.logical_heads = profile.modes[0]
        .heads
        .iter()
        .enumerate()
        .filter(|(_, head)| !head.master_shared)
        .map(|(index, head)| light_fixture::PatchedHead {
            profile_head_id: Some(head.id),
            head_index: index as u16,
            fixture_id: FixtureId::new(),
        })
        .collect();
    point.universe = None;
    point.address = None;
    let mover_profile = package("jb-lighting--jbled-a7.toskfixture");
    let mut mover = operational_fixture(mover_id);
    mover.fixture_number = Some(1);
    mover.definition = mover_profile
        .resolved_definition(mover_profile.modes[0].id)
        .unwrap();
    mover.location = light_fixture::FixtureLocation {
        x: -2000,
        y: 1000,
        z: 5000,
    };
    mover.universe = None;
    mover.address = None;
    let mut target = operational_fixture(target_id);
    target.fixture_number = Some(5);
    target.location = light_fixture::FixtureLocation {
        x: 4000,
        y: 7000,
        z: 6000,
    };
    target.position_master = Some(point_id.0);
    target.universe = None;
    target.address = None;
    state
        .output
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![point, mover, target].into(),
            groups: vec![light_programmer::GroupDefinition {
                id: "1".into(),
                fixtures: vec![mover_id],
                ..Default::default()
            }]
            .into(),
            revision: state.output.snapshot().revision + 1,
            ..Default::default()
        })
        .unwrap();
    (point_id, mover_id, target_id)
}

#[test]
fn semantic_aim_keeps_point_uuid_local_offsets_and_fixed_world_coordinates() {
    use light_core::programming::{PositionIntent, ScalarIntent, TargetReference};
    let (state, data_dir) = test_state();
    let (point, mover, target) = install_semantic_aim_rig(&state);
    let expected =
        PositionIntent::target(TargetReference::Point { point_id: point.0 }, [3., 4., 4.]);
    let intent = |number| {
        super::super::programmer_aim_command::aim_target_intent(&state, number)
            .unwrap()
            .unwrap()
    };
    assert_eq!(
        intent(901),
        PositionIntent::target(TargetReference::Point { point_id: point.0 }, [0.; 3])
    );
    assert_eq!(intent(5), expected);
    let first = super::super::programmer_aim_command::aim_selection(&state, &[mover], 5).unwrap();
    assert_eq!(
        first,
        vec![(
            mover,
            AttributeKey("position".into()),
            AttributeValue::Position(Arc::new(expected.clone()))
        )]
    );
    let revision = state.programming.normal_values_revision();
    state.output.engine().set_tracked_overrides(
        [
            ("point.position.x", 0.535),
            ("point.position.y", 0.465),
            ("point.rotation.x", 0.625),
            ("point.rotation.y", 0.4),
            ("point.rotation.z", 0.7),
        ]
        .map(|(key, value)| {
            light_engine::TrackedOverride::new(
                point,
                AttributeKey(key.into()),
                AttributeValue::Normalized(value),
            )
        }),
    );
    let PositionIntent::Target {
        reference,
        offset_metres,
    } = intent(5)
    else {
        unreachable!()
    };
    assert_eq!(reference, TargetReference::Point { point_id: point.0 });
    for (value, expected) in offset_metres.iter().zip([3., 4., 4.]) {
        let ScalarIntent::Value(value) = value else {
            unreachable!()
        };
        assert!((*value - expected).abs() < 0.00001);
    }
    assert_eq!(
        state.programming.normal_values_revision(),
        revision,
        "tracking/inspection never authors Position"
    );
    let mut snapshot = (*state.output.snapshot()).clone();
    Arc::make_mut(&mut snapshot.fixtures)
        .iter_mut()
        .find(|fixture| fixture.fixture_id == target)
        .unwrap()
        .position_master = None;
    state.output.replace_snapshot(snapshot).unwrap();
    assert_eq!(
        intent(5),
        PositionIntent::target(TargetReference::Origin, [4., 7., 6.])
    );
    let _ = std::fs::remove_dir_all(data_dir);
}
