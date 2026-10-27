//! Initialization is not an OFF DMX window: use the real CPU script engine to prove both paths.
use super::*;
use std::sync::Arc;
use viz_scene::{EffectProgram, FixtureInstance, ParticleTrigger};

// Matches the shipped Flame Jet's height arithmetic: empty dmx gives NaN, real zeros give .4m.
const FLAME: &str = "export function effect(input){const level=input.dmx[0]/255;return {emitters:[{family:'flame',width:.2,height:.4+input.dmx[1]/255*3.6,intensity:level,density:level,lifetime:.28,state:level>0?'hold':'off'}]}}";
fn rig(script: &str) -> (Effects, Scene, SceneValues) {
    let effects = Effects::new();
    assert!(effects.engine.is_some(), "real CPU effect engine required");
    let mut scene = Scene::default();
    scene.fixtures.push(FixtureInstance {
        instance_id: Uuid::from_u128(1),
        ..Default::default()
    });
    let mut emitter = crate::demo::build_scene()
        .emitters
        .into_iter()
        .find(|emitter| emitter.kind == EmitterKind::Effect)
        .expect("the actual demo has an Effect emitter");
    emitter.fixture_index = 0;
    emitter.effect = Some(EffectProgram {
        script: Some(Arc::from(script)),
        script_key: 17,
        result_version: 1,
    });
    scene.emitters.push(emitter);
    let mut values = SceneValues::default();
    values.resize(1);
    (effects, scene, values)
}
#[test]
fn empty_initial_window_is_deferred_but_a_valid_zero_off_window_executes() {
    let (mut effects, scene, mut values) = rig(FLAME);
    effects.run(&scene, &mut values, 0.0);
    assert!(values.effect_frames[0].emitters.is_empty());
    assert!(values.effect_frames[0].fault.is_none());
    assert!(effects.clocks.is_empty(), "no synthetic startup invocation");
    values.effect_frames[0].slots = vec![0, 0, 0];
    effects.run(&scene, &mut values, 1.0);
    let frame = &values.effect_frames[0];
    assert!(frame.fault.is_none());
    assert_eq!(
        frame.emitters.len(),
        1,
        "an initialized OFF frame must reach the script"
    );
    assert_eq!(frame.emitters[0].trigger, ParticleTrigger::Off);
    assert_eq!(frame.emitters[0].reach_metres, 0.4);
    assert_eq!(frame.emitters[0].intensity, 0.0);
}
#[test]
fn on_to_zero_off_and_later_zero_ticks_preserve_script_state_and_release_tail() {
    let script = "let calls=0; export function effect(input){calls++;return {emitters:[{family:'flame',width:.1,height:1,intensity:input.dmx[0]?1:.5,density:calls/10,lifetime:1,state:input.dmx[0]?'hold':'release'}]}}";
    let (mut effects, scene, mut values) = rig(script);
    values.emitters[0].intensity = 1.0;
    values.effect_frames[0].slots = vec![255];
    effects.run(&scene, &mut values, 0.0);
    assert_eq!(
        values.effect_frames[0].emitters[0].trigger,
        ParticleTrigger::Hold
    );
    values.emitters[0].intensity = 0.0;
    values.effect_frames[0].slots = vec![0];
    effects.run(&scene, &mut values, 1.0);
    let frame = &values.effect_frames[0];
    assert!(frame.fault.is_none());
    assert_eq!(frame.emitters[0].trigger, ParticleTrigger::Release);
    assert_eq!(
        frame.emitters[0].intensity, 0.5,
        "script-owned tail must survive raw OFF"
    );
    assert!((frame.emitters[0].density - 0.2).abs() < 1e-6);
    effects.run(&scene, &mut values, 2.0);
    assert!(
        (values.effect_frames[0].emitters[0].density - 0.3).abs() < 1e-6,
        "unchanged initialized OFF still ticks"
    );
    assert_eq!(
        values.emitters[0].intensity, 0.0,
        "authoritative target untouched"
    );
}
#[test]
fn returning_to_uninitialized_controls_clears_stale_output_and_fault() {
    let (mut effects, scene, mut values) = rig(FLAME);
    values.effect_frames[0].slots = vec![255, 0, 0];
    effects.run(&scene, &mut values, 0.0);
    assert!(!values.effect_frames[0].emitters.is_empty());
    values.effect_frames[0].slots.clear();
    values.effect_frames[0].fault = Some("obsolete previous-window error".into());
    effects.run(&scene, &mut values, 1.0);
    assert!(values.effect_frames[0].emitters.is_empty());
    assert!(values.effect_frames[0].fault.is_none());
    assert!(effects.fault(&values).is_none());
}
#[test]
fn initialized_invalid_descriptor_and_invalid_script_remain_visible_faults() {
    for script in [
        "export function effect(){return {emitters:[{family:'flame',width:.1,height:0,intensity:0,density:0,lifetime:1,state:'off'}]}}",
        "export function effect( {",
    ] {
        let (mut effects, scene, mut values) = rig(script);
        values.effect_frames[0].slots = vec![0, 0, 0];
        effects.run(&scene, &mut values, 0.0);
        assert!(values.effect_frames[0].emitters.is_empty());
        assert!(
            values.effect_frames[0]
                .fault
                .as_ref()
                .is_some_and(|reason| !reason.is_empty())
        );
        assert!(effects.fault(&values).is_some());
    }
}
#[test]
fn missing_packaged_script_is_reported_even_before_control_input() {
    let (mut effects, mut scene, mut values) = rig(FLAME);
    scene.emitters[0].effect.as_mut().unwrap().script = None;
    effects.run(&scene, &mut values, 0.0);
    assert!(
        values.effect_frames[0]
            .fault
            .as_ref()
            .unwrap()
            .contains("ships no effect script")
    );
}
