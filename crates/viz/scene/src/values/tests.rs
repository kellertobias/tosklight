use super::*;
use crate::scene::{
    BodyKind, EmitterInstance, EmitterKind, EmitterLayoutCells, EmitterOptics, FixtureBody,
    FixtureInstance,
};
use glam::Vec3;

#[test]
fn physical_uv_only_has_no_visible_persistence_without_changing_native_drive() {
    let mut value = EmitterValues {
        intensity: 1.0,
        shutter: 1.0,
        held_intensity: 1.0,
        colour: [0.0; 3],
        uv_drive: 1.0,
        physical_color: Some(PhysicalColorState::default()),
        ..Default::default()
    };
    assert_eq!(value.visible_intensity(), 0.0);
    assert_eq!(value.retained_visible_intensity(), 0.0);
    assert_eq!((value.intensity, value.uv_drive), (1.0, 1.0));
    value.physical_color.as_mut().unwrap().visible_complete = true;
    assert_eq!(
        value.retained_visible_intensity(),
        0.0,
        "measured zero is also invisible"
    );
    value.colour[0] = 0.01;
    assert_eq!(
        value.visible_intensity(),
        1.0,
        "known visible color retains its encoded level"
    );
    value.colour = [0.0; 3];
    value.physical_color = None;
    assert_eq!(
        value.retained_visible_intensity(),
        1.0,
        "legacy render semantics stay unchanged"
    );
}

fn fixture(instance_id: Uuid, name: &str) -> FixtureInstance {
    FixtureInstance {
        instance_id,
        fixture_id: instance_id,
        name: name.into(),
        number: None,
        position: Vec3::ZERO,
        rotation_degrees: Vec3::ZERO,
        position_master: None,
        bracket_degrees: 0.0,
        bracket_hinge: None,
        shaper_degrees: None,
        installed_colour: [1.0; 3],
        installed_shaper_angles_degrees: [0.0; 4],
        drawn_as_scenery: false,
        invisible: false,
        body: FixtureBody {
            size: Vec3::splat(0.3),
            kind: BodyKind::Lantern,
        },
        patched: true,
        address: None,
        model: None,
        fallback: None,
    }
}

fn emitter(fixture_index: u32, head_index: u16) -> EmitterInstance {
    EmitterInstance {
        fixture_index,
        head_index,
        label: "head".into(),
        local_origin: Vec3::ZERO,
        tilt_pivot: Vec3::ZERO,
        local_orientation_degrees: Vec3::ZERO,
        pan: None,
        tilt: None,
        beam_angle_degrees: 10.0,
        field_angle_degrees: 20.0,
        optics: EmitterOptics::default(),
        kind: EmitterKind::Beam,
        cells: EmitterLayoutCells::single(),
        laser: None,
        effect: None,
        live_shaper_angle_roles: [false; 4],
        shaper_roles: [false; 4],
        live_shaper_rotation_role: false,
    }
}

/// A rig of exactly these fixtures and these heads, which is all these tests need of a scene.
fn rig(fixtures: Vec<FixtureInstance>, emitters: Vec<EmitterInstance>) -> Scene {
    Scene {
        fixtures,
        emitters,
        ..Scene::default()
    }
}

/// Two fixtures, each with one head, and the first one is removed.
#[test]
fn a_head_keeps_its_level_when_the_fixture_before_it_is_removed() {
    let first = Uuid::from_u128(1);
    let second = Uuid::from_u128(2);
    let previous = rig(
        vec![fixture(first, "one"), fixture(second, "two")],
        vec![emitter(0, 0), emitter(1, 0)],
    );

    let mut values = SceneValues::default();
    values.resize(2);
    values.emitters[0].intensity = 0.25;
    values.emitters[1].intensity = 0.8;
    values.emitters[1].colour = [1.0, 0.0, 0.0];

    let next = rig(vec![fixture(second, "two")], vec![emitter(0, 0)]);

    values.carry_over(&previous, &next);
    assert_eq!(values.emitters.len(), 1);
    assert_eq!(values.emitters[0].intensity, 0.8);
    assert_eq!(values.emitters[0].colour, [1.0, 0.0, 0.0]);
}

