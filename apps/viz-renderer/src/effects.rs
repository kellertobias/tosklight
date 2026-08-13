//! Running package-owned Effect fixture programs once per displayed frame.

use std::collections::HashMap;
use viz_effect::{EffectEngine, EffectRequest};
use viz_scene::{EmitterKind, Scene, SceneValues};

pub struct Effects {
    engine: Option<EffectEngine>,
    unavailable: Option<String>,
    last_run: HashMap<usize, f32>,
}

impl Effects {
    pub fn new() -> Self {
        let (engine, unavailable) = match EffectEngine::new() {
            Ok(engine) => (Some(engine), None),
            Err(reason) => (None, Some(reason)),
        };
        Self {
            engine,
            unavailable,
            last_run: HashMap::new(),
        }
    }

    pub fn run(&mut self, scene: &Scene, values: &mut SceneValues, time: f32) {
        if !scene
            .emitters
            .iter()
            .any(|emitter| emitter.kind == EmitterKind::Effect)
        {
            return;
        }
        let Some(engine) = self.engine.as_mut() else {
            let reason = self
                .unavailable
                .clone()
                .unwrap_or_else(|| "no effect runtime".into());
            for (index, emitter) in scene.emitters.iter().enumerate() {
                if emitter.kind == EmitterKind::Effect
                    && let Some(frame) = values.effect_frames.get_mut(index)
                {
                    frame.emitters.clear();
                    frame.fault = Some(reason.clone());
                }
            }
            return;
        };
        for (index, emitter) in scene.emitters.iter().enumerate() {
            if emitter.kind != EmitterKind::Effect {
                continue;
            }
            let Some(program) = emitter.effect.as_ref() else {
                continue;
            };
            let Some(frame) = values.effect_frames.get_mut(index) else {
                continue;
            };
            let Some(source) = program.script.as_ref() else {
                frame.emitters.clear();
                frame.fault = Some("this Effect fixture's profile ships no effect script".into());
                continue;
            };
            let previous = self.last_run.insert(index, time);
            let elapsed = previous.map_or(0.0, |last| (time - last).max(0.0));
            let fixture = scene.fixtures.get(emitter.fixture_index as usize);
            let identity = fixture
                .map(|fixture| fixture.instance_id.to_string())
                .unwrap_or_default();
            let seed = fixture.map_or(0, |fixture| {
                u64::from_le_bytes(
                    fixture.instance_id.as_bytes()[..8]
                        .try_into()
                        .unwrap_or([0; 8]),
                )
            });
            let produced = engine.run(
                index,
                &EffectRequest {
                    source,
                    source_key: program.script_key,
                    result_version: program.result_version,
                    slots: &frame.slots,
                    time_seconds: f64::from(time),
                    elapsed_seconds: f64::from(elapsed),
                    intensity: values
                        .emitters
                        .get(index)
                        .map_or(0.0, |value| value.intensity),
                    fixture_identity: &identity,
                    capture_seed: seed,
                },
            );
            frame.version = produced.version;
            frame.timeline_seconds = time;
            frame.emitters = produced.emitters;
            frame.fault = produced.fault;
        }
        let live = scene.emitters.len();
        engine.retain(|index| index < live);
        self.last_run.retain(|index, _| *index < live);
    }

    pub fn fault(&self, values: &SceneValues) -> Option<String> {
        if let Some(reason) = &self.unavailable {
            return Some(format!("effect scripts unavailable: {reason}"));
        }
        let faults: Vec<_> = values
            .effect_frames
            .iter()
            .filter_map(|frame| frame.fault.as_deref())
            .collect();
        let first = faults.first()?;
        Some(if faults.len() == 1 {
            format!("effect: {first}")
        } else {
            format!("effect: {first} (and {} more)", faults.len() - 1)
        })
    }
}
