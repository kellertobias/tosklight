//! TL-544 G3: typed Color, Position and Focus/Zoom values on the command line, keypad and OSC,
//! through the actual v2 command-line route and the Programmer values service.
use super::*;
use crate::runtime::output_scheduler::physical_adapters::color::profiles::{patched, rgbw};
use crate::runtime::output_scheduler::position_test_support as physical;
use light_core::programming::{ColorProgram, PositionIntent, ProgrammingOwner, ScalarIntent};
use light_core::{AttributeValue, FixtureId};

struct Rig {
    scenario: CommandHttpScenario,
    colors: [FixtureId; 3],
    mover: FixtureId,
}

/// Three RGBW fixtures (1-3, Group 1 in that order) and one calibrated moving head (4).
async fn rig() -> Rig {
    let scenario = CommandHttpScenario::new().await;
    let profile = rgbw();
    let colors = [FixtureId::new(), FixtureId::new(), FixtureId::new()];
    let mut fixtures = colors
        .iter()
        .enumerate()
        .map(|(index, id)| {
            let mut fixture = patched(&profile, *id, 1 + 20 * index as u16);
            fixture.fixture_number = Some(index as u32 + 1);
            fixture
        })
        .collect::<Vec<_>>();
    let mover = FixtureId::new();
    let mut head = physical::patched(&physical::moving_head(), mover, 100);
    head.fixture_number = Some(4);
    fixtures.push(head);
    scenario
        .state
        .output
        .replace_snapshot(EngineSnapshot {
            fixtures: fixtures.into(),
            groups: vec![light_programmer::GroupDefinition {
                id: "1".into(),
                name: "Colours".into(),
                fixtures: colors.to_vec(),
                ..Default::default()
            }]
            .into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    Rig {
        scenario,
        colors,
        mover,
    }
}

async fn run(scenario: &CommandHttpScenario, id: &str, command: &str) -> serde_json::Value {
    let response = scenario.execute(id, Some(command)).await;
    assert_eq!(response.status(), StatusCode::OK);
    json(response).await
}

async fn accepted(scenario: &CommandHttpScenario, id: &str, command: &str) {
    let body = run(scenario, id, command).await;
    assert_eq!(body["outcome"], "accepted", "{command}: {body}");
}

fn fixture_value(
    scenario: &CommandHttpScenario,
    fixture: FixtureId,
    owner: ProgrammingOwner,
) -> Option<AttributeValue> {
    scenario
        .state
        .programming
        .get(scenario.session.id)
        .unwrap()
        .values
        .iter()
        .find(|value| value.fixture_id == fixture && value.attribute == owner.key())
        .map(|value| value.value.clone())
}

fn rgb(value: Option<AttributeValue>) -> [f32; 3] {
    let Some(AttributeValue::ColorProgram(program)) = value else {
        panic!("a Color program, got {value:?}")
    };
    let ColorProgram::Semantic { intent } = program.as_ref() else {
        panic!("a semantic Color program")
    };
    intent.recipe.rgb
}

fn close(left: [f32; 3], right: [f32; 3]) -> bool {
    left.iter().zip(right).all(|(a, b)| (a - b).abs() < 1e-3)
}

#[tokio::test]
async fn color_tuples_spread_relative_and_undo_through_the_command_line() {
    let Rig {
        scenario, colors, ..
    } = rig().await;
    let depth = || {
        scenario
            .state
            .programming
            .undo_depth(scenario.session.id)
            .unwrap()
    };
    // The help's example: red via yellow to green over the ordered selection.
    let before = depth();
    accepted(
        &scenario,
        "spread",
        "FIXTURE 1 THRU 3 AT COLOR 100 DIV 0 DIV 0 THRU 0 DIV 100 DIV 0",
    )
    .await;
    let read = |fixture| rgb(fixture_value(&scenario, fixture, ProgrammingOwner::Color));
    assert!(
        close(read(colors[0]), [1., 0., 0.]),
        "{:?}",
        read(colors[0])
    );
    assert!(
        close(read(colors[1]), [0.5, 0.5, 0.]),
        "{:?}",
        read(colors[1])
    );
    assert!(
        close(read(colors[2]), [0., 1., 0.]),
        "{:?}",
        read(colors[2])
    );
    // One command is one Undo step.
    assert_eq!(depth(), before + 1);

    // Empty components keep their value; a leading + steps relatively.
    accepted(&scenario, "blue", "FIXTURE 2 AT COLOR DIV DIV 50").await;
    assert!(close(read(colors[1]), [0.5, 0.5, 0.5]));
    accepted(&scenario, "relative", "FIXTURE 3 AT COLOR + 20 DIV - 50").await;
    assert!(
        close(read(colors[2]), [0.2, 0.5, 0.]),
        "{:?}",
        read(colors[2])
    );

    // The current selection takes a bare `AT COLOR …`.
    accepted(&scenario, "select", "FIXTURE 1").await;
    accepted(&scenario, "current", "AT COLOR 0 DIV 0 DIV 100").await;
    assert!(close(read(colors[0]), [0., 0., 1.]));

    let undo = scenario.press_key(&scenario.token, "UND", "undo").await;
    assert_eq!(undo.status(), StatusCode::OK);
    assert!(close(read(colors[0]), [1., 0., 0.]));

    // Invalid values are rejected without touching the Programmer.
    for (id, command) in [
        ("too-many", "FIXTURE 1 AT COLOR 1 DIV 2 DIV 3 DIV 4 DIV 5"),
        ("range", "FIXTURE 1 AT COLOR 150"),
        ("relative-spread", "FIXTURE 1 THRU 3 AT COLOR + 10 THRU 20"),
        (
            "partial-spread",
            "FIXTURE 1 THRU 3 AT COLOR 10 DIV 5 THRU 20",
        ),
        ("empty", "FIXTURE 1 AT COLOR"),
        ("points", "FIXTURE 1 THRU 2 AT COLOR 0 THRU 50 THRU 100"),
    ] {
        let body = run(&scenario, id, command).await;
        assert_eq!(body["outcome"], "rejected", "{command}: {body}");
    }
    assert!(close(read(colors[0]), [1., 0., 0.]));
    let _ = std::fs::remove_dir_all(&scenario.data_dir);
}

#[tokio::test]
async fn a_live_group_takes_a_typed_color_spread_as_its_own_value() {
    let Rig { scenario, .. } = rig().await;
    accepted(
        &scenario,
        "group",
        "GROUP 1 AT COLOR 100 DIV 0 DIV 0 THRU 0 DIV 0 DIV 100",
    )
    .await;
    let programmer = scenario.state.programming.get(scenario.session.id).unwrap();
    let group = programmer
        .group_values
        .get("1")
        .and_then(|values| values.get(&ProgrammingOwner::Color.key()))
        .expect("the live Group stores its own Color");
    assert!(matches!(group.value, AttributeValue::ColorProgram(_)));
    assert!(programmer.values.is_empty(), "{:?}", programmer.values);
    let _ = std::fs::remove_dir_all(&scenario.data_dir);
}

#[tokio::test]
async fn position_angles_take_absolute_negative_and_relative_degrees() {
    let Rig {
        scenario, mover, ..
    } = rig().await;
    accepted(&scenario, "angles", "FIXTURE 4 AT POSITION 45 DIV - - 30").await;
    let angles = |scenario: &CommandHttpScenario| match fixture_value(
        scenario,
        mover,
        ProgrammingOwner::Position,
    ) {
        Some(AttributeValue::Position(intent)) => match intent.as_ref() {
            PositionIntent::Angles {
                pan_degrees: ScalarIntent::Value(pan),
                tilt_degrees: ScalarIntent::Value(tilt),
            } => (*pan, *tilt),
            other => panic!("Angles, got {other:?}"),
        },
        other => panic!("a Position intent, got {other:?}"),
    };
    assert_eq!(angles(&scenario), (45., -30.));
    accepted(&scenario, "pan-step", "FIXTURE 4 AT POSITION - 50").await;
    assert_eq!(angles(&scenario), (-5., -30.));
    // A Color value on a fixture without Color changes nothing and is not an error.
    let body = run(&scenario, "none", "FIXTURE 4 AT COLOR 100").await;
    assert_eq!(body["outcome"], "accepted", "{body}");
    assert!(fixture_value(&scenario, mover, ProgrammingOwner::Color).is_none());
    let _ = std::fs::remove_dir_all(&scenario.data_dir);
}

#[tokio::test]
async fn focus_and_zoom_share_the_focus_page_order() {
    let scenario = CommandHttpScenario::new().await;
    // The Cameo AURO SPOT Z300 declares Focus and a beam Zoom convention.
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../assets/fixture-library/cameo--auro-spot-z300.toskfixture");
    let profile = light_fixture::read_fixture_package(&std::fs::read(path).unwrap()).unwrap();
    let fixture = FixtureId::new();
    let mut spot = patched(&profile, fixture, 1);
    spot.definition = profile.resolved_definition(profile.modes[0].id).unwrap();
    scenario
        .state
        .output
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![spot].into(),
            groups: vec![light_programmer::GroupDefinition {
                id: "1".into(),
                name: "Spot".into(),
                fixtures: vec![fixture],
                ..Default::default()
            }]
            .into(),
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    accepted(&scenario, "focus-zoom", "GROUP 1 AT FOCUS 40 DIV 20").await;
    let programmer = scenario.state.programming.get(scenario.session.id).unwrap();
    let group = |owner: ProgrammingOwner| {
        programmer
            .group_values
            .get("1")
            .and_then(|values| values.get(&owner.key()))
            .map(|value| value.value.clone())
    };
    assert_eq!(
        group(ProgrammingOwner::Focus),
        Some(AttributeValue::Normalized(0.4))
    );
    // Zoom degrees adopt the displayed opening like a first encoder edit; with no published
    // output frame in this unit rig the Zoom edit holds quietly (the e2e covers the DMX).
    assert_ne!(
        group(ProgrammingOwner::Zoom),
        Some(AttributeValue::Normalized(0.2))
    );
    accepted(&scenario, "degroup", "DEGROUP 1 AT FOCUS 55").await;
    assert_eq!(
        fixture_value(&scenario, fixture, ProgrammingOwner::Focus),
        Some(AttributeValue::Normalized(0.55))
    );
    let _ = std::fs::remove_dir_all(&scenario.data_dir);
}

#[tokio::test]
async fn preload_captures_a_typed_family_value_in_the_pending_preload() {
    let Rig {
        scenario, colors, ..
    } = rig().await;
    scenario
        .state
        .programming
        .set_modes(scenario.session.id, Some(true), None, None, None);
    scenario
        .state
        .programming
        .arm_preload(scenario.session.id, true);
    accepted(&scenario, "preload", "FIXTURE 1 AT COLOR 0 DIV 100 DIV 0").await;
    assert!(fixture_value(&scenario, colors[0], ProgrammingOwner::Color).is_none());
    let snapshot = json(scenario.preload_values_snapshot().await).await;
    let text = snapshot.to_string();
    assert!(
        text.contains(&colors[0].0.to_string()) && text.contains("color"),
        "{snapshot}"
    );
    let _ = std::fs::remove_dir_all(&scenario.data_dir);
}

#[tokio::test]
async fn osc_family_writes_parse_like_the_command_line() {
    use super::super::osc_family_values::osc_family_values;
    use light_core::programming::{ProgrammingComponent, ScalarEdit};
    let pan = osc_family_values("pan", &[OscArgument::Float(-90.)]).unwrap();
    assert_eq!(pan.edits.len(), 1);
    assert_eq!(pan.edits[0].0.component, ProgrammingComponent::Pan);
    assert_eq!(pan.edits[0].1, ScalarEdit::Set(ScalarIntent::Value(-90.)));
    let red = osc_family_values("red", &[OscArgument::Int(0), OscArgument::Float(100.)]).unwrap();
    assert_eq!(
        red.edits[0].1,
        ScalarEdit::Set(ScalarIntent::Spread(vec![0., 1.]))
    );
    assert_eq!(red.spread_points, 2);
    let zoom = osc_family_values("zoom", &[OscArgument::String("25".into())]).unwrap();
    assert_eq!(zoom.edits[0].0.component, ProgrammingComponent::Zoom);
    assert!(osc_family_values("hue", &[OscArgument::Float(1.)]).is_err());
    assert!(osc_family_values("red", &[]).is_err());
    assert!(osc_family_values("red", &[OscArgument::Float(140.)]).is_err());
}

#[tokio::test]
async fn absolute_position_tuple_replaces_canonical_dls_target_with_distinct_copy_poses() {
    use light_core::programming::TargetReference;
    use light_fixture::{FixtureLocation, MultiPatchInstance};
    for preload in [false, true] {
        let clock = Arc::new(light_core::ManualClock::new(
            chrono::DateTime::from_timestamp_millis(1_000_000).unwrap(),
        ));
        let scenario = CommandHttpScenario::from_state(super::test_state_with_family_adapters(
            light_programmer::ProgrammerRegistry::with_clock(clock.clone()),
            Some(clock.clone()),
            light_core::programming::SUPPORTED_PROGRAMMING_CONTRACT,
        ))
        .await;
        let (point_id, mover, _) =
            super::aim_point_tests::install_semantic_aim_rig(&scenario.state);
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../assets/fixture-library/robe--robin-dls-profile.toskfixture");
        let profile = light_fixture::read_fixture_package(&std::fs::read(path).unwrap()).unwrap();
        let mode = profile.modes.iter().find(|m| m.name == "Mode 3").unwrap();
        let mut lamp = patched(&profile, mover, 1);
        lamp.definition = profile.resolved_definition(mode.id).unwrap();
        lamp.location = FixtureLocation {
            x: -1000,
            y: 0,
            z: 3000,
        };
        lamp.multipatch = vec![MultiPatchInstance {
            id: Uuid::new_v4(),
            universe: Some(1),
            address: Some(51),
            location: FixtureLocation {
                x: 1000,
                y: 0,
                z: 3000,
            },
            ..Default::default()
        }];
        let mut point = scenario
            .state
            .output
            .snapshot()
            .fixtures
            .iter()
            .find(|f| f.fixture_id == point_id)
            .unwrap()
            .clone();
        point.location = FixtureLocation {
            x: 0,
            y: 1000,
            z: 0,
        };
        scenario
            .state
            .output
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![point, lamp].into(),
                revision: 2,
                ..Default::default()
            })
            .unwrap();
        let target = AttributeValue::Position(Arc::new(PositionIntent::target(
            TargetReference::Point {
                point_id: point_id.0,
            },
            [0.; 3],
        )));
        scenario.state.programming.set(
            scenario.session.id,
            mover,
            ProgrammingOwner::Position.key(),
            target.clone(),
        );
        let frame = scenario
            .state
            .output
            .render_with_playback_events(
                &scenario.state.active_show.output_projection(),
                &scenario.state.playback.render_capability(),
                Default::default(),
            )
            .unwrap();
        scenario.state.output.render_frames_and_publish(
            &frame,
            light_wire::v2::visualization::VisualizationScope { show_id: None },
        );
        let source = scenario.state.output.latest_visualization_frame().unwrap();
        let capture = super::super::position_readout::capture_position_readouts(
            scenario.state.output.engine(),
            &source,
            &[mover],
        );
        // The real accepted Target commands both physical copies; no root-pose guess is valid.
        let capture = capture.expect("accepted current-generation source");
        assert_eq!(
            capture.owners[0].readout.commands.as_ref().unwrap().len(),
            2
        );
        assert!(
            capture.owners[0].common_angles().is_none(),
            "{:?}; programmer={:?}",
            capture.owners[0],
            fixture_value(&scenario, mover, ProgrammingOwner::Position)
        );
        if preload {
            scenario
                .state
                .programming
                .set_modes(scenario.session.id, Some(true), None, None, None);
            scenario
                .state
                .programming
                .arm_preload(scenario.session.id, true);
        }
        accepted(
            &scenario,
            "absolute-copy-angles",
            "FIXTURE 1 AT POSITION 45 DIV 30",
        )
        .await;
        let state = scenario.state.programming.get(scenario.session.id).unwrap();
        let values = if preload {
            &state.preload_pending
        } else {
            state.values.as_ref()
        };
        let position = values
            .iter()
            .find(|v| v.fixture_id == mover && v.attribute == ProgrammingOwner::Position.key())
            .unwrap();
        assert_eq!(
            position.value,
            AttributeValue::Position(Arc::new(PositionIntent::angles(45., 30.)))
        );
        if preload {
            assert_eq!(
                fixture_value(&scenario, mover, ProgrammingOwner::Position),
                Some(target)
            );
        } else {
            clock.advance_millis(3100);
            let rendered = scenario
                .state
                .output
                .render_with_playback_events(
                    &scenario.state.active_show.output_projection(),
                    &scenario.state.playback.render_capability(),
                    Default::default(),
                )
                .unwrap();
            let copies = rendered
                .rendered
                .physical
                .instances
                .iter()
                .filter(|i| i.fixture_id == mover)
                .collect::<Vec<_>>();
            assert_eq!(copies.len(), 2);
            for instance in copies {
                assert_eq!(&instance.native_raw[..2], &[149, 155]);
                let previous = frame
                    .rendered
                    .physical
                    .instances
                    .iter()
                    .find(|i| i.instance_id == instance.instance_id)
                    .unwrap();
                assert_eq!(&instance.native_raw[2..], &previous.native_raw[2..]);
            }
            accepted(
                &scenario,
                "signed-copy-angles",
                "FIXTURE 1 AT POSITION - - 270 DIV - - 30",
            )
            .await;
            assert_eq!(
                fixture_value(&scenario, mover, ProgrammingOwner::Position),
                Some(AttributeValue::Position(Arc::new(PositionIntent::angles(
                    -270., -30.
                ))))
            );
        }
        let _ = std::fs::remove_dir_all(&scenario.data_dir);
    }
}