#[test]
fn a_newly_patched_head_starts_at_its_defaults() {
    let known = Uuid::from_u128(1);
    let added = Uuid::from_u128(9);
    let previous = rig(vec![fixture(known, "one")], vec![emitter(0, 0)]);
    let mut values = SceneValues::default();
    values.resize(1);
    values.emitters[0].intensity = 0.5;

    let next = rig(
        vec![fixture(added, "new"), fixture(known, "one")],
        vec![emitter(0, 0), emitter(1, 0)],
    );

    values.carry_over(&previous, &next);
    assert_eq!(values.emitters.len(), 2);
    assert_eq!(values.emitters[0].intensity, 0.0);
    assert_eq!(values.emitters[1].intensity, 0.5);
}

#[test]
fn static_values_do_not_request_display_clock_frames() {
    let mut values = SceneValues::default();
    values.resize(1);
    assert!(!values.is_time_driven(&PersistencePreference::default()));
}

#[test]
fn persistence_and_unsettled_motion_request_display_clock_frames() {
    let mut values = SceneValues::default();
    values.resize(1);
    values.emitters[0].held_intensity = 1.0;
    assert!(values.is_time_driven(&PersistencePreference::default()));
    values.emitters[0].held_intensity = 0.0;
    values.emitters[0]
        .pan_motion
        .set_target(PhysicalMotionTarget::Position {
            degrees: 90.0,
            max_speed: 180.0,
            acceleration: 360.0,
            deceleration: 360.0,
        });
    assert!(values.is_time_driven(&PersistencePreference::default()));
}

#[test]
fn a_settled_position_target_is_static() {
    let mut values = SceneValues::default();
    values.resize(1);
    values.emitters[0].pan_motion = PhysicalMotionState {
        position_degrees: 90.0,
        velocity_degrees_per_second: 0.0,
        target: Some(PhysicalMotionTarget::Position {
            degrees: 90.0,
            max_speed: 180.0,
            acceleration: 360.0,
            deceleration: 360.0,
        }),
    };
    assert!(!values.is_time_driven(&PersistencePreference::default()));
}

/// Heads of one multi-head fixture are told apart by their head index, not by their order.
#[test]
fn each_head_of_a_fixture_keeps_its_own_value() {
    let bar = Uuid::from_u128(7);
    let previous = rig(
        vec![fixture(bar, "bar")],
        vec![emitter(0, 0), emitter(0, 1), emitter(0, 2)],
    );
    let mut values = SceneValues::default();
    values.resize(3);
    for (index, emitter) in values.emitters.iter_mut().enumerate() {
        emitter.intensity = index as f32 / 10.0;
    }

    // The middle head is gone: a mode change that drops a cell.
    let next = rig(
        vec![fixture(bar, "bar")],
        vec![emitter(0, 0), emitter(0, 2)],
    );

    values.carry_over(&previous, &next);
    assert_eq!(values.emitters[0].intensity, 0.0);
    assert_eq!(values.emitters[1].intensity, 0.2);
}

#[test]
fn absolute_motion_preserves_multi_turn_targets_and_respects_limits() {
    let mut motion = PhysicalMotionState::default();
    motion.set_target(PhysicalMotionTarget::Position {
        degrees: 630.0,
        max_speed: 180.0,
        acceleration: 360.0,
        deceleration: 360.0,
    });
    motion.advance(0.25);
    assert!(motion.position_degrees > 0.0 && motion.position_degrees < 630.0);
    assert!(motion.velocity_degrees_per_second <= 180.0);
    for _ in 0..200 {
        motion.advance(0.1);
    }
    assert!((motion.position_degrees - 630.0).abs() < 0.001);
    assert_eq!(motion.velocity_degrees_per_second, 0.0);
}

#[test]
fn position_motion_can_start_toward_a_negative_target_from_rest() {
    let mut motion = PhysicalMotionState::default();
    motion.set_target(PhysicalMotionTarget::Position {
        degrees: -54.0,
        max_speed: 180.0,
        acceleration: 360.0,
        deceleration: 360.0,
    });

    motion.advance(0.1);

    assert!(motion.position_degrees < 0.0);
    assert!(motion.velocity_degrees_per_second < 0.0);
}

