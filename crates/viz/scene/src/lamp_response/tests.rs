use super::*;
use crate::{
    CellValue, EmitterInstance, EmitterOptics, FixtureBody, FixtureInstance, PersistencePreference,
};

const WHITE: [f32; 3] = [1.0; 3];

fn rig(profile: LampResponseProfile) -> (Scene, SceneValues) {
    // Deserialize ordinary semantic types so additive defaulted scene fields need no parallel
    // hard-coded test constructor. This is not fixture-package authoring or a renderer harness.
    let fixture: FixtureInstance = serde_json::from_value(serde_json::json!({
        "instance_id": Uuid::from_u128(1), "fixture_id": Uuid::from_u128(2),
        "name": "Declared source", "number": null, "position": [0,0,0],
        "rotation_degrees": [0,0,0], "bracket_degrees": 0, "shaper_degrees": null,
        "installed_colour": WHITE, "installed_shaper_angles_degrees": [0,0,0,0],
        "lamp_response": profile, "body": FixtureBody::default(),
        "patched": false, "address": null, "model": null, "fallback": null
    }))
    .unwrap();
    let emitter: EmitterInstance = serde_json::from_value(serde_json::json!({
        "fixture_index": 0, "head_index": 0, "label": "head", "local_origin": [0,0,0],
        "tilt_pivot": [0,0,0], "local_orientation_degrees": [0,0,0], "pan": null,
        "tilt": null, "beam_angle_degrees": 10, "field_angle_degrees": 20,
        "optics": EmitterOptics::default(), "kind": "Beam",
        "cells": crate::EmitterLayoutCells::single(), "laser": null, "effect": null,
        "live_shaper_angle_roles": [false,false,false,false], "shaper_roles": [false,false,false,false],
        "live_shaper_rotation_role": false
    })).unwrap();
    let scene = Scene {
        fixtures: vec![fixture],
        emitters: vec![emitter],
        ..Default::default()
    };
    let mut values = SceneValues::default();
    values.resize(1);
    values.advance_lamp_response(&scene, 0.0);
    (scene, values)
}

#[test]
fn rise_is_fast_finite_and_never_overridden_by_raw_target() {
    let (scene, mut values) = rig(LampResponseProfile::TungstenNominal);
    values.emitters[0].intensity = 1.0;
    values.advance_lamp_response(&scene, 0.01);
    assert!((0.3..0.4).contains(&values.emitters[0].retained_visible_intensity()));
    assert_eq!(values.emitters[0].intensity, 1.0);
    assert!(values.is_time_driven(&PersistencePreference {
        decay_seconds: 0.0,
        ..Default::default()
    }));
    values.advance_lamp_response(&scene, 2.0);
    assert_eq!(values.emitters[0].visible_intensity(), 1.0);
    assert!(
        !values.emitters[0]
            .lamp_response
            .as_ref()
            .unwrap()
            .is_moving()
    );
}

#[test]
fn high_fall_is_fast_and_low_tail_is_slower_with_no_knee_step() {
    let mut channel = Channel::new(1.0);
    let crossing = HIGH_FALL_SECONDS * (1.0 / KNEE).ln();
    channel.advance(0.0, crossing);
    assert!((channel.output - KNEE).abs() < 1e-12);
    let mut before = Channel::new(1.0);
    before.advance(0.0, crossing - 1e-8);
    channel.advance(0.0, 1e-8);
    assert!((before.output - channel.output).abs() < 1e-6);
    let mut high = Channel::new(0.8);
    let mut low = Channel::new(0.3);
    high.advance(0.0, 0.01);
    low.advance(0.0, 0.01);
    assert!(high.output / 0.8 < low.output / 0.3);
}

#[test]
fn elapsed_time_partition_and_large_hidden_gap_preserve_the_solution() {
    for target in [0.0, 0.2, 0.4, 0.7, 1.0] {
        let mut whole = Channel::new(0.9);
        let mut partitioned = whole.clone();
        whole.advance(target, 0.16);
        for _ in 0..160 {
            partitioned.advance(target, 0.001);
        }
        assert!((whole.output - partitioned.output).abs() < 1e-10);
        whole.advance(target, 10_000.0);
        assert_eq!(whole.output, level(target));
    }
}

#[test]
fn led_unknown_and_discharge_profiles_remain_instant() {
    let (scene, mut values) = rig(LampResponseProfile::Instant);
    for target in [1.0, 0.3, 0.0] {
        values.emitters[0].intensity = target;
        values.advance_lamp_response(&scene, 0.0);
        assert_eq!(values.emitters[0].visible_intensity(), target);
        assert!(values.emitters[0].lamp_response.is_none());
    }
}

