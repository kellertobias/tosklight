//! What the simulator shows for a head's colour system: a discrete wheel's slot colours, and each
//! head of a multi-head fixture resolving its own colour rather than one shared configuration.

use super::*;
use light_fixture::{ColorWheelSlot, HeadColorSystem};

fn wheel_channel(id: uuid::Uuid, head_id: uuid::Uuid) -> FixtureChannel {
    let (mut fixture, _) = fixture();
    retarget_only_channel(&mut fixture, "color.wheel.1");
    let profile = fixture.definition.profile_snapshot.as_deref().unwrap();
    FixtureChannel {
        id,
        head_id,
        ..profile.modes[0].channels[0].clone()
    }
}

fn slot(id: &str, label: &str, range: (u32, u32), measured: Option<Xyz>) -> ColorWheelSlot {
    ColorWheelSlot {
        steady: None,
        semantic_id: id.into(),
        label: label.into(),
        dmx_from: range.0,
        dmx_to: range.1,
        measured_xyz: measured,
    }
}

/// Two heads, each with its own wheel and its own slot colours.
fn two_wheel_mode() -> (light_fixture::FixtureMode, [uuid::Uuid; 2]) {
    let mut profile = FixtureProfile::blank();
    let mode = &mut profile.modes[0];
    let heads = [uuid::Uuid::new_v4(), uuid::Uuid::new_v4()];
    mode.heads = heads
        .iter()
        .enumerate()
        .map(|(index, id)| FixtureHead {
            id: *id,
            name: format!("Head {}", index + 1),
            master_shared: false,
        })
        .collect();
    let channels = [uuid::Uuid::new_v4(), uuid::Uuid::new_v4()];
    mode.channels = vec![
        wheel_channel(channels[0], heads[0]),
        wheel_channel(channels[1], heads[1]),
    ];
    let measured_amber = Xyz {
        x: 0.6,
        y: 0.5,
        z: 0.05,
    };
    mode.color_systems = vec![
        HeadColorSystem {
            calibration: Default::default(),
            head_id: heads[0],
            correction_matrix: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            system: ColorSystem::DiscreteWheel {
                channel_id: channels[0],
                slots: vec![
                    slot("open", "Open", (0, 9), None),
                    slot("deep_red", "Deep Red", (10, 19), None),
                    slot("amber", "Amber", (20, 29), Some(measured_amber)),
                    slot("gobo_effect", "Effect", (30, 39), None),
                ],
            },
        },
        HeadColorSystem {
            calibration: Default::default(),
            head_id: heads[1],
            correction_matrix: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            system: ColorSystem::DiscreteWheel {
                channel_id: channels[1],
                slots: vec![
                    slot("open", "Open", (0, 9), None),
                    slot(
                        "custom",
                        "Custom",
                        (10, 19),
                        Some(Xyz {
                            x: 0.1,
                            y: 0.2,
                            z: 0.9,
                        }),
                    ),
                ],
            },
        },
    ];
    (profile.modes.remove(0), heads)
}

fn close(actual: Option<Xyz>, expected: Xyz) -> bool {
    actual.is_some_and(|actual| {
        (actual.x - expected.x).abs() < 0.001
            && (actual.y - expected.y).abs() < 0.001
            && (actual.z - expected.z).abs() < 0.001
    })
}

#[test]
fn a_discrete_wheel_renders_its_measured_or_named_slot_colour() {
    let (mode, [head, _]) = two_wheel_mode();
    let fallback = Xyz {
        x: 0.2,
        y: 0.2,
        z: 0.2,
    };
    let at = |raw: u32| profile_visual_color(&mode, head, &[(0, raw), (1, 0)], Some(fallback));

    // A measured slot is authoritative.
    assert!(close(
        at(25),
        Xyz {
            x: 0.6,
            y: 0.5,
            z: 0.05
        }
    ));
    // Unmeasured slots show the colour their name describes: open is white, deep red a dark red.
    assert!(close(at(4), light_fixture::srgb_to_xyz(1.0, 1.0, 1.0)));
    assert!(close(at(12), light_fixture::srgb_to_xyz(0.7, 0.0, 0.0)));
    // A slot no name describes, and a raw between slots, leave the fallback in charge.
    assert!(close(at(35), fallback));
    assert!(close(at(200), fallback));
}

