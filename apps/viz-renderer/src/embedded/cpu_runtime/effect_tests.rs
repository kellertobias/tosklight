use super::*;
use crate::embedded::{adopt_provider_values, producer::Producer};
use std::sync::Arc;
use viz_scene::EmitterKind;

#[test]
fn stateful_effect_edges_and_timeline_continue_hidden_and_stop_without_gpu_work() {
    let now = Instant::now();
    let mut scene = crate::demo::build_scene();
    scene
        .emitters
        .retain(|emitter| emitter.kind == EmitterKind::Effect);
    assert_eq!(scene.emitters.len(), 1);
    scene.emitters[0].effect.as_mut().unwrap().script = Some(Arc::from(
        "let count=0,last=0; export function effect(input){let slot=input.dmx[0];if(slot===255&&last!==255)count++;last=slot;return {emitters:slot===0?[]:[{family:'flame',width:.1,height:1,intensity:count/10,density:1,lifetime:1}]}}",
    ));
    scene.emitters[0].effect.as_mut().unwrap().script_key += 100;
    let mut cpu = CpuRuntime::new(
        crate::lasers::Lasers::new(None),
        crate::physics::Physics::new(),
        now,
    );
    assert!(
        cpu.effects.is_none(),
        "no effect VM is created until its first simulation tick"
    );
    let view = ViewConfiguration::default();
    let mut values = SceneValues::default();
    let mut producer = Producer::new(true, now);
    producer.set_visible(false, now);
    for (millis, slot) in [(0, 0), (100, 255), (101, 0), (102, 255), (103, 128)] {
        let mut incoming = SceneValues::default();
        incoming.resize(1);
        incoming.emitters[0].intensity = 1.0;
        incoming.effect_frames[0].slots = vec![slot];
        adopt_provider_values(&mut values, incoming, 0);
        let at = now + Duration::from_millis(millis);
        cpu.advance(&scene, &mut values, &view, millis as f32 / 1000.0, at, true);
        assert!(producer.begin(at).is_none());
        assert_eq!(values.effect_frames[0].fault, None);
    }
    assert!((values.effect_frames[0].emitters[0].intensity - 0.2).abs() < 0.001);
    cpu.advance(
        &scene,
        &mut values,
        &view,
        1.0,
        now + Duration::from_secs(1),
        false,
    );
    assert_eq!(values.effect_frames[0].timeline_seconds, 1.0);
    assert!((values.effect_frames[0].emitters[0].intensity - 0.2).abs() < 0.001);
    cpu.advance(
        &scene,
        &mut values,
        &view,
        1.001,
        now + Duration::from_millis(1001),
        false,
    );
    assert_eq!(
        values.effect_frames[0].timeline_seconds, 1.0,
        "periodic CPU work respects its deadline"
    );
    producer.set_visible(true, now + Duration::from_secs(1));
    assert_eq!(
        values.effect_frames[0].timeline_seconds, 1.0,
        "reveal consumes the current resolved frame"
    );
    cpu.succeeded();
    let mut incoming = SceneValues::default();
    incoming.resize(1);
    incoming.effect_frames[0].slots = vec![0];
    adopt_provider_values(&mut values, incoming, 0);
    cpu.advance(
        &scene,
        &mut values,
        &view,
        1.1,
        now + Duration::from_millis(1100),
        true,
    );
    assert!(values.effect_frames[0].emitters.is_empty());
    assert!(
        cpu.dirty(),
        "a provider slot-only replacement must not hide the effect stop transition"
    );
    scene.emitters.clear();
    cpu.advance(
        &scene,
        &mut values,
        &view,
        1.2,
        now + Duration::from_millis(1200),
        true,
    );
    assert!(
        cpu.effects.is_none(),
        "removed effect fixtures release their bounded VM history"
    );
}