#[test]
fn optical_shutter_is_after_thermal_source_and_does_not_cool_the_filament() {
    let (scene, mut values) = rig(LampResponseProfile::TungstenNominal);
    values.emitters[0].intensity = 1.0;
    values.advance_lamp_response(&scene, 2.0);
    values.emitters[0].shutter = 0.0;
    values.advance_lamp_response(&scene, 0.5);
    assert_eq!(values.emitters[0].visible_intensity(), 0.0);
    assert_eq!(
        values.emitters[0].lamp_response.as_ref().unwrap().output(),
        1.0
    );
    values.emitters[0].shutter = 1.0;
    assert_eq!(values.emitters[0].visible_intensity(), 1.0);
}

#[test]
fn provider_replacement_keeps_local_state_and_cannot_inject_a_full_target_hold() {
    let (scene, mut previous) = rig(LampResponseProfile::TungstenNominal);
    previous.emitters[0].intensity = 1.0;
    previous.advance_lamp_response(&scene, 0.01);
    let before = previous.emitters[0].visible_intensity();
    let mut incoming = SceneValues::default();
    incoming.resize(1);
    incoming.emitters[0].intensity = 1.0;
    incoming.emitters[0].held_intensity = 1.0;
    incoming.retain_visual_motion_runtime_from(&previous);
    incoming.advance_lamp_response(&scene, 0.0);
    assert_eq!(incoming.emitters[0].retained_visible_intensity(), before);
    incoming.advance_lamp_response(&scene, 0.01);
    assert!(incoming.emitters[0].visible_intensity() > before);
}

#[test]
fn cells_keep_independent_tails_pov_uses_simulated_output_and_state_is_scene_bounded() {
    let (scene, mut values) = rig(LampResponseProfile::TungstenNominal);
    values.emitters[0].cells = vec![CellValue::default(); 3];
    values.advance_lamp_response(&scene, 0.0);
    values.emitters[0].cells[0].intensity = 1.0;
    values.advance_lamp_response(&scene, 0.01);
    values.apply_persistence(&PersistencePreference::default(), 0.01);
    assert!((0.3..0.4).contains(&values.emitters[0].resolved_cell_intensity(0)));
    values.emitters[0].shutter = 0.0;
    assert!(values.is_time_driven(&PersistencePreference::default()));
    assert_eq!(
        values.emitters[0]
            .lamp_response
            .as_ref()
            .unwrap()
            .cells
            .len(),
        1
    );
    values.emitters[0].cells.clear();
    values.advance_lamp_response(&scene, 0.01);
    assert!(
        values.emitters[0]
            .lamp_response
            .as_ref()
            .unwrap()
            .cells
            .is_empty()
    );
}

#[test]
fn frozen_clone_is_immutable_and_seek_or_new_physical_identity_drops_history() {
    let (mut scene, mut values) = rig(LampResponseProfile::TungstenNominal);
    values.emitters[0].intensity = 1.0;
    values.advance_lamp_response(&scene, 0.01);
    let frozen = values.clone();
    values.advance_lamp_response(&scene, 1.0);
    assert!(frozen.emitters[0].visible_intensity() < 0.5);
    values.emitters[0].intensity = 0.2;
    scene.fixtures[0].instance_id = Uuid::from_u128(99);
    values.advance_lamp_response(&scene, 0.0);
    assert_eq!(values.emitters[0].visible_intensity(), 0.2);
    values.reset_lamp_response();
    assert!(values.emitters[0].lamp_response.is_none());
}

#[test]
fn invalid_elapsed_is_inert_and_cooling_uses_only_current_source_colour() {
    let (scene, mut values) = rig(LampResponseProfile::TungstenNominal);
    values.emitters[0].intensity = 1.0;
    values.emitters[0].colour = [1.0, 0.25, 0.1];
    values.advance_lamp_response(&scene, 2.0);
    values.emitters[0].intensity = 0.0;
    for elapsed in [f64::NAN, f64::INFINITY, -1.0] {
        values.advance_lamp_response(&scene, elapsed);
        assert_eq!(values.emitters[0].visible_intensity(), 1.0);
    }
    values.advance_lamp_response(&scene, 0.05);
    assert!(values.emitters[0].visible_intensity() > 0.0);
    assert_eq!(
        values.emitters[0].resolved_source_colour(),
        [1.0, 0.25, 0.1]
    );
    // Colour blackout can also make legacy decoded intensity zero: never infer that zero
    // means an electrical power cut and substitute a previous white/coloured source.
    values.emitters[0].colour = [0.0; 3];
    values.emitters[0].held_intensity = 1.0;
    values.advance_lamp_response(&scene, 0.01);
    assert_eq!(values.emitters[0].resolved_source_colour(), [0.0; 3]);
    assert_eq!(values.emitters[0].retained_visible_intensity(), 0.0);
    values.emitters[0].physical_color = Some(crate::PhysicalColorState::default());
    values.emitters[0].uv_drive = 1.0;
    assert_eq!(values.emitters[0].visible_intensity(), 0.0);
    values.emitters[0].intensity = 0.5;
    values.advance_lamp_response(&scene, 0.01);
    assert_eq!(values.emitters[0].retained_visible_intensity(), 0.0);
}

