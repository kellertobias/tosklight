use super::tests_support::*;
use super::*;
use crate::plan::{EffectWindow, LaserWindow, PhysicsWindow};

/// The reason the gate is integrated at all: no rate may vanish because the frames happened to
/// fall in its dark half, and none may read as continuously lit either. Averaged over a
/// second, a quarter-duty strobe delivers a quarter of the light at every rate.
#[test]
fn a_strobe_delivers_its_duty_cycle_at_every_rate_against_every_frame_clock() {
    for hz in [2.0_f32, 15.0, 25.0, 47.0, 90.0] {
        for frame_rate in [30.0_f32, 60.0, 144.0] {
            let step = 1.0 / frame_rate;
            let frames = frame_rate as usize;
            let total: f32 = (0..frames)
                .map(|index| {
                    let now = index as f32 * step;
                    strobe_openness(now, now + step, hz)
                })
                .sum();
            let mean = total / frames as f32;
            assert!(
                (mean - STROBE_DUTY).abs() < 0.02,
                "{hz} Hz at {frame_rate} fps averaged {mean}, wanted {STROBE_DUTY}"
            );
        }
    }
}

/// A strobe well below the frame rate has to still look like a strobe: some frames fully lit,
/// some fully dark. An average that was right but never varied would be a dimmer.
#[test]
fn a_slow_strobe_still_produces_light_and_dark_frames() {
    let step = 1.0 / 60.0;
    let samples: Vec<f32> = (0..60)
        .map(|index| {
            let now = index as f32 * step;
            strobe_openness(now, now + step, 5.0)
        })
        .collect();
    assert!(
        samples.iter().any(|value| *value >= 0.99),
        "no frame caught a full flash"
    );
    assert!(
        samples.iter().any(|value| *value <= 0.01),
        "no frame was fully dark"
    );
}

/// A laser is handed its fixture's raw slots, in patch order, and nothing else.
#[test]
fn a_laser_emitter_captures_its_fixture_footprint() {
    let binding = EmitterBinding {
        universes: vec![1],
        laser_window: Some(LaserWindow {
            logical_universe: 1,
            slots: vec![1, 2, 3, 4],
        }),
        ..EmitterBinding::default()
    };
    let mut decoder = Decoder::new(vec![binding]);
    let scene = scene(&[EmitterKind::Laser]);
    let mut values = SceneValues::default();
    decoder.apply(
        &scene,
        &[frame(&[(0, 10), (1, 20), (2, 30), (3, 40)])],
        &mut values,
        0.0,
    );
    assert_eq!(values.laser_scans[0].slots, vec![10, 20, 30, 40]);
}

#[test]
fn an_effect_program_receives_the_exact_fixture_slots_in_patch_order() {
    let binding = EmitterBinding {
        universes: vec![1],
        effect_window: Some(EffectWindow {
            logical_universe: 1,
            slots: vec![1, 2, 3],
        }),
        ..EmitterBinding::default()
    };
    let mut decoder = Decoder::new(vec![binding]);
    let scene = scene(&[EmitterKind::Effect]);
    let mut values = SceneValues::default();
    decoder.apply(
        &scene,
        &[frame(&[(0, 17), (1, 91), (2, 203)])],
        &mut values,
        0.0,
    );
    assert_eq!(values.effect_frames[0].slots, vec![17, 91, 203]);
}

#[test]
fn a_physics_body_receives_the_exact_fixture_slots_in_patch_order() {
    let binding = EmitterBinding {
        universes: vec![1],
        physics_window: Some(PhysicsWindow {
            body_index: 0,
            logical_universe: 1,
            slots: vec![3, 1, 2],
        }),
        ..EmitterBinding::default()
    };
    let mut decoder = Decoder::new(vec![binding]);
    let mut scene = scene(&[EmitterKind::Emissive]);
    let id = uuid::Uuid::new_v4();
    scene.physics_scenery.push(viz_scene::PhysicsSceneryObject {
        fixture_instance_id: id,
        scenery: viz_scene::SceneryObject {
            id,
            name: "Kabuki".into(),
            size: glam::Vec3::ONE,
            colour: [0.2; 3],
            roughness: 0.8,
            kind: viz_scene::SceneryKind::Curtain,
            chords: 1,
            ..Default::default()
        },
        program: viz_scene::PhysicsProgram::default(),
        body: viz_scene::PhysicsBody {
            mass_kilograms: 1.0,
            gravity_metres_per_second_squared: 9.806_65,
        },
        constraints: viz_scene::PhysicsConstraints {
            floor_y_metres: 0.0,
            scenery_collision: true,
            self_collision: false,
        },
    });
    let mut values = SceneValues::default();
    decoder.apply(
        &scene,
        &[frame(&[(0, 17), (1, 91), (2, 203)])],
        &mut values,
        0.0,
    );
    assert_eq!(values.physics_frames[0].slots, vec![203, 17, 91]);
}

/// A head that is not a laser must never grow a footprint, or every fixture in the show would
/// carry a copy of its own DMX for nothing.
#[test]
fn a_beam_emitter_captures_no_footprint() {
    let binding = EmitterBinding {
        universes: vec![1],
        laser_window: Some(LaserWindow {
            logical_universe: 1,
            slots: vec![1],
        }),
        ..EmitterBinding::default()
    };
    let mut decoder = Decoder::new(vec![binding]);
    let scene = scene(&[EmitterKind::Beam]);
    let mut values = SceneValues::default();
    decoder.apply(&scene, &[frame(&[(0, 255)])], &mut values, 0.0);
    assert!(values.laser_scans[0].slots.is_empty());
}