#[test]
fn provider_frames_keep_renderer_kinematics_but_replace_the_target() {
    let old_target = PhysicalMotionTarget::Position {
        degrees: -54.0,
        max_speed: 180.0,
        acceleration: 360.0,
        deceleration: 360.0,
    };
    let new_target = PhysicalMotionTarget::Position {
        degrees: 54.0,
        max_speed: 180.0,
        acceleration: 360.0,
        deceleration: 360.0,
    };
    let mut previous = SceneValues::default();
    previous.resize(1);
    previous.emitters[0].pan_motion = PhysicalMotionState {
        position_degrees: -12.0,
        velocity_degrees_per_second: -40.0,
        target: Some(old_target),
    };
    let mut incoming = SceneValues::default();
    incoming.resize(1);
    incoming.emitters[0].pan_motion.target = Some(new_target);

    incoming.retain_visual_motion_runtime_from(&previous);

    assert_eq!(incoming.emitters[0].pan_motion.position_degrees, -12.0);
    assert_eq!(
        incoming.emitters[0].pan_motion.velocity_degrees_per_second,
        -40.0
    );
    assert_eq!(incoming.emitters[0].pan_motion.target, Some(new_target));
}

#[test]
fn a_replaced_target_is_followed_without_teleporting() {
    let mut motion = PhysicalMotionState::default();
    motion.set_target(PhysicalMotionTarget::Position {
        degrees: 90.0,
        max_speed: 90.0,
        acceleration: 180.0,
        deceleration: 180.0,
    });
    motion.advance(0.25);
    let before = motion.position_degrees;
    motion.set_target(PhysicalMotionTarget::Position {
        degrees: -90.0,
        max_speed: 90.0,
        acceleration: 180.0,
        deceleration: 180.0,
    });
    assert_eq!(motion.position_degrees, before);
    motion.advance(0.25);
    assert!(motion.position_degrees > -90.0);
}

#[test]
fn endless_motion_accelerates_to_an_authored_signed_velocity() {
    let mut motion = PhysicalMotionState::default();
    motion.set_target(PhysicalMotionTarget::Velocity {
        degrees_per_second: -120.0,
        acceleration: 240.0,
        deceleration: 360.0,
    });
    motion.advance(0.25);
    assert_eq!(motion.velocity_degrees_per_second, -60.0);
    assert_eq!(motion.position_degrees, -15.0);
    motion.advance(0.25);
    assert_eq!(motion.velocity_degrees_per_second, -120.0);
}

#[test]
fn an_ordered_wheel_crosses_intermediate_slots() {
    let mut wheel = WheelMotionState::default();
    wheel.set_target(3, 4, 180.0, 720.0, 720.0);
    let mut visited = Vec::new();
    for _ in 0..20 {
        wheel.advance(0.1);
        let slot = wheel.visible_slot().unwrap();
        if visited.last() != Some(&slot) {
            visited.push(slot);
        }
    }
    assert!(visited.windows(2).all(|pair| pair[0] <= pair[1]));
    assert!(visited.contains(&1));
    assert!(visited.contains(&2));
    assert_eq!(visited.last(), Some(&3));
}

#[test]
fn independent_optical_wheels_traverse_without_resetting_each_other() {
    let mut slow = OpticalWheelValues::default();
    slow.wheel_motion.set_target(3, 4, 90.0, 360.0, 360.0);
    let mut fast = OpticalWheelValues::default();
    fast.wheel_motion.set_target(1, 4, 360.0, 1440.0, 1440.0);
    let mut values = SceneValues::default();
    values.resize(1);
    values.emitters[0].gobo_wheels = vec![slow, fast];
    let mut visited = Vec::new();
    for _ in 0..50 {
        values.apply_physical_motion(0.1);
        let slot = values.emitters[0].gobo_wheels[0].slot(4);
        if visited.last() != Some(&slot) {
            visited.push(slot);
        }
    }
    assert!(visited.contains(&1) && visited.contains(&2));
    assert_eq!(values.emitters[0].gobo_wheels[0].slot(4), 3);
    assert_eq!(values.emitters[0].gobo_wheels[1].slot(4), 1);
}

#[test]
fn a_new_provider_frame_changes_slots_without_raising_a_released_body() {
    let previous = SceneValues {
        physics_frames: vec![PhysicsFrame {
            released: true,
            settled: true,
            position_offset: [0.0, -5.0, 0.0],
            slots: vec![255],
            ..PhysicsFrame::default()
        }],
        ..SceneValues::default()
    };
    let mut next = SceneValues {
        physics_frames: vec![PhysicsFrame {
            slots: vec![64],
            ..PhysicsFrame::default()
        }],
        ..SceneValues::default()
    };
    next.retain_physics_runtime_from(&previous, 1);
    assert!(next.physics_frames[0].released);
    assert!(next.physics_frames[0].settled);
    assert_eq!(next.physics_frames[0].position_offset, [0.0, -5.0, 0.0]);
    assert_eq!(next.physics_frames[0].slots, [64]);
}