#[test]
fn changed_fixture_identity_cannot_inherit_previous_scalar_or_cell_observer_hold() {
    let (mut scene, mut previous) = rig(LampResponseProfile::TungstenNominal);
    previous.emitters[0].cells = vec![CellValue::default()];
    previous.advance_lamp_response(&scene, 0.0);
    previous.emitters[0].held_intensity = 1.0;
    previous.emitters[0].cells[0].held_intensity = 1.0;
    let mut next = SceneValues::default();
    next.resize(1);
    next.emitters[0].cells = vec![CellValue::default()];
    next.retain_visual_motion_runtime_from(&previous);
    assert_eq!(next.emitters[0].held_intensity, 1.0);
    scene.fixtures[0].instance_id = Uuid::from_u128(9);
    next.advance_lamp_response(&scene, 0.0);
    assert_eq!(next.emitters[0].retained_visible_intensity(), 0.0);
    assert_eq!(next.emitters[0].resolved_cell_intensity(0), 0.0);
    assert_eq!(next.emitters[0].held_intensity, 0.0);
    assert_eq!(next.emitters[0].cells[0].held_intensity, 0.0);
    // Identity rejection must also precede switching to an instantaneous source profile.
    next.retain_visual_motion_runtime_from(&previous);
    scene.fixtures[0].lamp_response = LampResponseProfile::Instant;
    next.advance_lamp_response(&scene, 0.0);
    assert_eq!(next.emitters[0].retained_visible_intensity(), 0.0);
}

#[test]
fn invisible_thermal_state_keeps_advancing_without_requesting_pictures() {
    for black in [false, true] {
        let (scene, mut values) = rig(LampResponseProfile::TungstenNominal);
        values.emitters[0].cells = vec![CellValue::default()];
        values.advance_lamp_response(&scene, 0.0);
        values.emitters[0].intensity = 1.0;
        values.emitters[0].cells[0].intensity = 1.0;
        if black {
            values.emitters[0].colour = [0.0; 3];
            values.emitters[0].cells[0].colour = [0.0; 3];
            // Invisible observer state must not override the colour blackout eligibility.
            values.emitters[0].held_intensity = 1.0;
            values.emitters[0].cells[0].held_intensity = 1.0;
        } else {
            values.emitters[0].shutter = 0.0;
        }
        assert!(!values.advance_lamp_response(&scene, 0.01));
        assert!(
            values.emitters[0]
                .lamp_response
                .as_ref()
                .unwrap()
                .is_moving()
        );
        assert!(!values.is_time_driven(&PersistencePreference::default()));
        let frozen = values.clone();
        assert!(!values.advance_lamp_response(&scene, 2.0));
        let state = values.emitters[0].lamp_response.as_ref().unwrap();
        assert_eq!(state.output(), 1.0);
        assert_eq!(state.cell_output(0), Some(1.0));
        assert!(!state.is_moving());
        assert!(frozen.emitters[0].lamp_response.as_ref().unwrap().output() < 0.4);
        values.emitters[0].shutter = 1.0;
        values.emitters[0].colour = WHITE;
        values.emitters[0].cells[0].colour = WHITE;
        assert_eq!(values.emitters[0].visible_intensity(), 1.0);
        assert_eq!(values.emitters[0].visible_cell_intensity(0), 1.0);
    }
}

#[test]
fn visible_zero_output_warmup_is_time_driven_from_its_first_instant() {
    let (scene, mut values) = rig(LampResponseProfile::TungstenNominal);
    values.emitters[0].intensity = 1.0;
    assert!(!values.advance_lamp_response(&scene, 0.0));
    assert_eq!(values.emitters[0].visible_intensity(), 0.0);
    assert!(values.is_time_driven(&PersistencePreference::default()));
    assert!(values.advance_lamp_response(&scene, 0.01));
}

#[test]
fn same_physical_identity_changed_to_instant_drops_thermal_and_observer_history() {
    let (mut scene, mut values) = rig(LampResponseProfile::TungstenNominal);
    values.emitters[0].cells = vec![CellValue::default()];
    values.advance_lamp_response(&scene, 0.0);
    values.emitters[0].held_intensity = 1.0;
    values.emitters[0].cells[0].held_intensity = 1.0;
    let original = scene.fixtures[0].instance_id;
    scene.fixtures[0].lamp_response = LampResponseProfile::Instant;
    assert!(values.advance_lamp_response(&scene, 0.0));
    assert_eq!(scene.fixtures[0].instance_id, original);
    assert!(values.emitters[0].lamp_response.is_none());
    assert_eq!(values.emitters[0].held_intensity, 0.0);
    assert_eq!(values.emitters[0].cells[0].held_intensity, 0.0);
    assert_eq!(values.emitters[0].retained_visible_intensity(), 0.0);
    assert_eq!(values.emitters[0].resolved_cell_intensity(0), 0.0);
}