#[test]
fn each_head_resolves_its_own_wheel_colour() {
    let (mode, [first, second]) = two_wheel_mode();
    // The same raw value on both wheels means different colours, because each head owns its own
    // colour system.
    let channels = [(0, 12), (1, 12)];
    assert!(close(
        profile_visual_color(&mode, first, &channels, None),
        light_fixture::srgb_to_xyz(0.7, 0.0, 0.0)
    ));
    assert!(close(
        profile_visual_color(&mode, second, &channels, None),
        Xyz {
            x: 0.1,
            y: 0.2,
            z: 0.9
        }
    ));
}

fn shipped_sunstrip() -> FixtureProfile {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../assets/fixture-library/showtec--sunstrip-led-rgb-42206.toskfixture");
    light_fixture::read_fixture_package(&std::fs::read(path).unwrap()).unwrap()
}

#[test]
fn every_sunstrip_pixel_renders_its_own_additive_colour() {
    let profile = shipped_sunstrip();
    let mode = &profile.modes[0];
    let pixels = mode
        .heads
        .iter()
        .enumerate()
        .filter(|(_, head)| !head.master_shared)
        .collect::<Vec<_>>();
    assert_eq!(pixels.len(), 10);
    assert_eq!(mode.color_systems.len(), 10);

    let (mut fixture, _) = fixture();
    fixture.definition = profile.resolved_definition(mode.id).unwrap();
    fixture.logical_heads = pixels
        .iter()
        .map(|(index, head)| PatchedHead {
            profile_head_id: Some(head.id),
            head_index: *index as u16,
            fixture_id: FixtureId::new(),
        })
        .collect();

    let primaries = [
        light_fixture::srgb_to_xyz(1.0, 0.0, 0.0),
        light_fixture::srgb_to_xyz(0.0, 1.0, 0.0),
        light_fixture::srgb_to_xyz(0.0, 0.0, 1.0),
    ];
    let programmers = ProgrammerRegistry::default();
    let session = SessionId::new();
    programmers.start(session);
    for (index, head) in fixture.logical_heads.iter().enumerate() {
        programmers.set(
            session,
            head.fixture_id,
            AttributeKey::intensity(),
            AttributeValue::Normalized(1.0),
        );
        programmers.set(
            session,
            head.fixture_id,
            AttributeKey("color".into()),
            AttributeValue::ColorXyz(primaries[index % 3]),
        );
    }
    let heads = fixture.logical_heads.clone();
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            ..Default::default()
        })
        .unwrap();
    let rendered = engine.render(RenderOptions::default()).unwrap();
    let frame = &rendered.universes[&1];
    let projected = engine
        .profile_visualization_values(&engine.resolved_values(), RenderOptions::default())
        .unwrap();
    for (index, head) in heads.iter().enumerate() {
        let dmx = &frame[index * 3..index * 3 + 3];
        let lit = index % 3;
        for (emitter, value) in dmx.iter().enumerate() {
            assert_eq!(
                *value > 200,
                emitter == lit,
                "pixel {} emitter {emitter} is {value}",
                index + 1
            );
        }
        let Some(AttributeValue::ColorXyz(color)) =
            projected.get(&(head.fixture_id, AttributeKey("color".into())))
        else {
            panic!("pixel {} has no simulated colour", index + 1);
        };
        let expected = primaries[lit];
        let scale = expected.x.max(expected.y).max(expected.z);
        assert!(
            close(
                Some(*color),
                Xyz {
                    x: expected.x / scale,
                    y: expected.y / scale,
                    z: expected.z / scale,
                }
            ) || close(Some(*color), expected),
            "pixel {} renders {color:?}, not {expected:?}",
            index + 1
        );
    }
}