#[tokio::test]
async fn complete_typed_pair_matching_captured_copy_pose_detaches_target() {
    use light_core::programming::TargetReference;
    use light_fixture::{FixtureLocation, MultiPatchInstance};
    let clock = Arc::new(light_core::ManualClock::new(
        chrono::DateTime::from_timestamp_millis(1_000_000).unwrap(),
    ));
    let scenario = CommandHttpScenario::from_state(super::test_state_with_family_adapters(
        light_programmer::ProgrammerRegistry::with_clock(clock.clone()),
        Some(clock.clone()),
        light_core::programming::SUPPORTED_PROGRAMMING_CONTRACT,
    ))
    .await;
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../assets/fixture-library/robe--robin-dls-profile.toskfixture");
    let profile = light_fixture::read_fixture_package(&std::fs::read(path).unwrap()).unwrap();
    let mode = profile.modes.iter().find(|m| m.name == "Mode 3").unwrap();
    let mover = FixtureId::new();
    let mut lamp = patched(&profile, mover, 1);
    lamp.definition = profile.resolved_definition(mode.id).unwrap();
    lamp.location = FixtureLocation {
        x: 0,
        y: 0,
        z: 3000,
    };
    lamp.multipatch = vec![MultiPatchInstance {
        id: Uuid::new_v4(),
        universe: Some(1),
        address: Some(51),
        location: lamp.location,
        ..Default::default()
    }];
    scenario
        .state
        .output
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![lamp].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    let target = AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Origin,
        [0.; 3],
    )));
    scenario.state.programming.set(
        scenario.session.id,
        mover,
        ProgrammingOwner::Position.key(),
        target.clone(),
    );
    let frame = scenario
        .state
        .output
        .render_with_playback_events(
            &scenario.state.active_show.output_projection(),
            &scenario.state.playback.render_capability(),
            Default::default(),
        )
        .unwrap();
    scenario.state.output.render_frames_and_publish(
        &frame,
        light_wire::v2::visualization::VisualizationScope { show_id: None },
    );
    let source = scenario.state.output.latest_visualization_frame().unwrap();
    let capture = super::super::position_readout::capture_position_readouts(
        scenario.state.output.engine(),
        &source,
        &[mover],
    )
    .unwrap();
    assert_eq!(
        capture.owners[0].readout.commands.as_ref().unwrap().len(),
        2
    );
    let pose = capture.owners[0]
        .common_angles()
        .expect("equal physical copies provide a common accepted pose");
    let signed = |value: f32| {
        if value < 0. {
            format!("- - {}", -value)
        } else {
            value.to_string()
        }
    };
    let partial = format!("FIXTURE 1 AT POSITION {}", signed(pose.pan_degrees));
    accepted(&scenario, "same-captured-pan-only", &partial).await;
    assert_eq!(
        fixture_value(&scenario, mover, ProgrammingOwner::Position),
        Some(target)
    );
    let command = format!(
        "FIXTURE 1 AT POSITION {} DIV {}",
        signed(pose.pan_degrees),
        signed(pose.tilt_degrees)
    );
    accepted(&scenario, "same-captured-pair", &command).await;
    assert_eq!(
        fixture_value(&scenario, mover, ProgrammingOwner::Position),
        Some(AttributeValue::Position(Arc::new(PositionIntent::angles(
            pose.pan_degrees,
            pose.tilt_degrees
        ))))
    );
    let _ = std::fs::remove_dir_all(&scenario.data_dir);
}
